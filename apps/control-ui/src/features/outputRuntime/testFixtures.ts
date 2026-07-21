import { vi } from "vitest";
import type {
	OutputRuntimeActionOutcome,
	OutputRuntimeActionRequest,
	OutputRuntimeEventMessage,
	OutputRuntimeProjection,
	OutputRuntimeScope,
	OutputRuntimeSnapshot,
} from "./contracts";
import type {
	OutputRuntimeEventObserver,
	OutputRuntimeTransport,
} from "./transport";

export const SHOW_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
export const OTHER_SHOW_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
export const DESK_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
export const OTHER_DESK_ID = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
export const CORRELATION_ID = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";

export function outputProjection(
	overrides: Partial<OutputRuntimeProjection> = {},
): OutputRuntimeProjection {
	return {
		showId: SHOW_ID,
		identity: "global_master",
		revision: 1,
		grandMaster: 1,
		blackout: false,
		...overrides,
	};
}

export function outputSnapshot(
	overrides: Partial<OutputRuntimeProjection> & { cursor?: number } = {},
): OutputRuntimeSnapshot {
	const { cursor = 10, ...projection } = overrides;
	return { cursor, projection: outputProjection(projection) };
}

export function changedOutcome(
	requestId: string,
	projection: OutputRuntimeProjection,
	eventSequence = 11,
): OutputRuntimeActionOutcome {
	return {
		requestId,
		correlationId: CORRELATION_ID,
		status: "changed",
		projection,
		eventSequence,
		replayed: false,
		durability: "durable",
		warning: null,
	};
}

export function noChangeOutcome(
	requestId: string,
	projection = outputProjection(),
	replayed = false,
): OutputRuntimeActionOutcome {
	return {
		requestId,
		correlationId: CORRELATION_ID,
		status: "no_change",
		projection,
		eventSequence: null,
		replayed,
		durability: "durable",
		warning: null,
	};
}

export interface FakeOutputSubscription {
	scope: OutputRuntimeScope;
	afterSequence: number | null;
	observer: OutputRuntimeEventObserver;
	close: ReturnType<typeof vi.fn>;
	repair: ReturnType<typeof vi.fn>;
}

export class FakeOutputRuntimeTransport implements OutputRuntimeTransport {
	readonly subscriptions: FakeOutputSubscription[] = [];
	readonly loadSnapshot = vi.fn(async (scope: OutputRuntimeScope) =>
		outputSnapshot({ showId: scope.showId }),
	);
	readonly applyAction = vi.fn(
		async (scope: OutputRuntimeScope, request: OutputRuntimeActionRequest) =>
			changedOutcome(
				request.requestId,
				outputProjection({
					showId: scope.showId,
					revision: request.expectedRevision + 1,
					grandMaster: request.grandMaster ?? 1,
					blackout: request.blackout ?? false,
				}),
			),
	);

	readonly subscribe = vi.fn(
		(
			scope: OutputRuntimeScope,
			afterSequence: number | null,
			observer: OutputRuntimeEventObserver,
		) => {
			const subscription = {
				scope: { ...scope },
				afterSequence,
				observer,
				close: vi.fn(),
				repair: vi.fn(),
			};
			this.subscriptions.push(subscription);
			return subscription;
		},
	);

	emit(message: OutputRuntimeEventMessage, index = -1) {
		this.subscriptions.at(index)?.observer.message(message);
	}
}

export function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason: unknown) => void;
	const promise = new Promise<T>((onResolve, onReject) => {
		resolve = onResolve;
		reject = onReject;
	});
	return { promise, resolve, reject };
}

export async function settleOutputSession() {
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
}
