import { Button, NumberField, SelectField, TextField } from "@tosklight/ui";
import type { MediaServerFixture, PatchedFixture } from "../../api/types";
import {
	CONNECTION_LABELS,
	connectionState,
	draftProblems,
	fixtureDraft,
	MEDIA_SERVER_TYPE_LABELS,
	type MediaServerDraft,
	type MediaServerProtocol,
	mediaServerType,
	offlineHint,
	protocolOptions,
	sameDraft,
	supportedProtocols,
} from "./mediaServerRowModel";

export type RowMessage = { tone: "status" | "alert"; text: string };

export type MediaServerRowActions = {
	setDraft: (draft: MediaServerDraft) => void;
	apply: () => void;
	checkConnection: () => void;
	refreshThumbnails: () => void;
	toggleLive: () => void;
};

export type MediaServerRowView = {
	fixture: PatchedFixture;
	status?: MediaServerFixture;
	draft: MediaServerDraft;
	network: string | null;
	preview?: string;
	busy: "saving" | "checking" | "thumbnails" | "preview" | null;
	live: boolean;
	message?: RowMessage;
	actions: MediaServerRowActions;
};

/** Every patched Media Server, one configurable row each. */
export function MediaServerTable({ rows }: { rows: MediaServerRowView[] }) {
	if (!rows.length)
		return (
			<p>
				No patched devices expose media capabilities. Patch a Media Server from
				Discovered Media Servers or the Fixtures view.
			</p>
		);
	return (
		<table className="media-server-table" aria-label="Patched Media Servers">
			<thead>
				<tr>
					<th scope="col">#</th>
					<th scope="col">Name</th>
					<th scope="col">Type</th>
					<th scope="col">Protocol</th>
					<th scope="col">IP address</th>
					<th scope="col">Port</th>
					<th scope="col">Status</th>
					<th scope="col">Actions</th>
				</tr>
			</thead>
			<tbody>
				{rows.map((row) => (
					<MediaServerRow key={row.fixture.fixture_id} row={row} />
				))}
			</tbody>
		</table>
	);
}

function rowName(fixture: PatchedFixture): string {
	return (
		fixture.name ||
		`${fixture.definition.manufacturer} ${fixture.definition.model}`
	);
}

function MediaServerRow({ row }: { row: MediaServerRowView }) {
	const { fixture, draft, actions } = row;
	const name = rowName(fixture);
	const type = mediaServerType(fixture, row.status);
	const editable = supportedProtocols(fixture).length > 0;
	const problems = draftProblems(draft);
	const valid = !problems.ip && !problems.port;
	const changed = !sameDraft(draft, fixtureDraft(fixture));
	const endpointOff = draft.protocol === "off";
	return (
		<>
			<tr data-fixture-id={fixture.fixture_id} data-server-type={type}>
				<td>{fixture.fixture_number ?? "—"}</td>
				<th scope="row">{name}</th>
				<td>{MEDIA_SERVER_TYPE_LABELS[type]}</td>
				<td>
					<SelectField<MediaServerProtocol>
						ariaLabel={`${name} protocol`}
						value={draft.protocol}
						disabled={!editable || row.busy === "saving"}
						options={protocolOptions(fixture)}
						onChange={(protocol) => actions.setDraft({ ...draft, protocol })}
					/>
				</td>
				<td>
					<TextField
						aria-label={`${name} IP address`}
						disabled={!editable || endpointOff}
						value={draft.ip}
						placeholder="192.168.1.50"
						error={problems.ip}
						onChange={(event) =>
							actions.setDraft({ ...draft, ip: event.target.value })
						}
					/>
				</td>
				<td>
					<NumberField
						aria-label={`${name} port`}
						disabled={!editable || endpointOff}
						min="1"
						max="65535"
						value={draft.port}
						error={problems.port}
						onChange={(event) =>
							actions.setDraft({ ...draft, port: Number(event.target.value) })
						}
					/>
				</td>
				<td>
					<MediaServerStatusCell row={row} />
				</td>
				<td>
					<div className="media-server-row-actions">
						<Button
							disabled={
								!editable || !changed || !valid || row.busy === "saving"
							}
							onClick={actions.apply}
						>
							{row.busy === "saving" ? "Applying…" : "Apply"}
						</Button>
						<Button
							disabled={!fixture.direct_control || row.busy !== null}
							onClick={actions.checkConnection}
						>
							{row.busy === "checking" ? "Checking…" : "Check connection"}
						</Button>
						<Button
							disabled={!fixture.direct_control || row.busy !== null}
							onClick={actions.refreshThumbnails}
						>
							{row.busy === "thumbnails" ? "Refreshing…" : "Refresh Thumbnails"}
						</Button>
						<Button
							className={row.live ? "active" : ""}
							disabled={!fixture.direct_control || row.busy !== null}
							onClick={actions.toggleLive}
						>
							{row.live ? "Stop live preview" : "Start live preview"}
						</Button>
					</div>
				</td>
			</tr>
			{row.preview ? (
				<tr className="media-server-preview-row">
					<td colSpan={8}>
						<img
							className="media-preview"
							src={row.preview}
							alt={`${name} live CITP output preview`}
						/>
					</td>
				</tr>
			) : null}
		</>
	);
}

function MediaServerStatusCell({ row }: { row: MediaServerRowView }) {
	const state = connectionState(
		row.fixture,
		row.status,
		row.busy === "checking",
	);
	const error = row.status?.status.last_error;
	const lastSuccess = row.status?.status.last_success;
	return (
		<div className="media-server-status" data-state={state}>
			<b className={state === "connected" ? "online" : "offline"}>
				● {CONNECTION_LABELS[state]}
			</b>
			{row.network ? <small>{row.network}</small> : null}
			{state === "offline" && error ? (
				<small role="alert">{offlineHint(error)}</small>
			) : null}
			{state === "connected" && lastSuccess ? (
				<small>
					Last response {new Date(lastSuccess).toLocaleTimeString()}
				</small>
			) : null}
			{row.message ? (
				<small role={row.message.tone}>{row.message.text}</small>
			) : null}
		</div>
	);
}
