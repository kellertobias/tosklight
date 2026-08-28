import type {
	PsnBindingProjection,
	PsnSnapshot as WirePsnSnapshot,
	PsnUpdateOutcome as WirePsnUpdateOutcome,
	PsnUpdateRequest,
	PsnZoneProjection,
} from "../generated/light-wire";
import { type ClientTransport, jsonRequest } from "./transport";

/** Metres in the show's own stage space, in the order x, y, z. */
export type StagePoint = [number, number, number];

export interface PsnBinding {
	id: string;
	trackerId: number;
	pointFixtureId: string;
	enabled: boolean;
}

export interface PsnZone {
	id: string;
	name: string;
	minMetres: StagePoint;
	maxMetres: StagePoint;
	/** Empty means every tracker counts. */
	trackerIds: number[];
	enterMacroId: string | null;
	leaveMacroId: string | null;
	dwellMillis: number;
}

export interface PsnCalibration {
	offsetMetres: StagePoint;
	rotationDegrees: number;
	scale: number;
}

export interface PsnConfiguration {
	enabled: boolean;
	group: string;
	port: number;
	interface: string | null;
	staleAfterMillis: number;
	calibration: PsnCalibration;
	bindings: PsnBinding[];
	zones: PsnZone[];
}

export type PsnHealth =
	| { state: "silent" }
	| { state: "receiving" }
	| { state: "stale"; silentForMillis: number };

export interface PsnTracker {
	trackerId: number;
	/** What the sender calls it, once its info packet has said. */
	name: string | null;
	positionMetres: StagePoint | null;
	ageMillis: number;
	stale: boolean;
	source: string;
}

export interface PsnPlacement {
	bindingId: string;
	pointFixtureId: string;
	positionMetres: StagePoint;
	outOfReach: boolean;
}

export interface PsnStatus {
	enabled: boolean;
	listeningOn: string | null;
	health: PsnHealth | null;
	systemNames: string[];
	trackers: PsnTracker[];
	placements: PsnPlacement[];
	occupiedZoneIds: string[];
	frames: number;
	ignoredDatagrams: number;
	error: string | null;
}

export interface PsnPoint {
	fixtureId: string;
	name: string;
	fixtureNumber: number | null;
}

export interface PsnMacro {
	id: string;
	number: number;
	name: string;
}

export interface PsnSnapshot {
	revision: number;
	configuration: PsnConfiguration;
	status: PsnStatus;
	/** Every 3D Point in the show, as something to bind a tracker to. */
	points: PsnPoint[];
	/** Every Macro in the show, for a zone's enter and leave. */
	macros: PsnMacro[];
}

/** One edit, carrying only what the operator changed. */
export interface PsnEdit {
	enabled?: boolean;
	group?: string;
	port?: number;
	/** `null` clears the interface; leaving it out keeps whatever is stored. */
	interface?: string | null;
	staleAfterMillis?: number;
	calibration?: PsnCalibration;
	bindings?: PsnBinding[];
	zones?: PsnZone[];
}

export class PsnApiClient {
	constructor(private readonly transport: ClientTransport) {}

	snapshot(): Promise<PsnSnapshot> {
		return this.transport
			.request<WirePsnSnapshot>("/api/v2/psn")
			.then(mapSnapshot);
	}

	update(edit: PsnEdit): Promise<PsnSnapshot["configuration"]> {
		const request: PsnUpdateRequest = {
			request_id: crypto.randomUUID(),
			...(edit.enabled === undefined ? {} : { enabled: edit.enabled }),
			...(edit.group === undefined ? {} : { group: edit.group }),
			...(edit.port === undefined ? {} : { port: edit.port }),
			...(edit.interface === undefined ? {} : { interface: edit.interface }),
			...(edit.staleAfterMillis === undefined
				? {}
				: { stale_after_millis: edit.staleAfterMillis }),
			...(edit.calibration === undefined
				? {}
				: {
						calibration: {
							offset_metres: edit.calibration.offsetMetres,
							rotation_degrees: edit.calibration.rotationDegrees,
							scale: edit.calibration.scale,
						},
					}),
			...(edit.bindings === undefined
				? {}
				: { bindings: edit.bindings.map(wireBinding) }),
			...(edit.zones === undefined ? {} : { zones: edit.zones.map(wireZone) }),
		};
		return this.transport
			.request<WirePsnUpdateOutcome>(
				"/api/v2/psn/update",
				jsonRequest("POST", request),
			)
			.then((outcome) => mapConfiguration(outcome.configuration));
	}
}

function wireBinding(binding: PsnBinding): PsnBindingProjection {
	return {
		id: binding.id,
		tracker_id: binding.trackerId,
		point_fixture_id: binding.pointFixtureId,
		enabled: binding.enabled,
	};
}

function wireZone(zone: PsnZone): PsnZoneProjection {
	return {
		id: zone.id,
		name: zone.name,
		min_metres: zone.minMetres,
		max_metres: zone.maxMetres,
		tracker_ids: [...zone.trackerIds],
		enter_macro_id: zone.enterMacroId,
		leave_macro_id: zone.leaveMacroId,
		dwell_millis: zone.dwellMillis,
	};
}

function mapSnapshot(snapshot: WirePsnSnapshot): PsnSnapshot {
	return {
		revision: snapshot.revision,
		configuration: mapConfiguration(snapshot.configuration),
		status: {
			enabled: snapshot.status.enabled,
			listeningOn: snapshot.status.listening_on ?? null,
			health: mapHealth(snapshot.status.health),
			systemNames: [...snapshot.status.system_names],
			trackers: snapshot.status.trackers.map((tracker) => ({
				trackerId: tracker.tracker_id,
				name: tracker.name ?? null,
				positionMetres: point(tracker.position_metres),
				ageMillis: tracker.age_millis,
				stale: tracker.stale,
				source: tracker.source,
			})),
			placements: snapshot.status.placements.map((placement) => ({
				bindingId: placement.binding_id,
				pointFixtureId: placement.point_fixture_id,
				positionMetres: point(placement.position_metres) ?? [0, 0, 0],
				outOfReach: placement.out_of_reach,
			})),
			occupiedZoneIds: [...snapshot.status.occupied_zone_ids],
			frames: snapshot.status.frames,
			ignoredDatagrams: snapshot.status.ignored_datagrams,
			error: snapshot.status.error ?? null,
		},
		points: snapshot.points.map((point) => ({
			fixtureId: point.fixture_id,
			name: point.name,
			fixtureNumber: point.fixture_number ?? null,
		})),
		macros: snapshot.macros.map((entry) => ({
			id: entry.id,
			number: entry.number,
			name: entry.name,
		})),
	};
}

function mapConfiguration(
	configuration: WirePsnSnapshot["configuration"],
): PsnConfiguration {
	return {
		enabled: configuration.enabled,
		group: configuration.group,
		port: configuration.port,
		interface: configuration.interface ?? null,
		staleAfterMillis: configuration.stale_after_millis,
		calibration: {
			offsetMetres: point(configuration.calibration.offset_metres) ?? [0, 0, 0],
			rotationDegrees: configuration.calibration.rotation_degrees,
			scale: configuration.calibration.scale,
		},
		bindings: configuration.bindings.map((binding) => ({
			id: binding.id,
			trackerId: binding.tracker_id,
			pointFixtureId: binding.point_fixture_id,
			enabled: binding.enabled,
		})),
		zones: configuration.zones.map((zone) => ({
			id: zone.id,
			name: zone.name,
			minMetres: point(zone.min_metres) ?? [0, 0, 0],
			maxMetres: point(zone.max_metres) ?? [0, 0, 0],
			trackerIds: [...zone.tracker_ids],
			enterMacroId: zone.enter_macro_id ?? null,
			leaveMacroId: zone.leave_macro_id ?? null,
			dwellMillis: zone.dwell_millis,
		})),
	};
}

function mapHealth(
	health: WirePsnSnapshot["status"]["health"],
): PsnHealth | null {
	if (!health) return null;
	if (health.state === "stale")
		return { state: "stale", silentForMillis: health.silent_for_millis };
	return { state: health.state };
}

function point(value: number[] | null | undefined): StagePoint | null {
	if (!value || value.length < 3) return null;
	return [value[0], value[1], value[2]];
}
