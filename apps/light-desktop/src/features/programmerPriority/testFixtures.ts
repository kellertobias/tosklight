import { vi } from "vitest";
import type {
	ProgrammerPriorityActionOutcome,
	ProgrammerPriorityActionRequest,
	ProgrammerPriorityEventMessage,
	ProgrammerPriorityProjection,
	ProgrammerPriorityScope,
	ProgrammerPrioritySnapshot,
} from "./contracts";
import type {
	ProgrammerPriorityEventObserver,
	ProgrammerPriorityTransport,
} from "./transport";

export const USER_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
export const OTHER_USER_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
export const CORRELATION_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";

export function priorityProjection(
	overrides: Partial<ProgrammerPriorityProjection> = {},
): ProgrammerPriorityProjection {
	return {
		userId: USER_ID,
		revision: 1,
		priority: 0,
		changedAt: "2026-07-21T10:00:00Z",
		...overrides,
	};
}

export function prioritySnapshot(
	overrides: Partial<ProgrammerPriorityProjection> & { cursor?: number } = {},
): ProgrammerPrioritySnapshot {
	const { cursor = 10, ...projection } = overrides;
	return { cursor, projection: priorityProjection(projection) };
}

export function changedOutcome(
	requestId: string,
	projection: ProgrammerPriorityProjection,
	eventSequence = 11,
): ProgrammerPriorityActionOutcome {
	return {
		requestId,
		correlationId: CORRELATION_ID,
		status: "changed",
		projection,
		eventSequence,
		replayed: false,
		warning: null,
	};
}

export function noChangeOutcome(
	requestId: string,
	projection = priorityProjection(),
	replayed = false,
): ProgrammerPriorityActionOutcome {
	return {
		requestId,
		correlationId: CORRELATION_ID,
		status: "no_change",
		projection,
		eventSequence: null,
		replayed,
		warning: null,
	};
}

export interface FakePrioritySubscription {
	scope: ProgrammerPriorityScope;
	afterSequence: number | null;
	observer: ProgrammerPriorityEventObserver;
	close: ReturnType<typeof vi.fn>;
	repair: ReturnType<typeof vi.fn>;
}

export class FakeProgrammerPriorityTransport
	implements ProgrammerPriorityTransport
{
	readonly subscriptions: FakePrioritySubscription[] = [];
	readonly loadSnapshot = vi.fn(async (scope: ProgrammerPriorityScope) =>
		prioritySnapshot({ userId: scope.userId }),
	);
	readonly applyAction = vi.fn(
		async (
			scope: ProgrammerPriorityScope,
			request: ProgrammerPriorityActionRequest,
		) =>
			changedOutcome(
				request.requestId,
				priorityProjection({
					userId: scope.userId,
					revision: request.expectedRevision + 1,
					priority: request.priority,
				}),
			),
	);

	readonly subscribe = vi.fn(
		(
			scope: ProgrammerPriorityScope,
			afterSequence: number | null,
			observer: ProgrammerPriorityEventObserver,
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

	emit(message: ProgrammerPriorityEventMessage, index = -1) {
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

export async function settlePrioritySession() {
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
}
