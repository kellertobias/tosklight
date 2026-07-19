import type { PatchedFixture } from "../../api/types";
import type {
	PatchEventMessage,
	PatchMutation,
	PatchMutationOutcome,
} from "./contracts";
import {
	changedPatchFixtureCandidate,
	type PatchDefinitionResolver,
	type PatchFixtureCandidate,
} from "./model";
import {
	type PatchEventStream,
	type PatchTransport,
	PatchTransportError,
} from "./transport";
import { PatchStore } from "./store";

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
	private reconnectTimer: ReturnType<typeof globalThis.setTimeout> | null = null;

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
		this.stopped = false;
		const lifecycle = ++this.lifecycle;
		this.startPromise = this.initialize(lifecycle).catch((error) => {
			if (this.isActive(lifecycle)) this.startPromise = null;
			throw error;
		});
		return this.startPromise;
	}

	stop(): void {
		if (this.stopped) return;
		this.stopped = true;
		this.lifecycle++;
		this.startPromise = null;
		this.repairPromise = null;
		if (this.reconnectTimer != null)
			globalThis.clearTimeout(this.reconnectTimer);
		this.reconnectTimer = null;
		this.stream?.close();
		this.stream = null;
	}

	patchFixtures(
		candidates: readonly PatchFixtureCandidate[],
		removeFixtureIds: readonly string[] = [],
	): Promise<PatchMutationOutcome> {
		if (!candidates.length && !removeFixtureIds.length)
			return Promise.reject(
				new Error("A Patch mutation must change at least one fixture"),
			);
		return this.queuePatch(candidates, removeFixtureIds, () => candidates);
	}

	updateFixture(
		fixtureId: string,
		changes: Partial<PatchedFixture>,
	): Promise<PatchMutationOutcome> {
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

	deleteFixture(fixtureId: string): Promise<PatchMutationOutcome> {
		return this.patchFixtures([], [fixtureId]);
	}

	private queuePatch(
		initial: readonly PatchFixtureCandidate[],
		removeFixtureIds: readonly string[],
		materialize: CandidateMaterializer,
	): Promise<PatchMutationOutcome> {
		const requestId = crypto.randomUUID();
		this.store.begin(requestId, initial, removeFixtureIds);
		return this.enqueueWrite(() =>
			this.runPatch(requestId, removeFixtureIds, materialize),
		);
	}

	private async runPatch(
		requestId: string,
		removeFixtureIds: readonly string[],
		materialize: CandidateMaterializer,
	): Promise<PatchMutationOutcome> {
		try {
			await this.start();
			const outcome = await this.submitPatch(
				requestId,
				removeFixtureIds,
				materialize,
			);
			this.requireRequestIdentity(requestId, outcome);
			const result = this.store.applyOutcome(requestId, outcome);
			if (result === "repair") await this.repair();
			return outcome;
		} catch (reason) {
			return this.failPatch(requestId, asError(reason));
		}
	}

	private async submitPatch(
		requestId: string,
		removeFixtureIds: readonly string[],
		materialize: CandidateMaterializer,
	): Promise<PatchMutationOutcome> {
		for (let conflicts = 0; ; conflicts++) {
			const candidates = materialize(requestId);
			this.store.replacePending(requestId, candidates, removeFixtureIds);
			const request = patchMutation(requestId, candidates, removeFixtureIds);
			try {
				return await this.sendReplaySafe(request);
			} catch (reason) {
				const error = asError(reason);
				if (!isConflict(error) || conflicts >= MAX_CONFLICT_RETRIES)
					throw error;
				await this.repair();
			}
		}
	}

	private async sendReplaySafe(
		request: PatchMutation,
	): Promise<PatchMutationOutcome> {
		const expectedRevision = this.requiredRevision();
		try {
			return await this.transport.patchFixtures(
				this.showId,
				expectedRevision,
				request,
			);
		} catch (reason) {
			const error = asError(reason);
			if (!isAmbiguous(error)) throw error;
			await this.repair();
			return this.transport.patchFixtures(
				this.showId,
				expectedRevision,
				request,
			);
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
	): Promise<never> {
		this.store.rollback(requestId, error);
		if (shouldRepair(error) && this.store.getSnapshot().showRevision != null)
			await this.repair().catch(() => undefined);
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
				const result = this.store.applyDelta(
					message.change,
					message.sequence,
				);
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
}

function asError(reason: unknown): Error {
	return reason instanceof Error ? reason : new Error(String(reason));
}

function patchMutation(
	requestId: string,
	candidates: readonly PatchFixtureCandidate[],
	removeFixtureIds: readonly string[],
): PatchMutation {
	return {
		requestId,
		fixtures: candidates.map((candidate) => candidate.input),
		removeFixtureIds: [...removeFixtureIds],
	};
}

function isConflict(error: Error): boolean {
	return (
		error instanceof PatchTransportError &&
		(error.status === 409 || error.currentRevision != null)
	);
}

function isAmbiguous(error: Error): boolean {
	return (
		!(error instanceof PatchTransportError) ||
		error.retryable ||
		error.status >= 500
	);
}

function shouldRepair(error: Error): boolean {
	return (
		!(error instanceof PatchTransportError) ||
		isConflict(error) ||
		error.retryable ||
		error.status >= 500
	);
}
