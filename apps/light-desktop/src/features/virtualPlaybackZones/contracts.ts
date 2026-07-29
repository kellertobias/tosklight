/** One-based surface cells map to Virtual Playback numbers 1001 through 9998. */
export const MAX_PERSISTED_VIRTUAL_PLAYBACK_ZONE_SLOT = 8_998;

export interface VirtualPlaybackZone {
	readonly id: string;
	readonly name: string;
	readonly slots: readonly number[];
}

export type VirtualPlaybackSurfacePageMode =
	| { readonly type: "follow_main" }
	| { readonly type: "pinned"; readonly page: number };

export interface VirtualPlaybackExclusionSurface {
	readonly revision: number;
	readonly pageMode: VirtualPlaybackSurfacePageMode;
	readonly zones: readonly VirtualPlaybackZone[];
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
	readonly desks: Readonly<
		Record<
			string,
			Readonly<Record<string, VirtualPlaybackExclusionSurface>>
		>
	>;
}

export interface VirtualPlaybackZonesSaveOutcome {
	readonly requestId: string;
	readonly showId: string;
	readonly deskId: string;
	readonly surfaceId: string;
	readonly surface: VirtualPlaybackExclusionSurface;
	readonly replayed: boolean;
	readonly changed: boolean;
}

export interface VirtualPlaybackZonesChange {
	readonly showId: string;
	readonly deskId: string;
	readonly surfaceId: string;
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
	saveSurface(
		scope: VirtualPlaybackZonesScope,
		surfaceId: string,
		expectedRevision: number,
		pageMode: VirtualPlaybackSurfacePageMode,
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
	/** Changes for session, show, desk, server transport, or authority replacement. */
	readonly authorityGeneration: number;
	readonly available: boolean;
	readonly error: string | null;
	getSurface(surfaceId: string): readonly VirtualPlaybackZone[] | null;
	isSavingSurface(surfaceId: string): boolean;
	subscribeSurface(surfaceId: string, listener: () => void): () => void;
	activateSurface(surfaceId: string): () => void;
	loadSurface(surfaceId: string): Promise<readonly VirtualPlaybackZone[] | null>;
	saveSurface(
		surfaceId: string,
		pageMode: VirtualPlaybackSurfacePageMode,
		zones: readonly VirtualPlaybackZone[],
	): Promise<readonly VirtualPlaybackZone[] | null>;
	clearError(): void;
}
