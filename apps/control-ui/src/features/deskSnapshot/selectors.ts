import type { ShowEntry } from "../../api/types";
import type { DeskSnapshot } from "./store";

export function selectBootstrap(snapshot: DeskSnapshot) {
	return snapshot.bootstrap;
}

export function selectBootstrapReady(snapshot: DeskSnapshot) {
	return snapshot.bootstrap !== null;
}

export function selectSession(snapshot: DeskSnapshot) {
	return snapshot.session;
}

export function selectActiveShow(snapshot: DeskSnapshot) {
	return snapshot.bootstrap?.active_show ?? null;
}

export function selectActiveShowId(snapshot: DeskSnapshot) {
	return snapshot.bootstrap?.active_show?.id ?? null;
}

export function selectHardwareConnected(snapshot: DeskSnapshot) {
	return snapshot.bootstrap?.hardware_connected ?? false;
}

export function selectFrameRateHz(snapshot: DeskSnapshot) {
	return snapshot.bootstrap?.frame_rate_hz ?? null;
}

export function selectActiveTimecode(snapshot: DeskSnapshot) {
	return snapshot.bootstrap?.active_timecode ?? null;
}

export function selectOutputHealth(snapshot: DeskSnapshot) {
	return snapshot.bootstrap?.output_health ?? null;
}

export function equalActiveShow(
	left: ShowEntry | null,
	right: ShowEntry | null,
) {
	if (left === right) return true;
	if (!left || !right) return false;
	return (
		left.id === right.id &&
		left.name === right.name &&
		left.path === right.path &&
		left.revision === right.revision &&
		left.updated_at === right.updated_at &&
		equalRevisionCopy(left.revision_copy, right.revision_copy)
	);
}

function equalRevisionCopy(
	left: ShowEntry["revision_copy"],
	right: ShowEntry["revision_copy"],
) {
	if (left === right) return true;
	if (!left || !right) return false;
	return (
		left.show_id === right.show_id &&
		left.show_name === right.show_name &&
		left.revision === right.revision &&
		left.revision_name === right.revision_name &&
		left.copied_at === right.copied_at
	);
}
