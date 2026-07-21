import {
	type Dispatch,
	type SetStateAction,
	useEffect,
	useMemo,
	useState,
} from "react";
import { useServer } from "../../api/ServerContext";
import type { MediaServerFixture, PatchedFixture } from "../../api/types";
import { usePatch, usePatchView } from "../../features/patch/PatchContext";
import { Button, FormLayout, NumberField, TextField } from "../common";

type Draft = { ip: string; port: number };

export function MediaServerSetup({ active = true }: { active?: boolean }) {
	const server = useServer();
	const patch = usePatch();
	usePatchView(active);
	const [drafts, setDrafts] = useState<Record<string, Draft>>({});
	const [busy, setBusy] = useState<string | null>(null);
	const [live, setLive] = useState<Set<string>>(() => new Set());
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
			for (const fixtureId of live) void server.refreshMediaPreview(fixtureId);
		}, 1_000);
		return () => window.clearInterval(timer);
	}, [active, live, server.refreshMediaPreview]);
	if (!active || patch.status !== "ready")
		return <p>Patch authority loading…</p>;
	if (!mediaFixtures.length)
		return <p>No patched devices expose media capabilities.</p>;
	return (
		<div className="media-server-setup">
			<p>
				CITP endpoints belong to the physical master fixture. Every logical
				media layer inherits the same endpoint.
			</p>
			{patch.error && <p role="alert">{patch.error}</p>}
			{mediaFixtures.map((fixture) => (
				<MediaServerController
					key={fixture.fixture_id}
					fixture={fixture}
					status={matchingStatus(server.mediaServers, fixture)}
					draft={drafts[fixture.fixture_id] ?? fixtureDraft(fixture)}
					preview={server.mediaPreviewUrls[fixture.fixture_id]}
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
		port: fixture.direct_control?.port ?? 4811,
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
			status.endpoint.protocol === endpoint.protocol &&
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
	server: ReturnType<typeof useServer>,
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
		if (await server.refreshMediaPreview(fixtureId))
			setLive((current) => new Set(current).add(fixtureId));
	} finally {
		setBusy(null);
	}
}

async function refreshThumbnails(
	server: ReturnType<typeof useServer>,
	fixtureId: string,
	setBusy: Dispatch<SetStateAction<string | null>>,
): Promise<void> {
	setBusy(fixtureId);
	try {
		await server.refreshMediaThumbnails(
			fixtureId,
			Array.from({ length: 16 }, (_, index) => index),
		);
	} finally {
		setBusy(null);
	}
}
