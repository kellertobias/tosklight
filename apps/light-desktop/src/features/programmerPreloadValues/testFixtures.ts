import { vi } from "vitest";
import type {
	ProgrammerPreloadFixtureValue,
	ProgrammerPreloadGroupValue,
	ProgrammerPreloadValuesEventMessage,
	ProgrammerPreloadValuesProjection,
	ProgrammerPreloadValuesScope,
	ProgrammerPreloadValuesSnapshot,
} from "./contracts";
import type {
	ProgrammerPreloadValuesEventObserver,
	ProgrammerPreloadValuesEventTransport,
} from "./transport";

export const SHOW_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
export const OTHER_SHOW_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
export const USER_ID = "operator-a";
export const OTHER_USER_ID = "operator-b";
export const FIXTURE_1 = "11111111-1111-4111-8111-111111111111";
export const FIXTURE_2 = "22222222-2222-4222-8222-222222222222";

export function preloadFixtureValue(
	level = 0.25,
	overrides: Partial<ProgrammerPreloadFixtureValue> = {},
): ProgrammerPreloadFixtureValue {
	return {
		fixtureId: FIXTURE_1,
		attribute: "intensity",
		value: { kind: "normalized", value: level },
		programmerOrder: 1,
		fade: false,
		fadeMillis: null,
		delayMillis: null,
		...overrides,
	};
}

export function preloadGroupValue(
	level = 0.5,
	overrides: Partial<ProgrammerPreloadGroupValue> = {},
): ProgrammerPreloadGroupValue {
	return {
		groupId: "front",
		attribute: "intensity",
		value: { kind: "normalized", value: level },
		programmerOrder: 2,
		fade: false,
		fadeMillis: null,
		delayMillis: null,
		...overrides,
	};
}

export function preloadProjection(
	options: {
		userId?: string;
		revision?: number;
		fixtureValues?: readonly ProgrammerPreloadFixtureValue[];
		groupValues?: readonly ProgrammerPreloadGroupValue[];
	} = {},
): ProgrammerPreloadValuesProjection {
	return {
		userId: options.userId ?? USER_ID,
		revision: options.revision ?? 1,
		fixtureValues: options.fixtureValues ?? [preloadFixtureValue()],
		groupValues: options.groupValues ?? [],
	};
}

export function preloadSnapshot(
	options: Parameters<typeof preloadProjection>[0] & { cursor?: number } = {},
): ProgrammerPreloadValuesSnapshot {
	return {
		cursor: options.cursor ?? 10,
		projection: preloadProjection(options),
	};
}

interface FakeSubscription {
	scope: ProgrammerPreloadValuesScope;
	after: number | null;
	observer: ProgrammerPreloadValuesEventObserver;
	close: ReturnType<typeof vi.fn>;
	repair: ReturnType<typeof vi.fn>;
}

export class FakeProgrammerPreloadValuesTransport
	implements ProgrammerPreloadValuesEventTransport
{
	readonly subscriptions: FakeSubscription[] = [];

	subscribe(
		scope: ProgrammerPreloadValuesScope,
		after: number | null,
		observer: ProgrammerPreloadValuesEventObserver,
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

	emit(message: ProgrammerPreloadValuesEventMessage) {
		this.subscriptions.at(-1)?.observer.message(message);
	}
}

export async function settlePreloadSession() {
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
}
