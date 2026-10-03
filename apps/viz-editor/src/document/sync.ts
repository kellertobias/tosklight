import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** The bound document's sync status changed. Carries the new status. */
export const SYNC_STATUS_EVENT = "sync-status-changed";

/** Precedence, highest first: error, conflict, offline, pending, synced. */
export type SyncPhase = "synced" | "pending" | "offline" | "conflict" | "error";

/** How a document opened from ToskLight Control stands with its desk. */
export interface SyncStatus {
	state: SyncPhase;
	/** The chip's words. */
	label: string;
	/** One sentence saying why. */
	detail: string;
	deskName: string;
	pending: number;
	conflicts: number;
	/** `true` only when Control has confirmed every edit. */
	savedToControl: boolean;
	/** Every edit is journaled on this computer before it is reported. */
	savedOnThisComputer: boolean;
}

export type ConflictReason =
	| "field_changed"
	| "object_deleted"
	| "object_modified"
	| "object_exists"
	| "refused"
	| "recovered";

/** One decision the operator owes: a field both sides changed, or a change Control refused. */
export interface SyncConflict {
	entry: number;
	kind: string;
	id: string;
	path: string;
	base: unknown;
	/** The Architect's version: the recoverable draft. */
	mine: unknown;
	/** Control's version, which the document shows until the operator decides. */
	theirs: unknown;
	reason: ConflictReason;
	label: string;
}

export type SyncResolution = "keep_control" | "use_mine";

export const syncSession = {
	/** `null` for a document bound to no desk. */
	status: () => invoke<SyncStatus | null>("sync_status"),
	conflicts: () => invoke<SyncConflict[]>("sync_conflicts"),
	resolve: (entry: number, resolution: SyncResolution) =>
		invoke<void>("resolve_sync_conflict", { entry, resolution }),
	dismissError: () => invoke<void>("dismiss_sync_error"),
	setOnline: (online: boolean) => invoke<void>("set_sync_online", { online }),
	onStatusChanged: (handler: (status: SyncStatus) => void): Promise<UnlistenFn> =>
		listen<SyncStatus>(SYNC_STATUS_EVENT, (event) => handler(event.payload)),
};
