import { useEffect, useRef } from "react";
import { request } from "./client";
import { requestId } from "./editing";
import { type Resource, useResource, writeResource } from "./resource";
import { useTelemetry } from "./telemetry";

import type {
    AutomationConfigurationView, AutomationOutputView, AutomationStatus as WireAutomationStatus,
    AutomationView as WireAutomationView, AuxiliaryLayerView, LayerPresetView,
    PlaylistEntryView, PlaylistView, ScheduleEventView, ScheduleTriggerView, UpcomingOccurrenceView,
} from "./generated/media-wire";

// Read models come from the generated server contract. New auxiliary drafts may omit fields
// that the ordinary layer preset fills with its documented defaults.
export type LayerPreset = Partial<LayerPresetView>;
export type AuxiliaryLayer = Omit<AuxiliaryLayerView, "mode" | "controls"> & {
    mode: "unchanged" | "off" | "fixed";
    controls: LayerPreset;
};
export type PlaylistEntry = PlaylistEntryView;
export type Playlist = Omit<PlaylistView, "auxiliaryLayers"> & { auxiliaryLayers: AuxiliaryLayer[] };
export type ScheduleTrigger = ScheduleTriggerView;
export type ScheduleEvent = ScheduleEventView;
export type AutomationOutput = AutomationOutputView;
export type AutomationConfiguration = Omit<AutomationConfigurationView, "playlists"> & { playlists: Playlist[] };
export type AutomationStatus = WireAutomationStatus;
export type ScheduleOccurrence = UpcomingOccurrenceView;
export type AutomationView = Omit<WireAutomationView, "configuration" | "status"> & {
    configuration: AutomationConfiguration;
    status: AutomationStatus[];
};

const BASE = "/automation";
const KEY = "automation";
/** Reconcile a confirmed configuration edit before selecting its newly created object. */
export function acceptAutomationSnapshot(snapshot: AutomationView): void {
	writeResource(KEY, snapshot);
}
/** Include every locally changed field, including a correction whose earlier save was refused. */
export function changedFields<T extends object>(draft: T, saved: T): Partial<T> {
	return Object.fromEntries(Object.keys(draft).filter((key) => key !== "id" && JSON.stringify(draft[key as keyof T]) !== JSON.stringify(saved[key as keyof T])).map((key) => [key, draft[key as keyof T]])) as Partial<T>;
}
function post(path: string, body: object): Promise<AutomationView> {
	return request(`${BASE}${path}`, { method: "POST", body: JSON.stringify({ requestId: requestId(), ...body }) });
}
export const automationApi = {
	read: (): Promise<AutomationView> => request(BASE),
	createPlaylist: (playlist: Omit<Playlist, "id">) => post("/playlists/create", playlist),
	updatePlaylist: (id: string, change: Partial<Omit<Playlist, "id">>) => post(`/playlists/${encodeURIComponent(id)}/update`, change),
	deletePlaylist: (id: string) => post(`/playlists/${encodeURIComponent(id)}/delete`, {}),
	createEvent: (event: Omit<ScheduleEvent, "id">) => post("/events/create", event),
	updateEvent: (id: string, change: Partial<Omit<ScheduleEvent, "id">>) => post(`/events/${encodeURIComponent(id)}/update`, change),
	deleteEvent: (id: string) => post(`/events/${encodeURIComponent(id)}/delete`, {}),
	updateOutput: (id: string, change: Partial<Pick<AutomationOutput, "fallbackPlaylistId" | "dmxSilenceSeconds">>) => post(`/outputs/${encodeURIComponent(id)}/update`, change),
};

export function useAutomation(): Resource<AutomationView> {
	const resource = useResource(KEY, automationApi.read);
	const telemetry = useTelemetry();
	const lastRevision = useRef<number | undefined>(undefined);
	const frame = telemetry.frame as (typeof telemetry.frame & { automationRevision?: number });
	useEffect(() => {
		const revision = frame?.automationRevision;
		if (revision === undefined || revision === lastRevision.current) return;
		lastRevision.current = revision;
		resource.reload();
	}, [frame?.automationRevision, resource.reload]);
	return resource;
}

/** Runtime ownership arrives over telemetry; configuration is read only after edits. */
export function useAutomationStatus(snapshot: AutomationView | undefined) {
	const telemetry = useTelemetry();
	const live = telemetry.frame as (typeof telemetry.frame & { automation?: AutomationStatus[] });
	return { status: live?.automation ?? snapshot?.status ?? [], connected: telemetry.connected };
}
