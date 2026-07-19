import {
	capturesProgrammerWrites,
	type ProgrammerCaptureModeProjection,
} from "../programmerCaptureMode/contracts";
import type { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import type { ProgrammerValuesScope } from "./contracts";

interface CaptureAuthorityOptions {
	scope: ProgrammerValuesScope;
	store: ProgrammerCaptureModeStore;
	repair(error: Error): Promise<void>;
}

/** Captures one exact-user capture-mode authority generation for a writer. */
export class ProgrammerValuesCaptureAuthority {
	private storeScope: number | null = null;

	constructor(private readonly options: CaptureAuthorityOptions) {}

	claimScope() {
		const state = this.options.store.getSnapshot();
		if (
			state.showId !== this.options.scope.showId ||
			state.userId !== this.options.scope.userId
		)
			return false;
		this.storeScope ??= this.options.store.captureScope();
		return true;
	}

	isScopeCurrent() {
		return (
			this.claimScope() &&
			this.options.store.isScopeCurrent(this.expectedStoreScope())
		);
	}

	readyProjection(): ProgrammerCaptureModeProjection | null {
		const state = this.options.store.getSnapshot();
		if (
			state.status !== "ready" ||
			state.repairRequired ||
			!state.projection ||
			!this.isScopeCurrent()
		)
			return null;
		return state.projection;
	}

	preconditionError(expectedRevision: number) {
		const projection = this.readyProjection();
		if (!projection)
			return new Error("Authoritative Programmer capture mode is unavailable");
		if (projection.revision !== expectedRevision)
			return new Error(
				"Programmer capture mode changed before the write was sent",
			);
		if (capturesProgrammerWrites(projection))
			return new Error(
				"Normal Programmer values are disabled while Preload capture is active",
			);
		return null;
	}

	repair(error: Error) {
		return this.options.repair(error);
	}

	private expectedStoreScope() {
		return this.storeScope ?? -1;
	}
}

export async function awaitProgrammerAuthorityRepairs(
	repairs: readonly [Promise<void>, Promise<void>],
) {
	const results = await Promise.allSettled(repairs);
	const failure = results.find(
		(result): result is PromiseRejectedResult => result.status === "rejected",
	);
	if (failure)
		throw new Error(
			`Programmer authority repair failed: ${asError(failure.reason).message}`,
		);
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
