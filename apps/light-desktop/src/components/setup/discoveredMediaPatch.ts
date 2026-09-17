import type {
	DiscoveredMediaOutput,
	DiscoveredMediaServer,
} from "../../api/client/mediaOutput";
import type {
	FixtureDefinition,
	OutputRoute,
	PatchedFixture,
	VersionedObject,
} from "../../api/types";
import type { MediaServersState } from "../../features/mediaServers/MediaServersContext";
import {
	changedPatchFixtureCandidate,
	newPatchFixtureCandidate,
} from "../../features/patch/model";
import type { usePatch } from "../../features/patch/PatchContext";
import { mediaServerDefinition } from "./mediaDiscoveryModel";
import {
	deskDmxInputFor,
	deskUniverseReaching,
	dmxInputLabel,
	type MediaDmxInput,
} from "./mediaPatchCoordination";

export type PatchChoice =
	| { kind: "suggested" }
	| { kind: "address"; universe: number; address: number };

export type DiscoveredPatchInput = {
	candidate: DiscoveredMediaServer;
	output: DiscoveredMediaOutput;
	patched?: PatchedFixture;
	choice: PatchChoice;
	patch: ReturnType<typeof usePatch>;
	library: readonly FixtureDefinition[];
	routes: readonly VersionedObject<OutputRoute>[];
	server: MediaServersState | null;
	/** Progress and outcome text for the card, shown as it changes. */
	report: (message: string) => void;
	onRemoteUpdated: (serverKey: string, output: DiscoveredMediaOutput) => void;
};

type Target = {
	universe: number;
	address: number;
	/** Set for Patch address: what the server must listen to so the desk universe arrives. */
	input: MediaDmxInput | null;
	/** Said after a suggested patch the desk cannot deliver yet. */
	warning: string | null;
};

function resolveTarget(input: DiscoveredPatchInput): Target | string {
	const { output, choice, routes } = input;
	if (choice.kind === "suggested") {
		const reaching = deskUniverseReaching(routes, output);
		return {
			universe: reaching ?? output.universe,
			address: output.startAddress,
			input: null,
			warning:
				reaching === null
					? ` No desk output route sends ${output.protocol === "sacn" ? "sACN" : "Art-Net"} ${output.universe}, so the server receives nothing yet. Add that route under Setup › Outputs, or use Patch address.`
					: null,
		};
	}
	const { universe, address } = choice;
	if (
		!Number.isInteger(universe) ||
		universe < 1 ||
		universe > 65535 ||
		!Number.isInteger(address) ||
		address < 1 ||
		address > 512
	)
		return "Choose a universe from 1 to 65535 and an address from 1 to 512.";
	const sent = deskDmxInputFor(routes, universe, output.protocol);
	if (!sent)
		return `The desk sends no network output for universe ${universe}, so the Media Server could not receive it. Add an output route for universe ${universe} under Setup › Outputs, then retry. Nothing was changed.`;
	return { universe, address, input: sent, warning: null };
}

function patchCandidate(
	input: DiscoveredPatchInput,
	definition: FixtureDefinition,
	target: Target,
) {
	const { candidate, output, patched, patch } = input;
	const endpoint = {
		protocol: "citp" as const,
		ip_address: candidate.host,
		port: candidate.citpPort,
	};
	if (patched)
		return changedPatchFixtureCandidate(patched, {
			definition,
			universe: target.universe,
			address: target.address,
			split_patches: [
				{ split: 1, universe: target.universe, address: target.address },
			],
			direct_control: endpoint,
			internal_bindings: { ...patched.internal_bindings, output: output.id },
		});
	const nextNumber =
		Math.max(
			0,
			...patch.fixtures.map((fixture) => fixture.fixture_number ?? 0),
		) + 1;
	const fresh = newPatchFixtureCandidate({
		name: `${candidate.name} ${output.name}`,
		fixture_number: nextNumber,
		definition,
		universe: target.universe,
		address: target.address,
	});
	return changedPatchFixtureCandidate(fresh.fixture, {
		direct_control: endpoint,
		internal_bindings: { library: null, output: output.id },
	});
}

/**
 * Patches a discovered output. The desk patch is validated (collision, footprint) and committed
 * first; Patch address then moves the server onto the protocol and universe the desk route
 * sends, and restores the desk patch when the server refuses.
 */
export async function patchDiscoveredServer(
	input: DiscoveredPatchInput,
): Promise<void> {
	const { output, report } = input;
	const target = resolveTarget(input);
	if (typeof target === "string") return report(target);
	if (!output.mode)
		return report(
			output.issue ??
				"This Media Server output cannot be patched from this desk. Update ToskLight Media, then Refresh Discovery.",
		);
	const definition = mediaServerDefinition(input.library, output.mode);
	if (!definition)
		return report(
			`The ToskLight Media Server ${output.mode} fixture profile is unavailable. Restore it in the Fixture Library, then retry.`,
		);
	report("Validating desk patch…");
	const fixture = patchCandidate(input, definition, target);
	const refused = await deskPatchRefusal(input, fixture);
	if (refused) return report(refused);
	const desk = `DMX ${target.universe}.${target.address}`;
	if (!target.input)
		return report(`Patched at ${desk}.${target.warning ?? ""}`);
	report(`Updating the Media Server to ${dmxInputLabel(target.input)}…`);
	try {
		if (!input.server)
			throw new Error("The Media Server connection is unavailable.");
		const updated = await input.server.updateDiscoveredMediaAddress({
			host: input.candidate.host,
			outputId: output.id,
			universe: target.input.universe,
			startAddress: target.address,
			protocol: target.input.protocol,
		});
		input.onRemoteUpdated(input.candidate.key, updated);
		report(remoteOutcome(desk, target, updated));
	} catch (error) {
		const rolledBack = await rollBack(input, fixture.fixture.fixture_id);
		const reason =
			error instanceof Error && error.message
				? error.message
				: "The Media Server could not be updated.";
		report(
			rolledBack
				? `${reason} The desk patch was restored. Refresh Discovery to confirm the Media Server's address, then retry when the server is reachable.`
				: `${reason} Restoring the desk patch also failed, so the desk and the Media Server may differ. Refresh Discovery to see both sides, then Patch suggested or retry.`,
		);
	}
}

/** Why the desk refused the patch (collision, footprint, authority), or null once it is stored. */
async function deskPatchRefusal(
	input: DiscoveredPatchInput,
	fixture: ReturnType<typeof patchCandidate>,
): Promise<string | null> {
	const refusal = (text: string) =>
		`${/[.!?]$/u.test(text.trim()) ? text.trim() : `${text.trim()}.`} The Media Server was not changed.`;
	const fallback =
		"The desk patch was rejected. Resolve the Patch conflict and retry. The Media Server was not changed.";
	try {
		if (await input.patch.patchFixtures([fixture])) return null;
		return input.patch.error ? refusal(input.patch.error) : fallback;
	} catch (error) {
		return error instanceof Error && error.message
			? refusal(error.message)
			: fallback;
	}
}

function remoteOutcome(
	desk: string,
	target: Target,
	updated: DiscoveredMediaOutput,
): string {
	const listens = target.input as MediaDmxInput;
	if (
		updated.protocol !== listens.protocol ||
		updated.universe !== listens.universe ||
		updated.startAddress !== target.address
	)
		return `The desk is patched at ${desk}, but the Media Server kept ${updated.protocol === "sacn" ? "sACN" : "Art-Net"} ${updated.universe} at address ${updated.startAddress}. Patch suggested or retry Patch address.`;
	return updated.dmxPendingRestart
		? `Patched at ${desk}. The Media Server listens to ${dmxInputLabel(listens)} after its next restart; restart it to activate the new DMX input.`
		: `Desk and Media Server now use ${desk}; the server listens to ${dmxInputLabel(listens)}.`;
}

async function rollBack(
	input: DiscoveredPatchInput,
	fixtureId: string,
): Promise<boolean> {
	try {
		return input.patched
			? Boolean(
					await input.patch.patchFixtures([
						changedPatchFixtureCandidate(input.patched, {}),
					]),
				)
			: await input.patch.deleteFixture(fixtureId);
	} catch {
		return false;
	}
}
