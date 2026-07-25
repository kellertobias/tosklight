import { vi } from "vitest";
import type {
	CommandLineProjection,
	ProgrammingChange,
	ProgrammingInteractionEventMessage,
	ProgrammingSnapshot,
	SelectionProjection,
} from "./contracts";
import type {
	ProgrammingEventObserver,
	ProgrammingEventScope,
	ProgrammingEventTransport,
} from "./transport";

export const SHOW_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
export const OTHER_SHOW_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
export const DESK_ID = "11111111-1111-4111-8111-111111111111";
export const OTHER_DESK_ID = "99999999-9999-4999-8999-999999999999";
export const FIXTURE_1 = "22222222-2222-4222-8222-222222222222";
export const FIXTURE_2 = "33333333-3333-4333-8333-333333333333";
export const FIXTURE_3 = "44444444-4444-4444-8444-444444444444";

export function commandLine(
	revision = 1,
	text = "FIXTURE",
	target: CommandLineProjection["target"] = "FIXTURE",
): CommandLineProjection {
	return {
		text,
		target,
		pristine: text === target,
		revision,
		pendingChoice: null,
	};
}

export function selection(
	revision = 1,
	selected = [FIXTURE_1],
): SelectionProjection {
	return {
		selected,
		expression: { type: "static" },
		revision,
		gestureOpen: false,
	};
}

export function programmingSnapshot({
	sequence = 10,
	deskId = DESK_ID,
	command = commandLine(),
	selected = selection(),
}: {
	sequence?: number;
	deskId?: string;
	command?: CommandLineProjection;
	selected?: SelectionProjection;
} = {}): ProgrammingSnapshot {
	return {
		cursor: sequence,
		projection: {
			deskId,
			commandLine: command,
			selection: selected,
		},
	};
}

export function commandChange({
	deskId = DESK_ID,
	revision = 2,
	text = "FIXTURE 2",
}: {
	deskId?: string;
	revision?: number;
	text?: string;
} = {}): ProgrammingChange {
	return {
		deskId,
		commandLine: commandLine(revision, text),
	};
}

export function selectionChange({
	deskId = DESK_ID,
	revision = 2,
	selected = [FIXTURE_2],
}: {
	deskId?: string;
	revision?: number;
	selected?: string[];
} = {}): ProgrammingChange {
	return {
		deskId,
		selection: selection(revision, selected),
	};
}

export interface FakeProgrammingSubscription {
	deskId: string;
	scope: ProgrammingEventScope;
	after: number | null;
	observer: ProgrammingEventObserver;
	close: ReturnType<typeof vi.fn>;
	repair: ReturnType<typeof vi.fn>;
}

export class FakeProgrammingTransport implements ProgrammingEventTransport {
	readonly subscriptions: FakeProgrammingSubscription[] = [];

	subscribe(
		deskId: string,
		scope: ProgrammingEventScope,
		after: number | null,
		observer: ProgrammingEventObserver,
	) {
		const subscription = {
			deskId,
			scope,
			after,
			observer,
			close: vi.fn(),
			repair: vi.fn(),
		};
		this.subscriptions.push(subscription);
		return subscription;
	}

	emit(message: ProgrammingInteractionEventMessage) {
		this.subscriptions.at(-1)?.observer.message(message);
	}
}

export async function settleSession() {
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
}
