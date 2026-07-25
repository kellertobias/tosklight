import type {
	UpdateMode,
	UpdateSettings,
	UpdateTargetFilter,
	UpdateTargetRequest,
} from "../../api/types";
import type { ShowObject, ShowObjectKind } from "../showObjects/contracts";
import type { ShowObjectsStore } from "../showObjects/store";
import {
	mutationResult,
	previewAuthority,
	programmingUpdateTarget,
	targetsAuthority,
} from "./authority";
import { repairProgrammingUpdateConflict } from "./conflictRepair";
import type {
	ProgrammingUpdateActionOutcome,
	ProgrammingUpdateActionRequest,
	ProgrammingUpdateCapability,
	ProgrammingUpdateMutationResult,
	ProgrammingUpdatePreviewRequest,
	ProgrammingUpdateScope,
	ProgrammingUpdateTargetsRequest,
	ProgrammingUpdateTransport,
	UpdatePreviewAuthority,
} from "./contracts";
import {
	asError,
	assertConfirmedProjection,
	directObject,
	needsExactCue,
	StaleProgrammingUpdateAuthority,
	transportFailure,
} from "./writerSupport";

export interface ProgrammingUpdateWriterOptions {
	scopeKey: string;
	scope: ProgrammingUpdateScope;
	store: ShowObjectsStore;
	transport: ProgrammingUpdateTransport;
	loadObject<K extends ShowObjectKind>(
		showId: string,
		kind: K,
		objectId: string,
	): Promise<ShowObject<K> | null>;
}

type QueryKind = "preview" | "settings" | "targets";

/** Owns Update query epochs and serializes each portable mutation. */
export class ProgrammingUpdateWriter implements ProgrammingUpdateCapability {
	readonly scopeKey: string;
	private stopped = false;
	private mutationTail: Promise<void> = Promise.resolve();
	private readonly queryEpochs: Record<QueryKind, number> = {
		preview: 0,
		settings: 0,
		targets: 0,
	};

	constructor(private readonly options: ProgrammingUpdateWriterOptions) {
		this.scopeKey = options.scopeKey;
	}

	async loadSettings() {
		const query = this.beginQuery("settings");
		try {
			const response = await this.options.transport.loadSettings(
				this.options.scope.deskId,
			);
			return this.isQueryCurrent(query) ? response.settings : null;
		} catch (reason) {
			return this.finishQueryError(query, reason);
		}
	}

	async saveSettings(settings: UpdateSettings) {
		const query = this.beginQuery("settings");
		try {
			const response = await this.retry(() =>
				this.options.transport.saveSettings(
					this.options.scope.deskId,
					settings,
				),
			);
			return this.isQueryCurrent(query) ? response.settings : null;
		} catch (reason) {
			return this.finishQueryError(query, reason);
		}
	}

	async preview(target: UpdateTargetRequest, mode: UpdateMode) {
		const query = this.beginQuery("preview");
		try {
			const authority = await this.loadPreview(target, mode, query.generation);
			return this.isQueryCurrent(query) ? authority : null;
		} catch (reason) {
			return this.finishQueryError(query, reason);
		}
	}

	async targets(filter: UpdateTargetFilter) {
		const query = this.beginQuery("targets");
		const request: ProgrammingUpdateTargetsRequest = {
			request_id: crypto.randomUUID(),
			filter,
		};
		try {
			const response = await this.options.transport.targets(
				this.options.scope.showId,
				request,
			);
			return this.isQueryCurrent(query)
				? targetsAuthority(response, this.scopeKey)
				: null;
		} catch (reason) {
			return this.finishQueryError(query, reason);
		}
	}

	confirm(authority: UpdatePreviewAuthority) {
		return this.enqueue(() => this.confirmNow(authority));
	}

	applyDirect(target: UpdateTargetRequest, mode: UpdateMode) {
		return this.enqueue(() => this.applyDirectNow(target, mode));
	}

	stop() {
		this.stopped = true;
		for (const kind of Object.keys(this.queryEpochs) as QueryKind[])
			this.queryEpochs[kind] += 1;
	}

	private async confirmNow(authority: UpdatePreviewAuthority) {
		if (
			authority.showId !== this.options.scope.showId ||
			authority.scopeKey !== this.scopeKey ||
			this.stopped
		)
			return null;
		const generation = this.generation();
		const token = this.options.store.beginPending(
			authority.showId,
			authority.object.kind,
			authority.object.object_id,
		);
		const request: ProgrammingUpdateActionRequest = {
			request_id: crypto.randomUUID(),
			action: {
				type: "confirm_preview",
				target: authority.requestTarget,
				mode: authority.preview.mode,
				expected_object_revision: authority.object.object_revision,
				expected_programmer_revision: authority.programmerRevision,
			},
		};
		try {
			const outcome = await this.sendAction(authority.showRevision, request);
			if (!this.isCurrent(generation)) return this.abandon(token);
			assertConfirmedProjection(outcome, authority.object);
			return this.settle(token, outcome, generation);
		} catch (reason) {
			if (!this.isCurrent(generation)) return this.abandon(token);
			this.options.store.abandon(token);
			await repairProgrammingUpdateConflict({
				error: asError(reason),
				object: authority.object,
				generation,
				...this.repairOptions(),
			});
			throw reason;
		}
	}

	private async applyDirectNow(target: UpdateTargetRequest, mode: UpdateMode) {
		if (this.stopped) return null;
		const generation = this.generation();
		let requestTarget = programmingUpdateTarget(target);
		let object = directObject(requestTarget);
		let revision = this.currentShowRevision();
		if (revision == null || needsExactCue(requestTarget)) {
			const authority = await this.loadPreview(target, mode, generation);
			if (!this.isCurrent(generation)) return null;
			requestTarget = authority.requestTarget;
			object = authority.object;
			revision = authority.showRevision;
		}
		const request: ProgrammingUpdateActionRequest = {
			request_id: crypto.randomUUID(),
			action: { type: "apply_direct", target: requestTarget, mode },
		};
		try {
			const outcome = await this.sendAction(revision, request);
			if (!this.isCurrent(generation)) return null;
			const result = mutationResult(outcome);
			this.install(result);
			return result;
		} catch (reason) {
			if (!this.isCurrent(generation)) return null;
			await repairProgrammingUpdateConflict({
				error: asError(reason),
				object,
				generation,
				...this.repairOptions(),
				...(object
					? {}
					: {
							resolveObject: async () => {
								const authority = await this.loadPreview(
									target,
									mode,
									generation,
								);
								return {
									object: authority.object,
									showRevision: authority.showRevision,
								};
							},
						}),
			});
			throw reason;
		}
	}

	private async loadPreview(
		target: UpdateTargetRequest,
		mode: UpdateMode,
		generation: number,
	) {
		const requestTarget = programmingUpdateTarget(target);
		const request: ProgrammingUpdatePreviewRequest = {
			request_id: crypto.randomUUID(),
			target: requestTarget,
			mode,
		};
		const response = await this.options.transport.preview(
			this.options.scope.showId,
			request,
		);
		if (!this.isCurrent(generation))
			throw new StaleProgrammingUpdateAuthority();
		return previewAuthority(response, requestTarget, this.scopeKey);
	}

	private sendAction(
		revision: number,
		request: ProgrammingUpdateActionRequest,
	) {
		return this.retry(() =>
			this.options.transport.apply(
				this.options.scope.showId,
				revision,
				request,
			),
		);
	}

	private settle(
		token: string,
		outcome: ProgrammingUpdateActionOutcome,
		generation: number,
	) {
		const result = mutationResult(outcome);
		const settled = this.options.store.settlePending(
			token,
			{
				objectId: result.object.id,
				revision: result.object.revision,
				object: result.object as never,
			},
			result.showRevision,
			result.eventSequence,
			generation,
		);
		return settled ? result : null;
	}

	private install(result: ProgrammingUpdateMutationResult) {
		this.options.store.installObjects(
			this.options.scope.showId,
			[
				{
					kind: result.object.kind,
					objectId: result.object.id,
					object: result.object as never,
				},
			],
			result.eventSequence,
			result.showRevision,
			"seal",
		);
	}

	private repairOptions() {
		return {
			showId: this.options.scope.showId,
			store: this.options.store,
			loadObject: this.options.loadObject,
		};
	}

	private retry<T>(operation: () => Promise<T>) {
		return operation().catch((reason) => {
			if (!transportFailure(asError(reason))?.retryable) throw reason;
			return operation();
		});
	}

	private enqueue(
		operation: () => Promise<ProgrammingUpdateMutationResult | null>,
	) {
		const result = this.mutationTail.then(operation, operation);
		this.mutationTail = result.then(
			() => undefined,
			() => undefined,
		);
		return result;
	}

	private beginQuery(kind: QueryKind) {
		return {
			kind,
			epoch: ++this.queryEpochs[kind],
			generation: this.generation(),
		};
	}

	private isQueryCurrent(
		query: ReturnType<ProgrammingUpdateWriter["beginQuery"]>,
	) {
		return (
			this.queryEpochs[query.kind] === query.epoch &&
			this.isCurrent(query.generation)
		);
	}

	private finishQueryError(
		query: ReturnType<ProgrammingUpdateWriter["beginQuery"]>,
		reason: unknown,
	): null {
		if (
			!this.isQueryCurrent(query) ||
			reason instanceof StaleProgrammingUpdateAuthority
		)
			return null;
		throw reason;
	}

	private currentShowRevision() {
		return (
			this.options.store.getSnapshot().showRevision ??
			this.options.scope.initialShowRevision
		);
	}

	private generation() {
		return this.options.store.getSnapshot().authorityGeneration;
	}

	private isCurrent(generation: number) {
		const snapshot = this.options.store.getSnapshot();
		return (
			!this.stopped &&
			snapshot.showId === this.options.scope.showId &&
			snapshot.authorityGeneration === generation
		);
	}

	private abandon(token: string): null {
		this.options.store.abandon(token);
		return null;
	}
}
