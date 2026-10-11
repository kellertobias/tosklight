import type { PlaybackAddressValue } from "../../api/playbackAddress";

export interface PlaybackSoloRegion {
	readonly id: string;
	readonly name: string;
	readonly members: readonly PlaybackAddressValue[];
}

export interface PlaybackSoloScope {
	readonly showId: string;
}

export interface PlaybackSoloAuthority {
	/** Changes whenever the authenticated server/session authority is replaced. */
	readonly authorityId: string;
	readonly scope: PlaybackSoloScope;
}

export interface PlaybackSoloSnapshot {
	readonly showId: string;
	readonly revision: number;
	readonly regions: readonly PlaybackSoloRegion[];
}

export interface PlaybackSoloSaveOutcome
	extends PlaybackSoloSnapshot {
	readonly requestId: string;
	readonly replayed: boolean;
	readonly changed: boolean;
}

export interface PlaybackSoloChange {
	readonly showId: string;
	readonly revision: number;
}

export interface PlaybackSoloEventObserver {
	/** Server acknowledged the subscription; snapshot repair may now close the gap. */
	ready?(): void;
	changed(change: PlaybackSoloChange): void;
	gap(): void;
	error(error: Error): void;
	closed(): void;
}

export interface PlaybackSoloEventStream {
	close(): void;
}

export interface PlaybackSoloTransport {
	loadSnapshot(
		scope: PlaybackSoloScope,
		signal?: AbortSignal,
	): Promise<PlaybackSoloSnapshot>;
	save(
		scope: PlaybackSoloScope,
		expectedRevision: number,
		regions: readonly PlaybackSoloRegion[],
		requestId: string,
		signal?: AbortSignal,
	): Promise<PlaybackSoloSaveOutcome>;
	subscribe?(
		scope: PlaybackSoloScope,
		observer: PlaybackSoloEventObserver,
	): PlaybackSoloEventStream;
}

export interface PlaybackSoloCapability {
	/** Stable for one authenticated server/session authority, even as local errors change. */
	readonly authorityId: string | null;
	/** Changes for session, show, server transport, or authority replacement. */
	readonly authorityGeneration: number;
	readonly available: boolean;
	readonly error: string | null;
	getRegions(): readonly PlaybackSoloRegion[] | null;
	isSaving(): boolean;
	subscribe(listener: () => void): () => void;
	activate(): () => void;
	load(): Promise<readonly PlaybackSoloRegion[] | null>;
	save(
		regions: readonly PlaybackSoloRegion[],
	): Promise<readonly PlaybackSoloRegion[] | null>;
	clearError(): void;
}
