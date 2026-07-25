import { vi } from "vitest";
import type {
	ProgrammerLifecycleChange,
	ProgrammerLifecycleEventMessage,
	ProgrammerLifecycleProjection,
	ProgrammerLifecycleRow,
	ProgrammerLifecycleSnapshot,
} from "./contracts";
import type {
	ProgrammerLifecycleEventObserver,
	ProgrammerLifecycleEventTransport,
} from "./transport";

export const AUTHORITY_A = "server-a/session-a";
export const AUTHORITY_B = "server-b/session-b";
export const PROGRAMMER_A = "11111111-1111-4111-8111-111111111111";
export const PROGRAMMER_B = "22222222-2222-4222-8222-222222222222";
export const USER_A = "operator-a";
export const USER_B = "operator-b";

export function lifecycleRow(
	overrides: Partial<ProgrammerLifecycleRow> = {},
): ProgrammerLifecycleRow {
	return {
		programmerId: PROGRAMMER_A,
		userId: USER_A,
		connected: true,
		selectedFixtureCount: 2,
		normalValueCount: 3,
		preloadActive: false,
		sessions: [{ sessionId: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa" }],
		...overrides,
	};
}

export function otherLifecycleRow(
	overrides: Partial<ProgrammerLifecycleRow> = {},
) {
	return lifecycleRow({
		programmerId: PROGRAMMER_B,
		userId: USER_B,
		normalValueCount: 1,
		sessions: [],
		...overrides,
	});
}

export function lifecycleProjection(
	overrides: Partial<ProgrammerLifecycleProjection> = {},
): ProgrammerLifecycleProjection {
	return {
		revision: 4,
		programmers: [lifecycleRow()],
		...overrides,
	};
}

export function lifecycleSnapshot(
	overrides: {
		cursor?: number;
		revision?: number;
		programmers?: readonly ProgrammerLifecycleRow[];
	} = {},
): ProgrammerLifecycleSnapshot {
	return {
		cursor: overrides.cursor ?? 10,
		projection: lifecycleProjection({
			revision: overrides.revision ?? 4,
			programmers: overrides.programmers ?? [lifecycleRow()],
		}),
	};
}

export function upsertChange(
	row: ProgrammerLifecycleRow,
	revision: number,
): ProgrammerLifecycleChange {
	return { revision, delta: { type: "upsert", programmer: row } };
}

export function removalChange(
	programmerId: string,
	revision: number,
): ProgrammerLifecycleChange {
	return { revision, delta: { type: "remove", programmerId } };
}

interface FakeSubscription {
	after: number | null;
	observer: ProgrammerLifecycleEventObserver;
	close: ReturnType<typeof vi.fn>;
	repair: ReturnType<typeof vi.fn>;
}

export class FakeProgrammerLifecycleTransport
	implements ProgrammerLifecycleEventTransport
{
	readonly subscriptions: FakeSubscription[] = [];

	subscribe(after: number | null, observer: ProgrammerLifecycleEventObserver) {
		const subscription = {
			after,
			observer,
			close: vi.fn(),
			repair: vi.fn(),
		};
		this.subscriptions.push(subscription);
		return subscription;
	}

	emit(message: ProgrammerLifecycleEventMessage) {
		this.subscriptions.at(-1)?.observer.message(message);
	}
}

export async function settleLifecycleSession() {
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
}
