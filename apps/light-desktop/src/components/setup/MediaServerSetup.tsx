import { Button, FormLayout, NumberField, TextField } from "@tosklight/ui";
import {
	type Dispatch,
	type SetStateAction,
	useEffect,
	useMemo,
	useState,
} from "react";
import type { MediaServerFixture, PatchedFixture } from "../../api/types";
import type {
	DiscoveredMediaOutput,
	DiscoveredMediaServer,
	MediaServerDiscovery,
} from "../../api/client/mediaOutput";
import { useFixtureLibrary } from "../../features/fixtureLibrary/FixtureLibraryContext";
import {
	type MediaServersState,
	useMediaServers,
} from "../../features/mediaServers/MediaServersContext";
import { usePatch, usePatchView } from "../../features/patch/PatchContext";
import {
	changedPatchFixtureCandidate,
	newPatchFixtureCandidate,
} from "../../features/patch/model";

type Draft = { ip: string; port: number };

export function MediaServerSetup({ active = true }: { active?: boolean }) {
	const server = useMediaServers();
	const library = useFixtureLibrary();
	const patch = usePatch();
	usePatchView(active);
	const [drafts, setDrafts] = useState<Record<string, Draft>>({});
	const [busy, setBusy] = useState<string | null>(null);
	const [live, setLive] = useState<Set<string>>(() => new Set());
	const [discovery, setDiscovery] = useState<MediaServerDiscovery | null>(null);
	const [discoveryBusy, setDiscoveryBusy] = useState(false);
	const [discoveryError, setDiscoveryError] = useState<string | null>(null);
	const [patchBusy, setPatchBusy] = useState<string | null>(null);
	const [patchMessage, setPatchMessage] = useState<Record<string, string>>({});
	const [addressChoice, setAddressChoice] = useState<string | null>(null);
	const [addressDraft, setAddressDraft] = useState({ universe: 1, address: 1 });
	const mediaFixtures = useMemo(
		() => patch.fixtures.filter(isMediaFixture),
		[patch.fixtures],
	);
	useEffect(() => {
		setDrafts(Object.fromEntries(mediaFixtures.map(fixtureDraftEntry)));
	}, [mediaFixtures]);
	useEffect(() => {
		if (!live.size || !active) return;
		const timer = window.setInterval(() => {
			for (const fixtureId of live)
				void refreshAdvertisedPreview(server, fixtureId);
		}, 1_000);
		return () => window.clearInterval(timer);
	}, [active, live, server]);
	const refreshDiscovery = async () => {
		if (!server) return;
		setDiscoveryBusy(true);
		setDiscoveryError(null);
		try {
			const next = await server.discoverMediaServers();
			setDiscovery(next);
			setDiscoveryError(next.discoveryError);
		} catch (error) {
			setDiscoveryError(
				error instanceof Error
					? error.message
					: "Media Server discovery is unavailable. Manual patching still works.",
			);
		} finally {
			setDiscoveryBusy(false);
		}
	};
	const updateDiscoveredOutput = (
		serverKey: string,
		output: DiscoveredMediaOutput,
	) =>
		setDiscovery((current) =>
			current
				? {
						...current,
						servers: current.servers.map((candidate) =>
							candidate.key === serverKey
								? {
										...candidate,
										outputs: candidate.outputs.map((currentOutput) =>
											currentOutput.id === output.id ? output : currentOutput,
										),
									}
								: candidate,
						),
					}
				: current,
		);
	useEffect(() => {
		if (active && server) void refreshDiscovery();
		// Discovery is deliberately refreshed when this setup surface becomes active.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [active, server]);
	if (!active || patch.status !== "ready")
		return <p>Patch authority loading…</p>;
	return (
		<div className="media-server-setup">
			<section
				className="media-discovery"
				aria-labelledby="discovered-media-servers"
			>
			<header>
				<div>
					<b id="discovered-media-servers">Discovered Media Servers</b>
					<p>
						Discovery suggests a patch only. The desk is not changed until you
						choose a patch action.
					</p>
				</div>
				<Button disabled={discoveryBusy} onClick={() => void refreshDiscovery()}>
					{discoveryBusy ? "Discovering…" : "Refresh discovery"}
				</Button>
			</header>
			{discoveryError && <p role="alert">{discoveryError}</p>}
			{!discoveryBusy && discovery?.servers.length === 0 && (
				<p>No ToskLight Pixel Media servers were found. Manual patching remains available.</p>
			)}
			{discovery?.servers.flatMap((candidate) =>
				candidate.outputs.length ? (
					candidate.outputs.map((output) => {
						const key = `${candidate.key}:${output.id}`;
						const patched = matchingDiscoveredFixture(
							mediaFixtures,
							candidate,
							output,
						);
						return (
							<article className="media-server-card" key={key}>
								<header>
									<div>
										<b>{candidate.name} · {output.name}</b>
										<small>{candidate.host} · {candidate.status}</small>
									</div>
									<strong>{patched ? "Patched" : "Not patched"}</strong>
								</header>
								<p>
									Suggested DMX {output.universe}.{output.startAddress} · {output.personality}
								</p>
								{output.dmxPendingRestart && (
									<p role="status">The Media Server has a DMX change pending restart.</p>
								)}
								<div className="media-actions">
									<Button
										disabled={patchBusy === key}
										onClick={() => void patchDiscoveredServer({
											candidate,
											output,
											patched,
											universe: output.universe,
											address: output.startAddress,
											changeRemote: false,
											patch,
											library: library?.fixtureLibrary ?? [],
											server,
											key,
											setBusy: setPatchBusy,
											setMessage: setPatchMessage,
											onRemoteUpdated: updateDiscoveredOutput,
										})}
									>
										Patch suggested
									</Button>
									<Button
										disabled={patchBusy === key}
										onClick={() => {
											setAddressChoice(key);
											setAddressDraft({
												universe: output.universe,
												address: output.startAddress,
											});
										}}
									>
										Patch address
									</Button>
								</div>
								{addressChoice === key && (
									<FormLayout labelPlacement="top" columns={2}>
										<NumberField
											label="Universe"
											min="1"
											max="65535"
											value={addressDraft.universe}
											onChange={(event) => setAddressDraft((current) => ({ ...current, universe: Number(event.target.value) }))}
										/>
										<NumberField
											label="Address"
											min="1"
											max="512"
											value={addressDraft.address}
											onChange={(event) => setAddressDraft((current) => ({ ...current, address: Number(event.target.value) }))}
										/>
										<Button onClick={() => void patchDiscoveredServer({
											candidate,
											output,
											patched,
											universe: addressDraft.universe,
											address: addressDraft.address,
											changeRemote: true,
											patch,
											library: library?.fixtureLibrary ?? [],
											server,
											key,
											setBusy: setPatchBusy,
											setMessage: setPatchMessage,
											onRemoteUpdated: updateDiscoveredOutput,
										})}>Confirm patch address</Button>
									</FormLayout>
								)}
								{patchMessage[key] && <p role="status">{patchMessage[key]}</p>}
							</article>
						);
					})
				) : (
					<article className="media-server-card" key={candidate.key}>
						<header><b>{candidate.name}</b><strong>Unavailable</strong></header>
						<p role="alert">{candidate.error ?? "No Media Server outputs are available."}</p>
					</article>
				),
			)}
			</section>
			<p>
				CITP endpoints belong to the physical master fixture. Every logical
				media layer inherits the same endpoint.
			</p>
			{!mediaFixtures.length && (
				<p>No patched devices expose media capabilities.</p>
			)}
			{patch.error && <p role="alert">{patch.error}</p>}
			{mediaFixtures.map((fixture) => (
				<MediaServerController
					key={fixture.fixture_id}
					fixture={fixture}
					status={matchingStatus(server?.mediaServers ?? [], fixture)}
					draft={drafts[fixture.fixture_id] ?? fixtureDraft(fixture)}
					preview={server?.mediaPreviewUrls[fixture.fixture_id]}
					busy={busy === fixture.fixture_id}
					live={live.has(fixture.fixture_id)}
					setDraft={(draft) =>
						setDrafts((current) => ({
							...current,
							[fixture.fixture_id]: draft,
						}))
					}
					save={async (draft) => {
						setBusy(fixture.fixture_id);
						try {
							await patch.updateFixture(fixture.fixture_id, {
								direct_control: draft.ip.trim()
									? {
											protocol: "citp",
											ip_address: draft.ip.trim(),
											port: draft.port,
										}
									: null,
							});
						} finally {
							setBusy(null);
						}
					}}
					toggleLive={() =>
						toggleLivePreview(
							server,
							fixture.fixture_id,
							live,
							setLive,
							setBusy,
						)
					}
					refreshThumbnails={() =>
						refreshThumbnails(server, fixture.fixture_id, setBusy)
					}
				/>
			))}
		</div>
	);
}

function matchingDiscoveredFixture(
	fixtures: readonly PatchedFixture[],
	server: DiscoveredMediaServer,
	output: DiscoveredMediaOutput,
): PatchedFixture | undefined {
	return fixtures.find((fixture) =>
		fixture.direct_control?.ip_address === server.host &&
		(fixture.internal_bindings?.output === output.id ||
			(fixture.universe === output.universe && fixture.address === output.startAddress)),
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
	library: readonly import("../../api/types").FixtureDefinition[];
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
	const mode =
		input.output.personality === "eight-layers" ? "8 layers" : "2 layers";
	const definition = input.library.find(
		(candidate) =>
			candidate.manufacturer === "ToskLight" &&
			candidate.name === "Media Server" &&
			candidate.mode === mode,
	);
	if (!definition) {
		setMessage(
			`The ToskLight Media Server ${mode} fixture profile is unavailable.`,
		);
		return;
	}
	input.setBusy(input.key);
	setMessage("Validating desk patch…");
	const original = input.patched;
	const fixture = original
		? changedPatchFixtureCandidate(original, {
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

function MediaServerController({
	fixture,
	status,
	draft,
	preview,
	busy,
	live,
	setDraft,
	save,
	toggleLive,
	refreshThumbnails,
}: {
	fixture: PatchedFixture;
	status?: MediaServerFixture;
	draft: Draft;
	preview?: string;
	busy: boolean;
	live: boolean;
	setDraft: (draft: Draft) => void;
	save: (draft: Draft) => Promise<void>;
	toggleLive: () => Promise<void>;
	refreshThumbnails: () => Promise<void>;
}) {
	const name = `${fixture.definition.manufacturer} ${fixture.definition.model}`;
	const supportsCitp =
		fixture.definition.direct_control_protocols?.includes("citp") ??
		Boolean(fixture.direct_control);
	const statusText = mediaStatusText(fixture, status, supportsCitp);
	return (
		<article className="media-server-card">
			<header>
				<b>{name}</b>
				<span className={status?.status.online ? "online" : "offline"}>
					{statusText}
				</span>
			</header>
			<FormLayout
				className="media-endpoint-form"
				labelPlacement="top"
				columns={2}
			>
				<TextField
					label="IP address"
					disabled={!supportsCitp}
					aria-label={`${name} CITP IP address`}
					value={draft.ip}
					placeholder="192.168.1.50"
					onChange={(event) => setDraft({ ...draft, ip: event.target.value })}
				/>
				<NumberField
					label="Port"
					disabled={!supportsCitp}
					aria-label={`${name} CITP port`}
					min="1"
					max="65535"
					value={draft.port}
					onChange={(event) =>
						setDraft({ ...draft, port: Number(event.target.value) })
					}
				/>
				<Button
					disabled={!supportsCitp || busy}
					onClick={() => void save(draft)}
				>
					{draft.ip.trim() ? "Save endpoint" : "Disable CITP"}
				</Button>
			</FormLayout>
			{fixture.direct_control && (
				<div className="media-actions">
					<Button
						className={live ? "active" : ""}
						disabled={busy}
						onClick={() => void toggleLive()}
					>
						{live ? "Stop live preview" : "Start live preview"}
					</Button>
					<Button disabled={busy} onClick={() => void refreshThumbnails()}>
						Refresh thumbnails 1–16
					</Button>
				</div>
			)}
			{preview ? (
				<img
					className="media-preview"
					src={preview}
					alt={`${name} live CITP output preview`}
				/>
			) : (
				<div className="media-preview media-preview-empty">
					{status?.status.last_error ? (
						<>
							<b>Preview unavailable</b>
							<small>{status.status.last_error}</small>
						</>
					) : (
						"No cached preview"
					)}
				</div>
			)}
			<small>
				{fixture.logical_heads.length} logical layers ·{" "}
				{status?.status.last_success
					? `Last response ${new Date(status.status.last_success).toLocaleString()}`
					: "No successful response yet"}
			</small>
		</article>
	);
}

function isMediaFixture(fixture: PatchedFixture): boolean {
	return (
		Boolean(fixture.direct_control) ||
		Boolean(fixture.definition.direct_control_protocols?.length) ||
		(fixture.definition.heads ?? []).some((head) =>
			head.parameters.some((parameter) =>
				parameter.attribute.startsWith("media."),
			),
		)
	);
}

function fixtureDraft(fixture: PatchedFixture): Draft {
	return {
		ip: fixture.direct_control?.ip_address ?? "",
		port: fixture.direct_control?.port ?? 4809,
	};
}

function fixtureDraftEntry(fixture: PatchedFixture): [string, Draft] {
	return [fixture.fixture_id, fixtureDraft(fixture)];
}

function matchingStatus(
	statuses: readonly MediaServerFixture[],
	fixture: PatchedFixture,
): MediaServerFixture | undefined {
	const endpoint = fixture.direct_control;
	if (!endpoint) return undefined;
	return statuses.find(
		(status) =>
			status.fixture_id === fixture.fixture_id &&
			status.endpoint?.protocol === endpoint.protocol &&
			status.endpoint.ip_address === endpoint.ip_address &&
			status.endpoint.port === endpoint.port,
	);
}

function mediaStatusText(
	fixture: PatchedFixture,
	status: MediaServerFixture | undefined,
	supportsCitp: boolean,
): string {
	if (status?.status.online) return "● Online";
	if (fixture.direct_control) return "● Offline";
	return supportsCitp ? "Not configured" : "Profile has no CITP capability";
}

async function toggleLivePreview(
	server: MediaServersState | null,
	fixtureId: string,
	live: ReadonlySet<string>,
	setLive: Dispatch<SetStateAction<Set<string>>>,
	setBusy: Dispatch<SetStateAction<string | null>>,
): Promise<void> {
	if (live.has(fixtureId)) {
		setLive((current) => {
			const next = new Set(current);
			next.delete(fixtureId);
			return next;
		});
		return;
	}
	setBusy(fixtureId);
	try {
		if (await refreshAdvertisedPreview(server, fixtureId))
			setLive((current) => new Set(current).add(fixtureId));
	} finally {
		setBusy(null);
	}
}

async function refreshThumbnails(
	server: MediaServersState | null,
	fixtureId: string,
	setBusy: Dispatch<SetStateAction<string | null>>,
): Promise<void> {
	setBusy(fixtureId);
	try {
		const inspection = await server?.inspectMediaServer(fixtureId);
		const firstFolder = inspection?.folders[0];
		if (!firstFolder) return;
		const elements = (inspection?.files ?? [])
			.filter((file) => file.folder_id === firstFolder.id)
			.slice(0, 16)
			.map((file) => file.id);
		if (elements.length)
			await server?.refreshMediaThumbnails(fixtureId, firstFolder.id, elements);
	} finally {
		setBusy(null);
	}
}

async function refreshAdvertisedPreview(
	server: MediaServersState | null,
	fixtureId: string,
): Promise<boolean> {
	const inspection = await server?.inspectMediaServer(fixtureId);
	const source = inspection?.preview_sources[0];
	return source
		? (await server?.refreshMediaPreview(fixtureId, source.id)) === true
		: false;
}
