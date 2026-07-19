import { vi } from "vitest";
import type {
	ProgrammerCaptureModeEventMessage,
	ProgrammerCaptureModeProjection,
	ProgrammerCaptureModeScope,
	ProgrammerCaptureModeSnapshot,
} from "./contracts";
import type {
	ProgrammerCaptureModeEventObserver,
	ProgrammerCaptureModeEventTransport,
} from "./transport";

export const SHOW_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
export const OTHER_SHOW_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
export const USER_ID = "operator-a";
export const OTHER_USER_ID = "operator-b";

export function captureModeProjection(
	overrides: Partial<ProgrammerCaptureModeProjection> = {},
): ProgrammerCaptureModeProjection {
	return {
		userId: USER_ID,
		revision: 1,
		blind: false,
		preview: false,
		preloadCaptureProgrammer: false,
		...overrides,
	};
}

export function captureModeSnapshot(
	overrides: Partial<ProgrammerCaptureModeProjection> & {
		cursor?: number;
	} = {},
): ProgrammerCaptureModeSnapshot {
	const { cursor = 10, ...projection } = overrides;
	return { cursor, projection: captureModeProjection(projection) };
}

interface FakeSubscription {
	scope: ProgrammerCaptureModeScope;
	after: number | null;
	observer: ProgrammerCaptureModeEventObserver;
	close: ReturnType<typeof vi.fn>;
	repair: ReturnType<typeof vi.fn>;
}

export class FakeProgrammerCaptureModeTransport
	implements ProgrammerCaptureModeEventTransport
{
	readonly subscriptions: FakeSubscription[] = [];

	subscribe(
		scope: ProgrammerCaptureModeScope,
		after: number | null,
		observer: ProgrammerCaptureModeEventObserver,
	) {
		const subscription = {
			scope: { ...scope },
			after,
			observer,
			close: vi.fn(),
			repair: vi.fn(),
		};
		this.subscriptions.push(subscription);
		return subscription;
	}

	emit(message: ProgrammerCaptureModeEventMessage) {
		this.subscriptions.at(-1)?.observer.message(message);
	}
}

export async function settleCaptureModeSession() {
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
}
