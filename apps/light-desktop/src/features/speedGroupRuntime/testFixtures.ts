import { vi } from "vitest";
import type {
	SpeedGroupActionOutcome,
	SpeedGroupActionRequest,
	SpeedGroupAuthorityProjection,
	SpeedGroupEventMessage,
	SpeedGroupId,
	SpeedGroupProjection,
	SpeedGroupRuntimeScope,
	SpeedGroupSnapshot,
} from "./contracts";
import type {
	SpeedGroupEventObserver,
	SpeedGroupRuntimeTransport,
} from "./transport";

export const DESK_ID = "00000000-0000-4000-8000-000000000101";
export const OTHER_DESK_ID = "00000000-0000-4000-8000-000000000102";
export const AUTHORITY_ID = "00000000-0000-4000-8000-000000000201";
export const OTHER_AUTHORITY_ID = "00000000-0000-4000-8000-000000000202";
export const CORRELATION_ID = "00000000-0000-4000-8000-000000000301";

type ChangedOutcome = Extract<SpeedGroupActionOutcome, { status: "changed" }>;
type NoChangeOutcome = Extract<
	SpeedGroupActionOutcome,
	{ status: "no_change" }
>;

const BPMS = [120, 90, 60, 30, 15];

export function speedGroup(
	group: SpeedGroupId,
	overrides: Partial<SpeedGroupProjection> = {},
): SpeedGroupProjection {
	return {
		group,
		manualBpm: BPMS[group.charCodeAt(0) - 65] ?? 120,
		paused: false,
		speedMasterScale: 1,
		synchronizedWith: null,
		phaseOriginMillis: 100,
		...overrides,
	};
}

export function speedAuthority(
	overrides: Partial<SpeedGroupAuthorityProjection> = {},
): SpeedGroupAuthorityProjection {
	return {
		authorityId: AUTHORITY_ID,
		revision: 1,
		groups: (["A", "B", "C", "D", "E"] as const).map((group) =>
			speedGroup(group),
		),
		...overrides,
	};
}

export function speedSnapshot(
	overrides: Partial<SpeedGroupAuthorityProjection> & { cursor?: number } = {},
): SpeedGroupSnapshot {
	const { cursor = 10, ...projection } = overrides;
	return { cursor, projection: speedAuthority(projection) };
}

export function changedOutcome(
	requestId: string,
	groups: readonly SpeedGroupProjection[],
	overrides: Partial<ChangedOutcome> = {},
): ChangedOutcome {
	return {
		requestId,
		correlationId: CORRELATION_ID,
		authorityId: AUTHORITY_ID,
		revision: 2,
		appliedAtMillis: 200,
		groups,
		status: "changed",
		eventSequence: 11,
		replayed: false,
		durability: "durable",
		warning: null,
		...overrides,
	};
}

export function noChangeOutcome(
	requestId: string,
	groups: readonly SpeedGroupProjection[],
	replayed = false,
): NoChangeOutcome {
	return {
		requestId,
		correlationId: CORRELATION_ID,
		authorityId: AUTHORITY_ID,
		revision: 1,
		appliedAtMillis: 200,
		groups,
		status: "no_change",
		eventSequence: null,
		replayed,
		durability: "durable",
		warning: null,
	};
}

export function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason: unknown) => void;
	const promise = new Promise<T>((accept, decline) => {
		resolve = accept;
		reject = decline;
	});
	return { promise, resolve, reject };
}

export interface FakeSpeedGroupSubscription {
	scope: SpeedGroupRuntimeScope;
	afterSequence: number | null;
	observer: SpeedGroupEventObserver;
	close: ReturnType<typeof vi.fn>;
	repair: ReturnType<typeof vi.fn>;
}

export class FakeSpeedGroupRuntimeTransport
	implements SpeedGroupRuntimeTransport
{
	readonly subscriptions: FakeSpeedGroupSubscription[] = [];
	readonly loadSnapshot = vi.fn(async (_scope: SpeedGroupRuntimeScope) =>
		speedSnapshot(),
	);
	readonly applyAction = vi.fn(
		async (
			_scope: SpeedGroupRuntimeScope,
			request: SpeedGroupActionRequest,
		) => {
			const action = request.action;
			const identities =
				action.type === "synchronize"
					? [action.source, action.target].sort()
					: [action.group];
			return changedOutcome(
				request.requestId,
				identities.map((group) => speedGroup(group)),
				{ revision: request.expectedRevision + 1 },
			);
		},
	);
	readonly subscribe = vi.fn(
		(
			scope: SpeedGroupRuntimeScope,
			afterSequence: number | null,
			observer: SpeedGroupEventObserver,
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

	emit(message: SpeedGroupEventMessage, index = -1) {
		this.subscriptions.at(index)?.observer.message(message);
	}
}

export async function settleSpeedGroupSession() {
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
}
