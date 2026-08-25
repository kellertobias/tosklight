import { vi } from "vitest";
import type {
	ProgrammerDynamicValue,
	ProgrammerFixtureValue,
	ProgrammerGroupValue,
	ProgrammerValuesChange,
	ProgrammerValuesEventMessage,
	ProgrammerValuesProjection,
	ProgrammerValuesScope,
	ProgrammerValuesSnapshot,
} from "./contracts";
import type {
	ProgrammerValuesEventObserver,
	ProgrammerValuesEventTransport,
} from "./transport";

export const SHOW_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
export const OTHER_SHOW_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
export const SESSION_ID = "operator-a";
export const OTHER_SESSION_ID = "operator-b";
export const FIXTURE_1 = "11111111-1111-4111-8111-111111111111";
export const FIXTURE_2 = "22222222-2222-4222-8222-222222222222";
export const DYNAMIC_INSTANCE_1 = "33333333-3333-4333-8333-333333333333";
export const DYNAMIC_INSTANCE_2 = "44444444-4444-4444-8444-444444444444";

export function dynamicValue(
	instanceLink = DYNAMIC_INSTANCE_1,
	overrides: Partial<ProgrammerDynamicValue> = {},
): ProgrammerDynamicValue {
	return {
		fixtureId: FIXTURE_1,
		attribute: "intensity",
		value: {
			type: "dynamic_off",
			instance_link: instanceLink,
			timing: { fade_millis: null, delay_millis: null },
		},
		programmerOrder: 3,
		changedAtMillis: 100,
		...overrides,
	};
}

export function fixtureValue(
	level = 0.25,
	overrides: Partial<ProgrammerFixtureValue> = {},
): ProgrammerFixtureValue {
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

export function groupValue(
	level = 0.5,
	overrides: Partial<ProgrammerGroupValue> = {},
): ProgrammerGroupValue {
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

export function valuesProjection(
	options: {
		sessionId?: string;
		revision?: number;
		fixtureValues?: readonly ProgrammerFixtureValue[];
		groupValues?: readonly ProgrammerGroupValue[];
		dynamicValues?: readonly ProgrammerDynamicValue[];
	} = {},
): ProgrammerValuesProjection {
	return {
		revision: options.revision ?? 1,
		fixtureValues: options.fixtureValues ?? [fixtureValue()],
		groupValues: options.groupValues ?? [],
		...(options.dynamicValues?.length
			? { dynamicValues: options.dynamicValues }
			: {}),
	};
}

export function valuesSnapshot(
	options: Parameters<typeof valuesProjection>[0] & { cursor?: number } = {},
): ProgrammerValuesSnapshot {
	return {
		cursor: options.cursor ?? 10,
		projection: valuesProjection(options),
	};
}

export function valuesChange(
	projection: ProgrammerValuesProjection,
): ProgrammerValuesChange {
	return {
		revision: projection.revision,
		fixtureValues: projection.fixtureValues,
		removedFixtureValues: [],
		groupValues: projection.groupValues,
		removedGroupValues: [],
		dynamicValues: projection.dynamicValues ?? [],
		removedDynamicValues: [],
	};
}

interface FakeSubscription {
	scope: ProgrammerValuesScope;
	after: number | null;
	observer: ProgrammerValuesEventObserver;
	close: ReturnType<typeof vi.fn>;
	repair: ReturnType<typeof vi.fn>;
}

export class FakeProgrammerValuesTransport
	implements ProgrammerValuesEventTransport
{
	readonly subscriptions: FakeSubscription[] = [];

	subscribe(
		scope: ProgrammerValuesScope,
		after: number | null,
		observer: ProgrammerValuesEventObserver,
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

	emit(message: ProgrammerValuesEventMessage) {
		this.subscriptions.at(-1)?.observer.message(message);
	}
}

export async function settleProgrammerValuesSession() {
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
}
