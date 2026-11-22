import { Button, NumberField, SelectField } from "@tosklight/ui/controls";
import { useState } from "react";
import { addressLabel, folderLabel, itemLabel } from "../../entities/catalog";
import type { AutomationStatus, AutomationView } from "../../shared/api/automation";
import type { CatalogItemView, CatalogView } from "../../shared/api/generated/media-wire";
import { sendTelemetryAction } from "../../shared/api/telemetry";

export function findAsset(catalog: CatalogView | undefined, id: string | null) {
	for (const folder of catalog?.folders ?? []) {
		const item = folder.items.find((candidate) => candidate.id === id);
		if (item) return { folder, item };
	}
	return undefined;
}

export function assetName(catalog: CatalogView | undefined, id: string | null) {
	const found = findAsset(catalog, id);
	return found ? `${addressLabel(found.folder.folder, found.item.file)} · ${found.item.name}` : id ? "Missing or parked media" : "Choose media";
}

export function AssetPicker({ catalog, assetId, onChange, label = "Clip", allowEmpty = false }: {
	catalog: CatalogView | undefined; assetId: string | null;
	onChange: (id: string | null, item: CatalogItemView | undefined) => void;
	label?: string; allowEmpty?: boolean;
}) {
	const found = findAsset(catalog, assetId);
	const [folderNumber, setFolderNumber] = useState(found?.folder.folder ?? catalog?.folders.find((folder) => folder.items.length > 0)?.folder ?? 1);
	const folder = catalog?.folders.find((candidate) => candidate.folder === folderNumber);
	return <div className="pixel-asset-picker">
		<SelectField label={`${label} folder`} ariaLabel={`${label} folder`} value={String(folderNumber)}
			options={(catalog?.folders ?? []).filter((candidate) => candidate.folder < 200).map((candidate) => ({ value: String(candidate.folder), label: folderLabel(candidate) }))}
			onChange={(value) => setFolderNumber(Number(value))} />
		<SelectField label={label} ariaLabel={label} value={found?.folder.folder === folderNumber ? assetId ?? "" : ""}
			options={[
				{ value: "", label: allowEmpty ? "None" : "Choose media", disabled: !allowEmpty },
				...(!found && assetId ? [{ value: assetId, label: "Missing or parked media", disabled: true }] : []),
				...(folder?.items ?? []).map((item) => ({ value: item.id, label: `${itemLabel(item)}${item.enabled === false ? " · Disabled" : ""}`, disabled: item.enabled === false })),
			]}
			onChange={(value) => onChange(value || null, folder?.items.find((item) => item.id === value))} />
		{assetId && (!found || found.item.enabled === false) && <p className="media-settings-note" role="alert">{!found ? "This media is missing or parked. Choose another clip." : "This media is disabled in the library."}</p>}
	</div>;
}

export function SecondsField({ label, millis, onChange, min = 0 }: { label: string; millis: number; onChange: (millis: number) => void; min?: number }) {
	return <NumberField label={label} value={millis / 1000} min={min} step={0.1} onChange={(event) => {
		const value = Number(event.target.value);
		if (Number.isFinite(value) && value >= min) onChange(Math.round(value * 1000));
	}} />;
}

export function CalendarField({ label, value, type, onChange }: { label: string; value: string; type: "date" | "time" | "datetime-local"; onChange: (value: string) => void }) {
	return <label className="pixel-calendar-field"><span>{label}</span><input type={type} step={type === "date" ? undefined : 1} value={value} onChange={(event) => onChange(event.target.value)} /></label>;
}

const OWNERS: Record<AutomationStatus["owner"], string> = {
	disabled: "Scheduling off", dmx: "DMX controlling", manual: "Manual control", schedule: "Scheduled playlist", fallback: "Fallback playlist", idle: "Waiting for an event", error: "Playback needs attention",
};

export function AutomationNow({ status, snapshot, connected, onFailure }: { status: AutomationStatus | undefined; snapshot: AutomationView; connected: boolean; onFailure: (message: string) => void }) {
	if (!status) return <p className="media-settings-note">No scheduling state for this output.</p>;
	const last = status;
	const outcomes: Record<string, string> = { started: "Started", completed: "Completed", suppressed: "Skipped", missed: "Skipped", interrupted: "Interrupted", failed: "Failed" };
	return <section className="pixel-automation-now" aria-label="Current scheduling state">
		<div className="pixel-automation-now-heading"><h2>{OWNERS[status.owner] ?? "Scheduling state"}</h2><span>{connected ? "Live" : "Snapshot · Live connection unavailable"}</span></div>
		{status.playlistName && <p className="pixel-playing-name">{status.playlistName}</p>}
		{status.entryIndex !== null && <p>Clip {status.entryIndex + 1} · Play {status.play} · {status.phase}</p>}
		{status.owner === "dmx" && <p>Last DMX packet {((status.dmxAgeMillis ?? 0) / 1000).toFixed(1)} s ago. {status.resumeInMillis === null ? "Waiting for DMX source loss." : `Scheduling resumes in ${(status.resumeInMillis / 1000).toFixed(1)} s.`}</p>}
		{status.owner === "manual" && <p>Release playback takeover to allow automatic scheduling to resume.</p>}
		{status.error && <p role="alert">{status.error}</p>}
		{last.lastOccurrenceOutcome && <p className="media-settings-note">Last event{last.lastOccurrenceName ? ` · ${last.lastOccurrenceName}` : ""}: {outcomes[last.lastOccurrenceOutcome] ?? last.lastOccurrenceOutcome}</p>}
		{status.playlistId && <Button disabled={!connected || status.owner === "dmx" || status.owner === "manual"} onClick={() => {
			void sendTelemetryAction({ type: "automation-apply-now", outputId: status.outputId }).catch((error) => onFailure(error instanceof Error ? error.message : "Could not restart the playlist."));
		}}>Apply playlist changes now</Button>}
		<p className="media-settings-note">Computer time zone: {snapshot.timeZone}. Missed events are skipped.</p>
	</section>;
}
