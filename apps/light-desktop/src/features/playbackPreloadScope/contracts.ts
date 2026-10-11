import type {
	PlaybackPreloadScope,
	PlaybackPreloadScopeSnapshot,
	PlaybackPreloadScopeUpdateOutcome,
} from "../../api/generated/light-wire";
export interface PreloadScopeAuthority {
	authorityId: string;
	scope: { showId: string };
}
export interface PreloadScopeTransport {
	loadSnapshot(
		authority: { showId: string },
		signal?: AbortSignal,
	): Promise<PlaybackPreloadScopeSnapshot>;
	save(
		authority: { showId: string },
		revision: number,
		scope: PlaybackPreloadScope,
		requestId: string,
		signal?: AbortSignal,
	): Promise<PlaybackPreloadScopeUpdateOutcome>;
	subscribe(
		authority: { showId: string },
		observer: PreloadScopeObserver,
	): { close(): void };
}
export interface PreloadScopeObserver {
	ready?(): void;
	changed(change: { showId: string; revision: number }): void;
	gap(): void;
	error(error: Error): void;
	closed(): void;
}
