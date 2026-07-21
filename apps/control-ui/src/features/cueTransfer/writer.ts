import type { ProgrammingInteractionStore } from "../programmingInteraction/store";
import type { ShowObjectsStore } from "../showObjects/store";
import type {
	CueTransferActionOutcome,
	CueTransferActionRequest,
	CueTransferCapability,
	CueTransferConflictRepair,
	CueTransferMode,
	CueTransferScope,
	CueTransferTransport,
} from "./contracts";
import { CueTransferTransportError } from "./contracts";

export interface CueTransferWriterOptions {
	scope: CueTransferScope;
	showStore: ShowObjectsStore;
	programmingStore: ProgrammingInteractionStore;
	transport: CueTransferTransport;
	repair: CueTransferConflictRepair;
	onError?: (error: Error | null) => void;
}

interface MutationScope {
	showGeneration: number;
	programmingScope: number;
	commandLineRevision: number;
	optimisticToken: string;
}

/** Applies one retained pending choice without parsing or replaying its command text. */
export class CueTransferWriter implements CueTransferCapability {
	private active = false;
	private stopped = false;
	private mutation: MutationScope | null = null;

	constructor(private readonly options: CueTransferWriterOptions) {}

	async apply(
		choice: Parameters<CueTransferCapability["apply"]>[0],
		mode: CueTransferMode,
	) {
		if (this.active || this.stopped) return false;
		const mutation = this.begin(choice);
		if (!mutation) return false;
		this.active = true;
		this.mutation = mutation;
		const request: CueTransferActionRequest = {
			requestId: crypto.randomUUID(),
			choiceId: choice.choiceId,
			mode,
			expectedCommandLineRevision: mutation.commandLineRevision,
		};
		try {
			const outcome = await this.send(choice.showRevision, request);
			return this.finish(choice, mode, request, outcome, mutation);
		} catch (reason) {
			return this.fail(asError(reason), mutation);
		} finally {
			if (this.mutation === mutation) this.mutation = null;
			this.active = false;
		}
	}

	stop() {
		this.stopped = true;
		const mutation = this.mutation;
		if (mutation)
			this.options.programmingStore.commit(
				mutation.optimisticToken,
				mutation.programmingScope,
			);
		this.mutation = null;
	}

	private begin(
		choice: Parameters<CueTransferCapability["apply"]>[0],
	): MutationScope | null {
		const show = this.options.showStore.getSnapshot();
		const programming = this.options.programmingStore.getSnapshot();
		const commandLine = programming.commandLine;
		const pending = commandLine?.pendingChoice;
		if (
			show.showId !== this.options.scope.showId ||
			choice.showId !== this.options.scope.showId ||
			programming.showId !== this.options.scope.showId ||
			programming.deskId !== this.options.scope.deskId ||
			!commandLine ||
			!pending ||
			pending.choiceId !== choice.choiceId ||
			pending.showRevision !== choice.showRevision ||
			pending.operation !== choice.operation
		)
			return null;
		const programmingScope = this.options.programmingStore.captureScope();
		const optimisticToken =
			this.options.programmingStore.beginOptimisticCommandLine(
				{
					text: commandLine.target,
					pristine: true,
					pendingChoice: null,
				},
				programmingScope,
			);
		return optimisticToken
			? {
					showGeneration: show.authorityGeneration,
					programmingScope,
					commandLineRevision: commandLine.revision,
					optimisticToken,
				}
			: null;
	}

	private send(
		expectedShowRevision: number,
		request: CueTransferActionRequest,
	) {
		const apply = () =>
			this.options.transport.apply(
				this.options.scope.showId,
				expectedShowRevision,
				request,
			);
		return apply().catch((reason) => {
			if (!(reason instanceof CueTransferTransportError) || !reason.retryable)
				throw reason;
			return apply();
		});
	}

	private finish(
		choice: Parameters<CueTransferCapability["apply"]>[0],
		mode: CueTransferMode,
		request: CueTransferActionRequest,
		outcome: CueTransferActionOutcome,
		mutation: MutationScope,
	) {
		if (!this.isCurrent(mutation)) return false;
		assertOutcome(choice, mode, request, outcome);
		if (
			!this.options.programmingStore.commitCommandLine(
				mutation.optimisticToken,
				outcome.commandLine,
				mutation.programmingScope,
			)
		)
			return false;
		this.options.showStore.installObjects(
			this.options.scope.showId,
			outcome.projections.map((projection) => ({
				kind: "cue_list" as const,
				objectId: projection.objectId,
				object: {
					kind: "cue_list" as const,
					id: projection.objectId,
					revision: projection.objectRevision,
					updated_at: "",
					body: projection.body,
				},
			})),
			outcome.showEventSequence,
			outcome.showRevision,
			"seal",
		);
		this.options.onError?.(
			outcome.persistenceWarning ? new Error(outcome.persistenceWarning) : null,
		);
		return true;
	}

	private async fail(error: Error, mutation: MutationScope) {
		if (!this.isCurrent(mutation)) return false;
		this.options.programmingStore.rollback(
			mutation.optimisticToken,
			error,
			mutation.programmingScope,
		);
		await this.repairConflict(error, mutation);
		if (!this.isCurrent(mutation)) return false;
		this.options.onError?.(error);
		return false;
	}

	private async repairConflict(error: Error, mutation: MutationScope) {
		if (!(error instanceof CueTransferTransportError) || error.status !== 409)
			return;
		if (error.currentRevision != null)
			this.options.showStore.installShowRevision(
				this.options.scope.showId,
				error.currentRevision,
				mutation.showGeneration,
			);
		const repairs: Promise<void>[] = [];
		if (error.currentRevision != null)
			repairs.push(this.repairCueLists(mutation));
		if (error.currentRelatedRevision != null || error.currentRevision == null)
			repairs.push(this.repairCommandLine(mutation));
		await Promise.all(repairs);
	}

	private async repairCueLists(mutation: MutationScope) {
		const before = this.options.showStore.getSnapshot();
		const eventFloor = before.eventSequence ?? 0;
		try {
			const snapshot = await this.options.repair.loadCueLists(
				this.options.scope.showId,
			);
			if (!this.isCurrent(mutation)) return;
			this.options.showStore.setCollection(
				this.options.scope.showId,
				"cue_list",
				snapshot.objects,
				eventFloor,
				snapshot.showRevision,
			);
		} catch {
			// Preserve the original revision conflict as the actionable error.
		}
	}

	private async repairCommandLine(mutation: MutationScope) {
		try {
			const commandLine = await this.options.repair.loadCommandLine(
				this.options.scope.deskId,
			);
			if (!this.isCurrent(mutation)) return;
			this.options.programmingStore.installCommandLineRepair(
				commandLine,
				mutation.programmingScope,
			);
		} catch {
			// Preserve the original revision conflict as the actionable error.
		}
	}

	private isCurrent(mutation: MutationScope) {
		const show = this.options.showStore.getSnapshot();
		return (
			!this.stopped &&
			show.showId === this.options.scope.showId &&
			show.authorityGeneration === mutation.showGeneration &&
			this.options.programmingStore.isScopeCurrent(mutation.programmingScope)
		);
	}
}

function assertOutcome(
	choice: Parameters<CueTransferCapability["apply"]>[0],
	mode: CueTransferMode,
	request: CueTransferActionRequest,
	outcome: CueTransferActionOutcome,
) {
	if (outcome.requestId !== request.requestId)
		throw new Error("Cue transfer response request ID does not match");
	if (outcome.choiceId !== request.choiceId)
		throw new Error("Cue transfer response choice ID does not match");
	if (outcome.showId !== choice.showId)
		throw new Error("Cue transfer response Show ID does not match");
	if (
		outcome.summary.operation !== choice.operation ||
		outcome.summary.mode !== mode
	)
		throw new Error("Cue transfer response does not match the selected choice");
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
