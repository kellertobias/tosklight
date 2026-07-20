/** Legacy grid cells remain portable even though Playback assignments stop at 127. */
export const MAX_PERSISTED_VIRTUAL_PLAYBACK_ZONE_SLOT = 144;

export interface VirtualPlaybackZone {
	readonly id: string;
	readonly name: string;
	readonly slots: readonly number[];
}

export interface VirtualPlaybackZonesScope {
	readonly showId: string;
	readonly deskId: string;
}

export interface VirtualPlaybackZonesAuthority {
	/** Changes whenever the authenticated server/session authority is replaced. */
	readonly authorityId: string;
	readonly scope: VirtualPlaybackZonesScope;
}

export interface VirtualPlaybackZonesSnapshot {
	readonly showId: string;
	readonly deskId: string;
	readonly surfaces: Readonly<Record<string, readonly VirtualPlaybackZone[]>>;
}

export interface VirtualPlaybackZonesSaveOutcome {
	readonly surfaceId: string;
	readonly zones: readonly VirtualPlaybackZone[];
}

export interface VirtualPlaybackZonesTransport {
	loadSnapshot(
		scope: VirtualPlaybackZonesScope,
		signal?: AbortSignal,
	): Promise<VirtualPlaybackZonesSnapshot>;
	saveSurface(
		scope: VirtualPlaybackZonesScope,
		surfaceId: string,
		zones: readonly VirtualPlaybackZone[],
		signal?: AbortSignal,
	): Promise<VirtualPlaybackZonesSaveOutcome>;
}

export interface VirtualPlaybackZonesCapability {
	/** Stable for one authenticated server/session authority, even as local errors change. */
	readonly authorityId: string | null;
	/** Changes for session, show, desk, server transport, or authority replacement. */
	readonly authorityGeneration: number;
	readonly available: boolean;
	readonly error: string | null;
	getSurface(surfaceId: string): readonly VirtualPlaybackZone[] | null;
	isSavingSurface(surfaceId: string): boolean;
	subscribeSurface(surfaceId: string, listener: () => void): () => void;
	loadSurface(surfaceId: string): Promise<readonly VirtualPlaybackZone[] | null>;
	saveSurface(
		surfaceId: string,
		zones: readonly VirtualPlaybackZone[],
	): Promise<readonly VirtualPlaybackZone[] | null>;
	clearError(): void;
}
