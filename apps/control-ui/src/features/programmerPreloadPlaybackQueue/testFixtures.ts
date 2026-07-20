import { vi } from "vitest";
import type {
	ProgrammerPreloadPlaybackQueueEntry,
	ProgrammerPreloadPlaybackQueueEventMessage,
	ProgrammerPreloadPlaybackQueueProjection,
	ProgrammerPreloadPlaybackQueueScope,
	ProgrammerPreloadPlaybackQueueSnapshot,
} from "./contracts";
import type {
	ProgrammerPreloadPlaybackQueueEventObserver,
	ProgrammerPreloadPlaybackQueueEventTransport,
} from "./transport";

export const SHOW_ID = "show-a";
export const USER_ID = "operator-a";
export const OTHER_USER_ID = "operator-b";
export const AUTHORITY_A = "server-a/session-a";
export const AUTHORITY_B = "server-b/session-b";

export function queuedPlayback(
	overrides: Partial<ProgrammerPreloadPlaybackQueueEntry> = {},
): ProgrammerPreloadPlaybackQueueEntry {
	return {
		playbackNumber: 7,
		page: null,
		action: "go",
		surface: "physical",
		...overrides,
	};
}

export function queueProjection(
	overrides: Partial<ProgrammerPreloadPlaybackQueueProjection> = {},
): ProgrammerPreloadPlaybackQueueProjection {
	return {
		userId: USER_ID,
		revision: 2,
		actions: [queuedPlayback()],
		...overrides,
	};
}

export function queueSnapshot(
	overrides: {
		cursor?: number;
		userId?: string;
		revision?: number;
		actions?: readonly ProgrammerPreloadPlaybackQueueEntry[];
	} = {},
): ProgrammerPreloadPlaybackQueueSnapshot {
	return {
		cursor: overrides.cursor ?? 10,
		projection: queueProjection({
			userId: overrides.userId ?? USER_ID,
			revision: overrides.revision ?? 2,
			actions: overrides.actions ?? [queuedPlayback()],
		}),
	};
}

interface FakeSubscription {
	scope: ProgrammerPreloadPlaybackQueueScope;
	after: number | null;
	observer: ProgrammerPreloadPlaybackQueueEventObserver;
	close: ReturnType<typeof vi.fn>;
	repair: ReturnType<typeof vi.fn>;
}

export class FakeProgrammerPreloadPlaybackQueueTransport
	implements ProgrammerPreloadPlaybackQueueEventTransport
{
	readonly subscriptions: FakeSubscription[] = [];

	subscribe(
		scope: ProgrammerPreloadPlaybackQueueScope,
		after: number | null,
		observer: ProgrammerPreloadPlaybackQueueEventObserver,
	) {
		const subscription = {
			scope,
			after,
			observer,
			close: vi.fn(),
			repair: vi.fn(),
		};
		this.subscriptions.push(subscription);
		return subscription;
	}

	emit(message: ProgrammerPreloadPlaybackQueueEventMessage) {
		this.subscriptions.at(-1)?.observer.message(message);
	}
}

export async function settleQueueSession() {
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
}
