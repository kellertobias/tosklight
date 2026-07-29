import { MAX_VIRTUAL_PLAYBACK_NUMBER } from "../../api/virtualPlaybackAddress";

/** Stable show-owned Virtual Playback numbers. Physical Playbacks stop at 1000. */
export const MIN_VIRTUAL_PLAYBACK_ZONE_NUMBER = 1_001;
export const MAX_VIRTUAL_PLAYBACK_ZONE_NUMBER = MAX_VIRTUAL_PLAYBACK_NUMBER;

export interface VirtualPlaybackZone {
	readonly id: string;
	readonly name: string;
	readonly playbackNumbers: readonly number[];
}

export interface VirtualPlaybackZonesScope {
	readonly showId: string;
}

export interface VirtualPlaybackZonesAuthority {
	/** Changes whenever the authenticated server/session authority is replaced. */
	readonly authorityId: string;
	readonly scope: VirtualPlaybackZonesScope;
}

export interface VirtualPlaybackZonesSnapshot {
	readonly showId: string;
	readonly revision: number;
	readonly zones: readonly VirtualPlaybackZone[];
}

export interface VirtualPlaybackZonesSaveOutcome
	extends VirtualPlaybackZonesSnapshot {
	readonly requestId: string;
	readonly replayed: boolean;
	readonly changed: boolean;
}

export interface VirtualPlaybackZonesChange {
	readonly showId: string;
	readonly revision: number;
}

export interface VirtualPlaybackZonesEventObserver {
	changed(change: VirtualPlaybackZonesChange): void;
	gap(): void;
	error(error: Error): void;
	closed(): void;
}

export interface VirtualPlaybackZonesEventStream {
	close(): void;
}

export interface VirtualPlaybackZonesTransport {
	loadSnapshot(
		scope: VirtualPlaybackZonesScope,
		signal?: AbortSignal,
	): Promise<VirtualPlaybackZonesSnapshot>;
	save(
		scope: VirtualPlaybackZonesScope,
		expectedRevision: number,
		zones: readonly VirtualPlaybackZone[],
		requestId: string,
		signal?: AbortSignal,
	): Promise<VirtualPlaybackZonesSaveOutcome>;
	subscribe?(
		scope: VirtualPlaybackZonesScope,
		observer: VirtualPlaybackZonesEventObserver,
	): VirtualPlaybackZonesEventStream;
}

export interface VirtualPlaybackZonesCapability {
	/** Stable for one authenticated server/session authority, even as local errors change. */
	readonly authorityId: string | null;
	/** Changes for session, show, server transport, or authority replacement. */
	readonly authorityGeneration: number;
	readonly available: boolean;
	readonly error: string | null;
	getZones(): readonly VirtualPlaybackZone[] | null;
	isSaving(): boolean;
	subscribe(listener: () => void): () => void;
	activate(): () => void;
	load(): Promise<readonly VirtualPlaybackZone[] | null>;
	save(
		zones: readonly VirtualPlaybackZone[],
	): Promise<readonly VirtualPlaybackZone[] | null>;
	clearError(): void;
}
