import type { PlaybackRuntimeStore } from "../playbackRuntime/store";
import type { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import type { ProgrammerLifecycleStore } from "../programmerLifecycle/store";
import type { ProgrammerPreloadPlaybackQueueStore } from "../programmerPreloadPlaybackQueue/store";
import type { ProgrammerPreloadValuesStore } from "../programmerPreloadValues/store";
import type { ProgrammingInteractionStore } from "../programmingInteraction/store";
import type { ShowObjectsStore } from "../showObjects/store";
import type {
	ProgrammerPreloadLifecycleActions,
	ProgrammerPreloadLifecycleOutcome,
	ProgrammerPreloadLifecycleRequest,
	ProgrammerPreloadLifecycleScope,
	ProgrammerPreloadLifecycleTransport,
} from "./contracts";
import { ProgrammerPreloadLifecycleTransportError } from "./contracts";
import type {
	ProgrammerPreloadLifecycleMutation,
	ProgrammerPreloadLifecycleStore,
} from "./store";
import {
	captureLifecycleAuthority,
	captureLifecycleGuard,
	type LifecycleAuthorityGuard,
	matchesLifecycleGuard,
} from "./writerAuthority";

export interface RepairPorts {
	captureMode(error: Error): Promise<void>;
	values(error: Error): Promise<void>;
	queue(error: Error): Promise<void>;
	selection(error: Error): Promise<void>;
	lifecycle(error: Error): Promise<void>;
	runtime(error: Error): Promise<void>;
}

export interface ProgrammerPreloadLifecycleWriterOptions {
	scope: ProgrammerPreloadLifecycleScope;
	store: ProgrammerPreloadLifecycleStore;
	captureModeStore: ProgrammerCaptureModeStore;
	valuesStore: ProgrammerPreloadValuesStore;
	queueStore: ProgrammerPreloadPlaybackQueueStore;
	selectionStore: ProgrammingInteractionStore;
	lifecycleStore: ProgrammerLifecycleStore;
	showStore: ShowObjectsStore;
	runtimeStore: PlaybackRuntimeStore;
	readPreloadActive(): boolean | null;
	transport: ProgrammerPreloadLifecycleTransport;
	repair: RepairPorts;
	onError?: (error: Error | null) => void;
}

export interface LifecycleAuthority {
	storeScope: number;
	captureModeScope: number;
	valuesScope: number;
	queueScope: number;
	selectionScope: number;
	lifecycleScope: number;
	runtimeScope: number | null;
	showGeneration: number;
	request: ProgrammerPreloadLifecycleRequest;
}

/** One exact-authority, replay-safe Preload lifecycle mutation at a time. */
export class ProgrammerPreloadLifecycleWriter
	implements ProgrammerPreloadLifecycleActions
{
	private stopped = false;
	private tail = Promise.resolve();

	constructor(private readonly options: ProgrammerPreloadLifecycleWriterOptions) {}

	enter(requestId: string = crypto.randomUUID()) {
		return this.enqueue("enter", requestId);
	}

	go(requestId: string = crypto.randomUUID()) {
		return this.enqueue("go", requestId);
	}

	clearPending(requestId: string = crypto.randomUUID()) {
		return this.enqueue("clear_pending", requestId);
	}

	release(requestId: string = crypto.randomUUID()) {
		return this.enqueue("release", requestId);
	}

	stop() {
		this.stopped = true;
		const pending = this.options.store.getSnapshot().pending;
		if (pending) this.options.store.abandon(pending.requestId);
	}

	private enqueue(
		action: ProgrammerPreloadLifecycleMutation,
		requestId: string,
	) {
		if (this.stopped) return Promise.resolve(null);
		let guard: LifecycleAuthorityGuard;
		try {
			guard = captureLifecycleGuard(this.options);
		} catch (reason) {
			return this.refuse(asError(reason).message);
		}
		const result = this.tail.then(() => this.execute(action, requestId, guard));
		this.tail = result.then(
			() => undefined,
			() => undefined,
		);
		return result;
	}

	private async execute(
		action: ProgrammerPreloadLifecycleMutation,
		requestId: string,
		guard: LifecycleAuthorityGuard,
	) {
		if (this.stopped) return null;
		let authority: LifecycleAuthority;
		try {
			authority = captureLifecycleAuthority(this.options, action, requestId);
			if (!matchesLifecycleGuard(authority, guard))
				throw new Error("Preload lifecycle authority was replaced");
		} catch (reason) {
			return this.refuse(asError(reason).message);
		}
		if (!this.begin(authority, action)) return null;
		try {
			const outcome = await this.requestWithOneRetry(authority.request);
			if (!this.isCurrent(authority)) return this.abandon(authority);
			if (!(await this.reconcile(authority, outcome))) return null;
			if (!this.options.store.settle(authority.request.requestId, authority.storeScope))
				return null;
			this.options.onError?.(
				outcome.warning ? new Error(outcome.warning) : null,
			);
			return outcome;
		} catch (reason) {
			return this.fail(asError(reason), authority, reason);
		}
	}

	private begin(
		authority: LifecycleAuthority,
		action: ProgrammerPreloadLifecycleMutation,
	) {
		const values = this.options.valuesStore.getSnapshot().projection;
		const active = this.options.readPreloadActive() ?? false;
		const optimisticActive =
			action === "release"
				? false
				: action === "go"
					? Boolean(values?.fixtureValues.length || values?.groupValues.length)
					: active;
		return this.options.store.begin(
			{ requestId: authority.request.requestId, action, optimisticActive },
			authority.storeScope,
		);
	}

	private async requestWithOneRetry(request: ProgrammerPreloadLifecycleRequest) {
		try {
			return await this.options.transport.applyAction(this.options.scope, request);
		} catch (reason) {
			if (!isRetryable(reason)) throw reason;
			return this.options.transport.applyAction(this.options.scope, request);
		}
	}

	private async reconcile(
		authority: LifecycleAuthority,
		outcome: ProgrammerPreloadLifecycleOutcome,
	) {
		try {
			if (outcome.captureModeEventSequence !== null)
				this.options.captureModeStore.applyProjection(
					outcome.captureMode,
					outcome.captureModeEventSequence,
					authority.captureModeScope,
				);
			if (outcome.valuesProjection && outcome.valuesEventSequence !== null)
				this.options.valuesStore.applyProjection(
					outcome.valuesProjection,
					outcome.valuesEventSequence,
					authority.valuesScope,
				);
			if (outcome.queueProjection && outcome.queueEventSequence !== null)
				this.options.queueStore.applyProjection(
					outcome.queueProjection,
					outcome.queueEventSequence,
					authority.queueScope,
				);
			for (const change of outcome.commit?.runtimeChanges ?? [])
				this.options.runtimeStore.applyProjection(
					change.projection,
					change.eventSequence,
				);
			await this.repairResponseOnlyAuthorities(authority, outcome);
			return this.isCurrent(authority);
		} catch (reason) {
			await this.repairAll(asError(reason), authority);
			if (!this.isCurrent(authority)) return false;
			throw reason;
		}
	}

	private async repairResponseOnlyAuthorities(
		authority: LifecycleAuthority,
		outcome: ProgrammerPreloadLifecycleOutcome,
	) {
		const currentSelection = this.options.selectionStore.authoritativeSelectionRevision(
			authority.selectionScope,
		);
		if (currentSelection !== outcome.selectionRevision)
			await this.options.repair.selection(
				new Error("Preload lifecycle selection outcome requires repair"),
			);
		if (this.options.readPreloadActive() !== outcome.active)
			await this.options.repair.lifecycle(
				new Error("Preload lifecycle status outcome requires repair"),
			);
		if (this.options.readPreloadActive() !== outcome.active)
			throw new Error("Authoritative Preload lifecycle status did not reconcile");
	}

	private async fail(
		error: Error,
		authority: LifecycleAuthority,
		reason: unknown,
	) {
		if (!this.isCurrent(authority)) return this.abandon(authority);
		if (requiresRepair(reason)) await this.repairAll(error, authority);
		if (!this.isCurrent(authority)) return this.abandon(authority);
		this.options.store.rollback(
			authority.request.requestId,
			error,
			authority.storeScope,
		);
		this.options.onError?.(error);
		return null;
	}

	private async repairAll(error: Error, authority: LifecycleAuthority) {
		const repairs = [
			this.options.repair.captureMode(error),
			this.options.repair.values(error),
			this.options.repair.queue(error),
			this.options.repair.selection(error),
			this.options.repair.lifecycle(error),
		];
		if (authority.runtimeScope !== null)
			repairs.push(this.options.repair.runtime(error));
		await Promise.allSettled(repairs);
	}

	private isCurrent(authority: LifecycleAuthority) {
		const show = this.options.showStore.getSnapshot();
		return (
			!this.stopped &&
			this.options.store.isScopeCurrent(authority.storeScope) &&
			this.options.captureModeStore.isScopeCurrent(authority.captureModeScope) &&
			this.options.valuesStore.isScopeCurrent(authority.valuesScope) &&
			this.options.queueStore.isScopeCurrent(authority.queueScope) &&
			this.options.selectionStore.isScopeCurrent(authority.selectionScope) &&
			this.options.lifecycleStore.isScopeCurrent(authority.lifecycleScope) &&
			(authority.runtimeScope === null ||
				this.options.runtimeStore.isScopeCurrent(authority.runtimeScope)) &&
			show.showId === this.options.scope.showId &&
			show.authorityGeneration === authority.showGeneration
		);
	}

	private abandon(authority: LifecycleAuthority) {
		this.options.store.abandon(authority.request.requestId, authority.storeScope);
		return null;
	}

	private refuse(message: string) {
		this.options.onError?.(new Error(message));
		return Promise.resolve(null);
	}
}

function isRetryable(reason: unknown) {
	return (
		reason instanceof ProgrammerPreloadLifecycleTransportError && reason.retryable
	);
}

function requiresRepair(reason: unknown) {
	return (
		reason instanceof ProgrammerPreloadLifecycleTransportError &&
		reason.status === 409
	);
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
