import type { PatchedFixture } from "../../api/types";
import type {
	PatchEventMessage,
	PatchFixturePolicyAction,
	PatchMutation,
	PatchMutationOutcome,
	PatchPlacement,
} from "./contracts";
import {
	changedPatchFixtureCandidate,
	type PatchDefinitionResolver,
	type PatchFixtureCandidate,
} from "./model";
import { type PatchEventStream, type PatchTransport } from "./transport";
import {
	asError,
	authorityChanged,
	isAmbiguous,
	isConflict,
	patchMutation,
	shouldRepair,
} from "./mutationSupport";
import { PatchStore } from "./store";
import { frontendPerformanceDiagnostics } from "../frontendWarmup/diagnostics";

export interface PatchSessionOptions {
	showId: string;
	transport: PatchTransport;
	initialFixtures?: readonly PatchedFixture[];
	resolveDefinition: PatchDefinitionResolver;
	onError?: (error: Error) => void;
}

type CandidateMaterializer = (
	requestId: string,
) => readonly PatchFixtureCandidate[];
type ReleasePatchView = () => void;

const MAX_CONFLICT_RETRIES = 2;

export class PatchSession {
	readonly store: PatchStore;
	private readonly transport: PatchTransport;
	private readonly showId: string;
	private readonly onError?: (error: Error) => void;
	private stream: PatchEventStream | null = null;
	private stopped = true;
	private lifecycle = 0;
	private startPromise: Promise<void> | null = null;
	private repairPromise: Promise<void> | null = null;
	private writeQueue: Promise<void> = Promise.resolve();
	private reconnectTimer: ReturnType<typeof globalThis.setTimeout> | null =
		null;
	private activeViews = 0;
	private releaseGeneration = 0;

	constructor(options: PatchSessionOptions) {
		this.showId = options.showId;
		this.transport = options.transport;
		this.onError = options.onError;
		this.store = new PatchStore(
			options.showId,
			options.resolveDefinition,
			options.initialFixtures,
		);
	}

	start(): Promise<void> {
		if (!this.stopped && this.startPromise) return this.startPromise;
		this.store.beginAuthorityLoad();
		this.stopped = false;
		const lifecycle = ++this.lifecycle;
		this.startPromise = this.initialize(lifecycle).catch((error) => {
			if (this.isActive(lifecycle)) this.startPromise = null;
			throw error;
		});
		return this.startPromise;
	}

	activate(): ReleasePatchView {
		this.activeViews++;
		this.releaseGeneration++;
		void this.start().catch(() => undefined);
		let active = true;
		return () => {
			if (!active) return;
			active = false;
			this.releaseView();
		};
	}

	stop(): void {
		if (this.stopped) return;
		this.stopped = true;
		this.activeViews = 0;
		this.releaseGeneration++;
		this.lifecycle++;
		this.startPromise = null;
		this.repairPromise = null;
		if (this.reconnectTimer != null)
			globalThis.clearTimeout(this.reconnectTimer);
		this.reconnectTimer = null;
		this.stream?.close();
		this.stream = null;
		this.store.deactivate();
	}

	refresh(): Promise<void> {
		return this.repair();
	}

	patchFixtures(
		candidates: readonly PatchFixtureCandidate[],
		removeFixtureIds: readonly string[] = [],
		placements: readonly PatchPlacement[] = [],
	): Promise<PatchMutationOutcome> {
		if (!candidates.length && !removeFixtureIds.length)
			return Promise.reject(
				new Error("A Patch mutation must change at least one fixture"),
			);
		return this.queuePatch(
			candidates,
			removeFixtureIds,
			() => candidates,
			placements,
		);
	}

	updateFixture(
		fixtureId: string,
		changes: Partial<PatchedFixture>,
	): Promise<PatchMutationOutcome> {
		if (this.writableLifecycle() == null)
			return Promise.reject(authorityChanged());
		const fixture = this.store
			.getSnapshot()
			.fixtures.find((candidate) => candidate.fixture_id === fixtureId);
		if (!fixture)
			return Promise.reject(new Error("Patched fixture was not found"));
		const optimistic = changedPatchFixtureCandidate(fixture, changes);
		return this.queuePatch([optimistic], [], (requestId) => {
			const base = this.store.fixtureBefore(requestId, fixtureId);
			if (!base) throw new Error("Patched fixture was not found");
			return [changedPatchFixtureCandidate(base, changes)];
		});
	}

	updatePolicy(
		fixtureId: string,
		action: PatchFixturePolicyAction,
		changes: Partial<PatchedFixture>,
	): Promise<PatchMutationOutcome> {
		const lifecycle = this.writableLifecycle();
		if (lifecycle == null) return Promise.reject(authorityChanged());
		const fixture = this.store
			.getSnapshot()
			.fixtures.find((candidate) => candidate.fixture_id === fixtureId);
		if (!fixture)
			return Promise.reject(new Error("Patched fixture was not found"));
		const requestId = crypto.randomUUID();
		const optimistic = changedPatchFixtureCandidate(fixture, changes);
		const performanceSample =
			frontendPerformanceDiagnostics.beginPatchMutation(requestId, 1);
		this.store.begin(requestId, [optimistic], []);
		performanceSample.optimisticStorePublished();
		return this.enqueueWrite(() =>
			this.runPolicy(
				requestId,
				fixtureId,
				action,
				changes,
				lifecycle,
				performanceSample,
			),
		);
	}

	deleteFixture(fixtureId: string): Promise<PatchMutationOutcome> {
		return this.patchFixtures([], [fixtureId]);
	}

	private queuePatch(
		initial: readonly PatchFixtureCandidate[],
		removeFixtureIds: readonly string[],
		materialize: CandidateMaterializer,
		placements: readonly PatchPlacement[] = [],
	): Promise<PatchMutationOutcome> {
		const lifecycle = this.writableLifecycle();
		if (lifecycle == null) return Promise.reject(authorityChanged());
		const requestId = crypto.randomUUID();
		const performanceSample =
			frontendPerformanceDiagnostics.beginPatchMutation(
				requestId,
				initial.length,
			);
		this.store.begin(requestId, initial, removeFixtureIds);
		performanceSample.optimisticStorePublished();
		return this.enqueueWrite(() =>
			this.runPatch(
				requestId,
				removeFixtureIds,
				materialize,
				placements,
				lifecycle,
				performanceSample,
			),
		);
	}

	private async runPatch(
		requestId: string,
		removeFixtureIds: readonly string[],
		materialize: CandidateMaterializer,
		placements: readonly PatchPlacement[],
		lifecycle: number,
		performanceSample: ReturnType<
			typeof frontendPerformanceDiagnostics.beginPatchMutation
		>,
	): Promise<PatchMutationOutcome> {
		try {
			this.requireActiveLifecycle(lifecycle);
			const outcome = await this.submitPatch(
				requestId,
				removeFixtureIds,
				materialize,
				placements,
				lifecycle,
			);
			performanceSample.responseDecoded();
			this.requireActiveLifecycle(lifecycle);
			this.requireRequestIdentity(requestId, outcome);
			const result = this.store.applyOutcome(requestId, outcome);
			performanceSample.authoritativeStorePublished();
			afterVisiblePaint(performanceSample.visiblePainted);
			if (result === "repair") await this.repair();
			this.requireActiveLifecycle(lifecycle);
			return outcome;
		} catch (reason) {
			if (!this.isActive(lifecycle)) throw authorityChanged();
			return this.failPatch(requestId, asError(reason), lifecycle);
		}
	}

	private async runPolicy(
		requestId: string,
		fixtureId: string,
		action: PatchFixturePolicyAction,
		changes: Partial<PatchedFixture>,
		lifecycle: number,
		performanceSample: ReturnType<
			typeof frontendPerformanceDiagnostics.beginPatchMutation
		>,
	): Promise<PatchMutationOutcome> {
		try {
			for (let conflicts = 0; ; conflicts++) {
				this.requireActiveLifecycle(lifecycle);
				const current = this.store.fixtureBefore(requestId, fixtureId);
				if (!current)
					throw new Error("Patched fixture was not found");
				this.store.replacePending(
					requestId,
					[changedPatchFixtureCandidate(current, changes)],
					[],
				);
				try {
					const outcome = await this.sendPolicyReplaySafe(
						fixtureId,
						requestId,
						action,
						lifecycle,
					);
					performanceSample.responseDecoded();
					this.requireRequestIdentity(requestId, outcome);
					const result = this.store.applyOutcome(requestId, outcome);
					performanceSample.authoritativeStorePublished();
					afterVisiblePaint(performanceSample.visiblePainted);
					if (result === "repair") await this.repair();
					return outcome;
				} catch (reason) {
					const error = asError(reason);
					if (!isConflict(error) || conflicts >= MAX_CONFLICT_RETRIES)
						throw error;
					await this.repair();
				}
			}
		} catch (reason) {
			if (!this.isActive(lifecycle)) throw authorityChanged();
			return this.failPatch(requestId, asError(reason), lifecycle);
		}
	}

	private async sendPolicyReplaySafe(
		fixtureId: string,
		requestId: string,
		action: PatchFixturePolicyAction,
		lifecycle: number,
	): Promise<PatchMutationOutcome> {
		const updatePolicy = this.transport.patchFixturePolicy?.bind(this.transport);
		if (!updatePolicy)
			throw new Error("Patch policy updates are not supported by this transport");
		const expectedRevision = this.requiredRevision();
		try {
			const outcome = await updatePolicy(
				this.showId,
				fixtureId,
				expectedRevision,
				requestId,
				action,
			);
			this.requireActiveLifecycle(lifecycle);
			return outcome;
		} catch (reason) {
			this.requireActiveLifecycle(lifecycle);
			const error = asError(reason);
			if (!isAmbiguous(error)) throw error;
			await this.repair();
			this.requireActiveLifecycle(lifecycle);
			return updatePolicy(
				this.showId,
				fixtureId,
				expectedRevision,
				requestId,
				action,
			);
		}
	}

	private async submitPatch(
		requestId: string,
		removeFixtureIds: readonly string[],
		materialize: CandidateMaterializer,
		placements: readonly PatchPlacement[],
		lifecycle: number,
	): Promise<PatchMutationOutcome> {
		for (let conflicts = 0; ; conflicts++) {
			this.requireActiveLifecycle(lifecycle);
			const candidates = materialize(requestId);
			this.store.replacePending(requestId, candidates, removeFixtureIds);
			const request = patchMutation(
				requestId,
				candidates,
				removeFixtureIds,
				placements,
			);
			try {
				return await this.sendReplaySafe(request, lifecycle);
			} catch (reason) {
				this.requireActiveLifecycle(lifecycle);
				const error = asError(reason);
				if (!isConflict(error) || conflicts >= MAX_CONFLICT_RETRIES)
					throw error;
				await this.repair();
				this.requireActiveLifecycle(lifecycle);
			}
		}
	}

	private async sendReplaySafe(
		request: PatchMutation,
		lifecycle: number,
	): Promise<PatchMutationOutcome> {
		const expectedRevision = this.requiredRevision();
		try {
			const outcome = await this.transport.patchFixtures(
				this.showId,
				expectedRevision,
				request,
			);
			this.requireActiveLifecycle(lifecycle);
			return outcome;
		} catch (reason) {
			this.requireActiveLifecycle(lifecycle);
			const error = asError(reason);
			if (!isAmbiguous(error)) throw error;
			await this.repair();
			this.requireActiveLifecycle(lifecycle);
			const outcome = await this.transport.patchFixtures(
				this.showId,
				expectedRevision,
				request,
			);
			this.requireActiveLifecycle(lifecycle);
			return outcome;
		}
	}

	private requiredRevision(): number {
		const revision = this.store.getSnapshot().patchRevision;
		if (revision == null)
			throw new Error("The authoritative Patch revision is not loaded");
		return revision;
	}

	private requireRequestIdentity(
		requestId: string,
		outcome: PatchMutationOutcome,
	): void {
		if (outcome.requestId !== requestId)
			throw new Error(
				"Patch response request identity does not match the request",
			);
	}

	private async failPatch(
		requestId: string,
		error: Error,
		lifecycle: number,
	): Promise<never> {
		this.requireActiveLifecycle(lifecycle);
		this.store.rollback(requestId, error);
		if (shouldRepair(error) && this.store.getSnapshot().showRevision != null)
			await this.repair().catch(() => undefined);
		this.requireActiveLifecycle(lifecycle);
		this.report(error);
		throw error;
	}

	private async initialize(lifecycle: number): Promise<void> {
		try {
			const snapshot = await this.transport.snapshot(this.showId);
			if (!this.isActive(lifecycle)) return;
			this.store.applySnapshot(snapshot);
			this.openStream(snapshot.cursor);
		} catch (reason) {
			if (!this.isActive(lifecycle)) return;
			const error = asError(reason);
			this.store.setError(error);
			this.report(error);
			throw error;
		}
	}

	private openStream(afterSequence: number): void {
		if (this.stopped) return;
		const lifecycle = this.lifecycle;
		this.stream?.close();
		this.stream = this.transport.subscribe(this.showId, afterSequence, {
			message: (message) => {
				if (this.isActive(lifecycle)) this.handleMessage(message);
			},
			error: (error) => {
				if (!this.isActive(lifecycle)) return;
				this.store.setError(error);
				this.report(error);
				this.requestRepair();
			},
			closed: () => {
				if (this.isActive(lifecycle)) this.scheduleReconnect();
			},
		});
	}

	private handleMessage(message: PatchEventMessage): void {
		switch (message.type) {
			case "event": {
				const result = this.store.applyDelta(message.change, message.sequence);
				if (result === "repair") this.requestRepair();
				return;
			}
			case "gap":
				this.requestRepair();
				return;
			case "error": {
				const error = new Error(message.error);
				this.store.setError(error);
				this.report(error);
				this.requestRepair();
				return;
			}
			case "ready":
			case "repaired":
				return;
		}
	}

	private repair(): Promise<void> {
		if (this.repairPromise) return this.repairPromise;
		const lifecycle = this.lifecycle;
		const repair = this.performRepair(lifecycle).finally(() => {
			if (this.repairPromise === repair) this.repairPromise = null;
		});
		this.repairPromise = repair;
		return repair;
	}

	private requestRepair(): void {
		void this.repair().catch(() => undefined);
	}

	private async performRepair(lifecycle: number): Promise<void> {
		if (!this.isActive(lifecycle)) return;
		this.store.markRepairing();
		try {
			const snapshot = await this.transport.snapshot(this.showId);
			if (!this.isActive(lifecycle)) return;
			this.store.applySnapshot(snapshot);
			if (this.stream) this.stream.repair(snapshot.cursor);
			else this.openStream(snapshot.cursor);
		} catch (reason) {
			if (!this.isActive(lifecycle)) return;
			const error = asError(reason);
			this.store.setError(error);
			this.report(error);
			throw error;
		}
	}

	private scheduleReconnect(): void {
		if (this.stopped || this.reconnectTimer != null) return;
		this.reconnectTimer = globalThis.setTimeout(() => {
			this.reconnectTimer = null;
			const cursor = this.store.getSnapshot().cursor;
			if (cursor == null) this.requestRepair();
			else this.openStream(cursor);
		}, 750);
	}

	private releaseView(): void {
		this.activeViews = Math.max(0, this.activeViews - 1);
		const generation = ++this.releaseGeneration;
		globalThis.queueMicrotask(() => {
			if (generation !== this.releaseGeneration || this.activeViews > 0) return;
			this.stop();
		});
	}

	private enqueueWrite<T>(operation: () => Promise<T>): Promise<T> {
		const result = this.writeQueue.then(operation, operation);
		this.writeQueue = result.then(
			() => undefined,
			() => undefined,
		);
		return result;
	}

	private report(error: Error): void {
		this.onError?.(error);
	}

	private isActive(lifecycle: number): boolean {
		return !this.stopped && this.lifecycle === lifecycle;
	}

	private writableLifecycle(): number | null {
		return !this.stopped && this.store.getSnapshot().status === "ready"
			? this.lifecycle
			: null;
	}

	private requireActiveLifecycle(lifecycle: number): void {
		if (!this.isActive(lifecycle)) throw authorityChanged();
	}
}

function afterVisiblePaint(callback: () => void) {
	const frame = globalThis.requestAnimationFrame;
	if (typeof frame !== "function") {
		callback();
		return;
	}
	frame(() => frame(callback));
}
