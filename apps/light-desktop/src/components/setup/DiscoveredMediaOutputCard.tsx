import { Button, FormLayout, NumberField } from "@tosklight/ui";
import { useMemo, useState } from "react";
import type {
	DiscoveredMediaOutput,
	DiscoveredMediaServer,
} from "../../api/client/mediaOutput";
import type { PatchedFixture } from "../../api/types";
import { useDmxDiagnostics } from "../../features/dmxDiagnostics/DmxDiagnosticsContext";
import { useFixtureLibrary } from "../../features/fixtureLibrary/FixtureLibraryContext";
import type { MediaServersState } from "../../features/mediaServers/MediaServersContext";
import { usePatch } from "../../features/patch/PatchContext";
import {
	type PatchChoice,
	patchDiscoveredServer,
} from "./discoveredMediaPatch";
import { mergeFixtureDefinitions } from "./fixtureProfileModel";
import { discoveredOutputFacts } from "./mediaDiscoveryModel";
import {
	DISCOVERED_PATCH_LABELS,
	deskUniverseReaching,
	discoveredConnection,
	discoveredPatchState,
	matchingDiscoveredFixture,
} from "./mediaPatchCoordination";

/** One discovered Media Server output, its current configuration, and its patch actions. */
export function DiscoveredMediaOutputCard({
	candidate,
	output,
	fixtures,
	server,
	onRemoteUpdated,
	message,
	onMessage,
}: {
	candidate: DiscoveredMediaServer;
	output: DiscoveredMediaOutput;
	fixtures: readonly PatchedFixture[];
	server: MediaServersState | null;
	onRemoteUpdated: (serverKey: string, output: DiscoveredMediaOutput) => void;
	/** The latest patch outcome for this output, kept by the discovery owner. */
	message?: string;
	onMessage: (text: string) => void;
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
	const routes = useDmxDiagnostics()?.outputRoutes ?? EMPTY_ROUTES;
	const [busy, setBusy] = useState(false);
	const [addressOpen, setAddressOpen] = useState(false);
	const [addressDraft, setAddressDraft] = useState({ universe: 1, address: 1 });
	const patched = matchingDiscoveredFixture(
		fixtures,
		candidate,
		output,
		routes,
	);
	const state = discoveredPatchState(patched, candidate, output, routes);
	const deskUniverse = deskUniverseReaching(routes, output);
	const patchable = Boolean(output.mode);
	const run = (choice: PatchChoice) => {
		setBusy(true);
		void patchDiscoveredServer({
			candidate,
			output,
			patched,
			choice,
			patch,
			library,
			routes,
			server,
			report: onMessage,
			onRemoteUpdated,
		}).finally(() => setBusy(false));
	};
	return (
		<article className="media-server-card" data-patch-state={state.kind}>
			<header>
				<div>
					<b>
						{candidate.name} · {output.name}
					</b>
					<small>
						{discoveredIdentity(candidate)} · Desk connection:{" "}
						{discoveredConnection(patched, server?.mediaServers ?? [])}
					</small>
				</div>
				<strong>{DISCOVERED_PATCH_LABELS[state.kind]}</strong>
			</header>
			<p>{discoveredOutputFacts(output, deskUniverse)}</p>
			{output.issue && <p role="alert">{output.issue}</p>}
			{state.problem && <p role="alert">{state.problem}</p>}
			{output.dmxPendingRestart && (
				<p role="status">The Media Server has a DMX change pending restart.</p>
			)}
			<div className="media-actions">
				<Button
					disabled={!patchable || busy}
					onClick={() => run({ kind: "suggested" })}
				>
					Patch suggested
				</Button>
				<Button
					disabled={!patchable || busy}
					onClick={() => {
						setAddressOpen(true);
						setAddressDraft({
							universe: patched?.universe ?? deskUniverse ?? output.universe,
							address: patched?.address ?? output.startAddress,
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
						disabled={busy}
						onClick={() => run({ kind: "address", ...addressDraft })}
					>
						Confirm patch address
					</Button>
				</FormLayout>
			)}
			{message && <p role="status">{message}</p>}
		</article>
	);
}

const EMPTY_ROUTES: never[] = [];

/** How the operator recognises a server: its address, type, CITP port, and whether it answers. */
export function discoveredIdentity(candidate: DiscoveredMediaServer): string {
	// A server that answered its health check is online, even when it needs an update.
	const online = Boolean(candidate.instance);
	return `${candidate.host} · ToskLight Media · CITP ${candidate.citpPort} · ${online ? "Online" : "Offline"}`;
}
