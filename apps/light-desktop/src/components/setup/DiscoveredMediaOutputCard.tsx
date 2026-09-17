import { Button, FormLayout, NumberField } from "@tosklight/ui";
import { type Dispatch, type SetStateAction, useMemo, useState } from "react";
import type {
	DiscoveredMediaOutput,
	DiscoveredMediaServer,
} from "../../api/client/mediaOutput";
import type { FixtureDefinition, PatchedFixture } from "../../api/types";
import { useFixtureLibrary } from "../../features/fixtureLibrary/FixtureLibraryContext";
import type { MediaServersState } from "../../features/mediaServers/MediaServersContext";
import {
	changedPatchFixtureCandidate,
	newPatchFixtureCandidate,
} from "../../features/patch/model";
import { usePatch } from "../../features/patch/PatchContext";
import { mergeFixtureDefinitions } from "./fixtureProfileModel";
import {
	discoveredOutputFacts,
	mediaServerDefinition,
	patchedModeMismatch,
} from "./mediaDiscoveryModel";

/** One discovered Media Server output, its current configuration, and its patch actions. */
export function DiscoveredMediaOutputCard({
	candidate,
	output,
	fixtures,
	server,
	onRemoteUpdated,
}: {
	candidate: DiscoveredMediaServer;
	output: DiscoveredMediaOutput;
	fixtures: readonly PatchedFixture[];
	server: MediaServersState | null;
	onRemoteUpdated: (serverKey: string, output: DiscoveredMediaOutput) => void;
}) {
	const patch = usePatch();
	const fixtureLibrary = useFixtureLibrary();
	// The shipped ToskLight Media Server lives in the profile library; legacy definitions are
	// merged in only for older installations.
	const library = useMemo(
		() =>
			mergeFixtureDefinitions(
				fixtureLibrary?.fixtureProfiles ?? [],
				fixtureLibrary?.fixtureLibrary ?? [],
			),
		[fixtureLibrary?.fixtureProfiles, fixtureLibrary?.fixtureLibrary],
	);
	const key = `${candidate.key}:${output.id}`;
	const [busy, setBusy] = useState<string | null>(null);
	const [message, setMessage] = useState<Record<string, string>>({});
	const [addressOpen, setAddressOpen] = useState(false);
	const [addressDraft, setAddressDraft] = useState({ universe: 1, address: 1 });
	const patched = matchingDiscoveredFixture(fixtures, candidate, output);
	const mismatch = patchedModeMismatch(patched, output);
	const patchable = Boolean(output.mode);
	const patchAt = (universe: number, address: number, changeRemote: boolean) =>
		void patchDiscoveredServer({
			candidate,
			output,
			patched,
			universe,
			address,
			changeRemote,
			patch,
			library,
			server,
			key,
			setBusy,
			setMessage,
			onRemoteUpdated,
		});
	return (
		<article className="media-server-card">
			<header>
				<div>
					<b>
						{candidate.name} · {output.name}
					</b>
					<small>
						{candidate.host} · {candidate.status}
					</small>
				</div>
				<strong>
					{!patchable
						? "Needs update"
						: mismatch
							? "Mode differs"
							: patched
								? "Patched"
								: "Not patched"}
				</strong>
			</header>
			<p>{discoveredOutputFacts(output)}</p>
			{output.issue && <p role="alert">{output.issue}</p>}
			{mismatch && <p role="alert">{mismatch}</p>}
			{output.dmxPendingRestart && (
				<p role="status">The Media Server has a DMX change pending restart.</p>
			)}
			<div className="media-actions">
				<Button
					disabled={!patchable || busy === key}
					onClick={() => patchAt(output.universe, output.startAddress, false)}
				>
					Patch suggested
				</Button>
				<Button
					disabled={!patchable || busy === key}
					onClick={() => {
						setAddressOpen(true);
						setAddressDraft({
							universe: output.universe,
							address: output.startAddress,
						});
					}}
				>
					Patch address
				</Button>
			</div>
			{addressOpen && (
				<FormLayout labelPlacement="top" columns={2}>
					<NumberField
						label="Universe"
						min="1"
						max="65535"
						value={addressDraft.universe}
						onChange={(event) =>
							setAddressDraft((current) => ({
								...current,
								universe: Number(event.target.value),
							}))
						}
					/>
					<NumberField
						label="Address"
						min="1"
						max="512"
						value={addressDraft.address}
						onChange={(event) =>
							setAddressDraft((current) => ({
								...current,
								address: Number(event.target.value),
							}))
						}
					/>
					<Button
						onClick={() =>
							patchAt(addressDraft.universe, addressDraft.address, true)
						}
					>
						Confirm patch address
					</Button>
				</FormLayout>
			)}
			{message[key] && <p role="status">{message[key]}</p>}
		</article>
	);
}

function matchingDiscoveredFixture(
	fixtures: readonly PatchedFixture[],
	server: DiscoveredMediaServer,
	output: DiscoveredMediaOutput,
): PatchedFixture | undefined {
	return fixtures.find(
		(fixture) =>
			fixture.direct_control?.ip_address === server.host &&
			(fixture.internal_bindings?.output === output.id ||
				(fixture.universe === output.universe &&
					fixture.address === output.startAddress)),
	);
}

async function patchDiscoveredServer(input: {
	candidate: DiscoveredMediaServer;
	output: DiscoveredMediaOutput;
	patched?: PatchedFixture;
	universe: number;
	address: number;
	changeRemote: boolean;
	patch: ReturnType<typeof usePatch>;
	library: readonly FixtureDefinition[];
	server: MediaServersState | null;
	key: string;
	setBusy: Dispatch<SetStateAction<string | null>>;
	setMessage: Dispatch<SetStateAction<Record<string, string>>>;
	onRemoteUpdated: (serverKey: string, output: DiscoveredMediaOutput) => void;
}): Promise<void> {
	const setMessage = (message: string) =>
		input.setMessage((current) => ({ ...current, [input.key]: message }));
	if (
		!Number.isInteger(input.universe) ||
		input.universe < 1 ||
		input.universe > 65535 ||
		!Number.isInteger(input.address) ||
		input.address < 1 ||
		input.address > 512
	) {
		setMessage(
			"Choose a universe from 1 to 65535 and an address from 1 to 512.",
		);
		return;
	}
	const mode = input.output.mode;
	if (!mode) {
		setMessage(
			input.output.issue ??
				"This Media Server output cannot be patched from this desk. Update ToskLight Media, then refresh discovery.",
		);
		return;
	}
	const definition = mediaServerDefinition(input.library, mode);
	if (!definition) {
		setMessage(
			`The ToskLight Media Server ${mode} fixture profile is unavailable. Restore it in the Fixture Library, then retry.`,
		);
		return;
	}
	input.setBusy(input.key);
	setMessage("Validating desk patch…");
	const original = input.patched;
	const fixture = original
		? changedPatchFixtureCandidate(original, {
				definition,
				universe: input.universe,
				address: input.address,
				split_patches: [
					{ split: 1, universe: input.universe, address: input.address },
				],
				direct_control: {
					protocol: "citp",
					ip_address: input.candidate.host,
					port: input.candidate.citpPort,
				},
				internal_bindings: {
					...original.internal_bindings,
					output: input.output.id,
				},
			})
		: (() => {
				const nextNumber =
					Math.max(
						0,
						...input.patch.fixtures.map(
							(candidate) => candidate.fixture_number ?? 0,
						),
					) + 1;
				const fresh = newPatchFixtureCandidate({
					name: `${input.candidate.name} ${input.output.name}`,
					fixture_number: nextNumber,
					definition,
					universe: input.universe,
					address: input.address,
				});
				return changedPatchFixtureCandidate(fresh.fixture, {
					direct_control: {
						protocol: "citp",
						ip_address: input.candidate.host,
						port: input.candidate.citpPort,
					},
					internal_bindings: { library: null, output: input.output.id },
				});
			})();
	try {
		const patched = await input.patch.patchFixtures([fixture]);
		if (!patched) {
			setMessage(
				input.patch.error ??
					"The desk patch was rejected. Resolve the Patch conflict and retry.",
			);
			return;
		}
		if (!input.changeRemote) {
			setMessage(`Patched at DMX ${input.universe}.${input.address}.`);
			return;
		}
		setMessage("Updating the selected Media Server…");
		try {
			if (!input.server)
				throw new Error("The Media Server connection is unavailable.");
			const updated = await input.server.updateDiscoveredMediaAddress({
				host: input.candidate.host,
				outputId: input.output.id,
				universe: input.universe,
				startAddress: input.address,
			});
			input.onRemoteUpdated(input.candidate.key, updated);
			setMessage(
				updated.dmxPendingRestart
					? `Patched at DMX ${input.universe}.${input.address}. Restart the Media Server to activate its new DMX input.`
					: `Desk and Media Server now use DMX ${input.universe}.${input.address}.`,
			);
		} catch (error) {
			const rolledBack = original
				? Boolean(
						await input.patch.patchFixtures([
							changedPatchFixtureCandidate(original, {}),
						]),
					)
				: await input.patch.deleteFixture(fixture.fixture.fixture_id);
			setMessage(
				`${error instanceof Error ? error.message : "The Media Server could not be updated."} ${rolledBack ? "The desk patch was restored; retry when the server is reachable." : "Desk rollback also failed. The addresses may differ; inspect both sides before retrying."}`,
			);
		}
	} finally {
		input.setBusy(null);
	}
}
