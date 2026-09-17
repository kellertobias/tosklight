import { useEffect, useMemo, useRef, useState } from "react";
import type { PatchedFixture } from "../../api/types";
import {
	type MediaServersState,
	useMediaServers,
} from "../../features/mediaServers/MediaServersContext";
import { usePatch, usePatchView } from "../../features/patch/PatchContext";
import { DiscoveredMediaOutputCard } from "./DiscoveredMediaOutputCard";
import {
	type MediaServerRowView,
	MediaServerTable,
	type RowMessage,
} from "./MediaServerTable";
import {
	fixtureDraft,
	isMediaFixture,
	type MediaServerDraft,
	matchingStatus,
	needsFirstCheck,
	networkState,
} from "./mediaServerRowModel";
import {
	type MediaServerDiscoveryController,
	useMediaServerDiscovery,
} from "./useMediaServerDiscovery";

type RowBusy = MediaServerRowView["busy"];
type Server = MediaServersState | null;

/**
 * Show Patch › Media Servers: the patched servers as a configuration table, and what discovery
 * found on the network. Refresh Discovery lives in the window title; the owner passes its
 * controller in, and a standalone mount discovers on its own.
 */
export function MediaServerSetup({
	active = true,
	discovery: shared,
}: {
	active?: boolean;
	discovery?: MediaServerDiscoveryController;
}) {
	const server = useMediaServers();
	const patch = usePatch();
	usePatchView(active);
	const own = useMediaServerDiscovery(active && !shared);
	const discovery = shared ?? own;
	const mediaFixtures = useMemo(
		() =>
			patch.fixtures
				.filter(isMediaFixture)
				.sort(
					(a, b) =>
						(a.fixture_number ?? Number.MAX_SAFE_INTEGER) -
						(b.fixture_number ?? Number.MAX_SAFE_INTEGER),
				),
		[patch.fixtures],
	);
	const rows = useMediaServerRows(server, mediaFixtures, active, discovery);
	if (!active || patch.status !== "ready")
		return <p>Patch authority loading…</p>;
	return (
		<div className="media-server-setup">
			<section
				className="media-server-patched"
				aria-labelledby="patched-media-servers"
			>
				<b id="patched-media-servers">Patched Media Servers</b>
				<p>
					CITP endpoints belong to the physical master fixture. Every logical
					media layer inherits the same endpoint.
				</p>
				{patch.error && <p role="alert">{patch.error}</p>}
				<MediaServerTable rows={rows} />
			</section>
			<DiscoveredMediaServers
				controller={discovery}
				fixtures={mediaFixtures}
				server={server}
			/>
		</div>
	);
}

function DiscoveredMediaServers({
	controller,
	fixtures,
	server,
}: {
	controller: MediaServerDiscoveryController;
	fixtures: readonly PatchedFixture[];
	server: Server;
}) {
	const { discovery, busy, error } = controller;
	return (
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
			</header>
			{busy && <p role="status">Discovering Media Servers…</p>}
			{error && <p role="alert">{error}</p>}
			{!busy && discovery?.servers.length === 0 && (
				<p>
					No ToskLight Pixel Media servers were found. Check that the Media
					Server is running on this network, then Refresh Discovery. Manual
					patching remains available.
				</p>
			)}
			{discovery?.servers.flatMap((candidate) =>
				candidate.outputs.length ? (
					candidate.outputs.map((output) => (
						<DiscoveredMediaOutputCard
							key={`${candidate.key}:${output.id}`}
							candidate={candidate}
							output={output}
							fixtures={fixtures}
							server={server}
							onRemoteUpdated={controller.updateOutput}
						/>
					))
				) : (
					<article className="media-server-card" key={candidate.key}>
						<header>
							<b>{candidate.name}</b>
							<strong>Unavailable</strong>
						</header>
						<p role="alert">
							{candidate.error ??
								"No Media Server outputs are available. Refresh Discovery."}
						</p>
					</article>
				),
			)}
		</section>
	);
}

/**
 * Row state for the table. Every busy flag and message is per fixture, so working on one server
 * never blocks or clears another.
 */
function useMediaServerRows(
	server: Server,
	fixtures: readonly PatchedFixture[],
	active: boolean,
	discovery: MediaServerDiscoveryController,
): MediaServerRowView[] {
	const patch = usePatch();
	const [drafts, setDrafts] = useState<Record<string, MediaServerDraft>>({});
	const [busy, setBusy] = useState<Record<string, RowBusy>>({});
	const [messages, setMessages] = useState<Record<string, RowMessage>>({});
	const [live, setLive] = useState<Set<string>>(() => new Set());
	const checked = useRef(new Set<string>());
	const statuses = server?.mediaServers ?? [];
	useEffect(() => {
		setDrafts(
			Object.fromEntries(
				fixtures.map((fixture) => [fixture.fixture_id, fixtureDraft(fixture)]),
			),
		);
	}, [fixtures]);
	const track = async (
		fixtureId: string,
		kind: RowBusy,
		work: () => Promise<RowMessage | null>,
	) => {
		setBusy((current) => ({ ...current, [fixtureId]: kind }));
		// A background check keeps the row's last outcome in view.
		if (kind !== "checking") setMessages(({ [fixtureId]: _, ...rest }) => rest);
		try {
			const message = await work();
			if (message)
				setMessages((current) => ({ ...current, [fixtureId]: message }));
		} finally {
			setBusy(({ [fixtureId]: _, ...rest }) => rest);
		}
	};
	useFirstChecks(server, fixtures, active, busy, checked, track);
	useLivePreview(server, live, active);
	return fixtures.map((fixture) => {
		const id = fixture.fixture_id;
		const draft = drafts[id] ?? fixtureDraft(fixture);
		return {
			fixture,
			status: matchingStatus(statuses, fixture),
			draft,
			network: networkState(fixture, discovery.discovery, discovery.error),
			preview: live.has(id) ? server?.mediaPreviewUrls[id] : undefined,
			busy: busy[id] ?? null,
			live: live.has(id),
			message: messages[id],
			actions: {
				setDraft: (next) =>
					setDrafts((current) => ({ ...current, [id]: next })),
				apply: () =>
					void track(id, "saving", () => applyDraft(patch, fixture, draft)),
				refreshThumbnails: () =>
					void track(id, "thumbnails", () => refreshThumbnails(server, id)),
				toggleLive: () =>
					void track(id, "preview", () =>
						toggleLive(server, id, live, setLive),
					),
			},
		};
	});
}

type Track = (
	fixtureId: string,
	kind: RowBusy,
	work: () => Promise<RowMessage | null>,
) => Promise<void>;

/** Ask each configured server once for its state when the desk has none to show yet. */
function useFirstChecks(
	server: Server,
	fixtures: readonly PatchedFixture[],
	active: boolean,
	busy: Record<string, RowBusy>,
	checked: { current: Set<string> },
	track: Track,
) {
	const statuses = server?.mediaServers;
	useEffect(() => {
		if (!active || !server) return;
		for (const fixture of fixtures) {
			const endpoint = fixture.direct_control;
			const key = `${fixture.fixture_id}@${endpoint?.ip_address}:${endpoint?.port}`;
			// A row still applying its endpoint is checked once the desk has stored it.
			if (checked.current.has(key) || busy[fixture.fixture_id]) continue;
			if (!needsFirstCheck(fixture, matchingStatus(statuses ?? [], fixture)))
				continue;
			checked.current.add(key);
			void track(fixture.fixture_id, "checking", async () => {
				await server.inspectMediaServer(fixture.fixture_id).catch(() => null);
				return null;
			});
		}
		// `track` is recreated every render; the checked set keeps this to once per endpoint.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [active, server, fixtures, statuses, busy]);
}

function useLivePreview(
	server: Server,
	live: ReadonlySet<string>,
	active: boolean,
) {
	useEffect(() => {
		if (!live.size || !active) return;
		const timer = window.setInterval(() => {
			for (const fixtureId of live)
				void refreshAdvertisedPreview(server, fixtureId);
		}, 1_000);
		return () => window.clearInterval(timer);
	}, [active, live, server]);
}

async function applyDraft(
	patch: ReturnType<typeof usePatch>,
	fixture: PatchedFixture,
	draft: MediaServerDraft,
): Promise<RowMessage> {
	const endpoint =
		draft.protocol === "off"
			? null
			: {
					protocol: draft.protocol,
					ip_address: draft.ip.trim(),
					port: draft.port,
				};
	const saved = await patch.updateFixture(fixture.fixture_id, {
		direct_control: endpoint,
	});
	if (!saved)
		return {
			tone: "alert",
			text: `${patch.error ?? "The desk refused this endpoint."} Check the address and retry.`,
		};
	return {
		tone: "status",
		text: endpoint
			? `Now using ${endpoint.ip_address}:${endpoint.port}.`
			: "Network control is off for this server.",
	};
}

/** Every advertised library folder, bounded so one server cannot push the others out of the cache. */
const THUMBNAILS_PER_SERVER = 128;

async function refreshThumbnails(
	server: Server,
	fixtureId: string,
): Promise<RowMessage> {
	if (!server)
		return {
			tone: "alert",
			text: "The desk connection is unavailable. Reconnect, then retry.",
		};
	let inspection: Awaited<ReturnType<MediaServersState["inspectMediaServer"]>>;
	try {
		inspection = await server.inspectMediaServer(fixtureId);
	} catch {
		// The row's status already carries the server's reason.
		return { tone: "alert", text: "Thumbnails were not refreshed." };
	}
	let remaining = THUMBNAILS_PER_SERVER;
	let refreshed = 0;
	for (const folder of inspection.folders) {
		const elements = inspection.files
			.filter((file) => file.folder_id === folder.id)
			.slice(0, remaining)
			.map((file) => file.id);
		if (!elements.length) continue;
		if (!(await server.refreshMediaThumbnails(fixtureId, folder.id, elements)))
			return {
				tone: "alert",
				text: `Refreshed ${refreshed} thumbnails, then the server stopped answering. Refresh Thumbnails to retry.`,
			};
		refreshed += elements.length;
		remaining -= elements.length;
		if (remaining <= 0) break;
	}
	return {
		tone: "status",
		text: refreshed
			? `Refreshed ${refreshed} thumbnails.`
			: "The server advertises no media files to preview.",
	};
}

async function toggleLive(
	server: Server,
	fixtureId: string,
	live: ReadonlySet<string>,
	setLive: (update: (current: Set<string>) => Set<string>) => void,
): Promise<RowMessage | null> {
	if (live.has(fixtureId)) {
		setLive((current) => {
			const next = new Set(current);
			next.delete(fixtureId);
			return next;
		});
		return null;
	}
	if (await refreshAdvertisedPreview(server, fixtureId)) {
		setLive((current) => new Set(current).add(fixtureId));
		return null;
	}
	return {
		tone: "alert",
		text: "The server sent no preview. Check that it advertises an output preview, then retry.",
	};
}

async function refreshAdvertisedPreview(
	server: Server,
	fixtureId: string,
): Promise<boolean> {
	const inspection = await server
		?.inspectMediaServer(fixtureId)
		.catch(() => null);
	const source = inspection?.preview_sources[0];
	return source
		? (await server?.refreshMediaPreview(fixtureId, source.id)) === true
		: false;
}
