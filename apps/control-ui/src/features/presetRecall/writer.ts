import { normalizePresetFamily } from "../../presetFamilies";
import {
	capturesProgrammerWrites,
	type ProgrammerCaptureModeProjection,
} from "../programmerCaptureMode/contracts";
import type { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import type { ProgrammerValuesProjection } from "../programmerValues/contracts";
import type { ProgrammerValuesStore } from "../programmerValues/store";
import { ProgrammerValuesProtocolError } from "../programmerValues/transport";
import type { ProgrammingInteractionStore } from "../programmingInteraction/store";
import type { ShowObject } from "../showObjects/contracts";
import type { ShowObjectsStore } from "../showObjects/store";
import type { ShowObjectAuthorityStamp } from "../showObjects/storeTypes";
import type {
	PresetRecallActions,
	PresetRecallOutcome,
	PresetRecallRequest,
	PresetRecallScope,
	PresetRecallTransport,
	RecallPresetInput,
} from "./contracts";
import { PresetRecallTransportError } from "./contracts";

interface PresetAuthoritySnapshot {
	object: ShowObject<"preset"> | null;
	showRevision: number;
}

export interface PresetRecallWriterOptions {
	scope: PresetRecallScope;
	showStore: ShowObjectsStore;
	valuesStore: ProgrammerValuesStore;
	captureModeStore: ProgrammerCaptureModeStore;
	programmingStore: ProgrammingInteractionStore;
	transport: PresetRecallTransport;
	loadPreset(
		showId: string,
		objectId: string,
	): Promise<PresetAuthoritySnapshot>;
	repairValues(error: Error): Promise<void>;
	repairCaptureMode(error: Error): Promise<void>;
	repairSelection(error: Error): Promise<void>;
	onError?: (error: Error | null) => void;
}

interface RecallAuthority {
	showGeneration: number;
	presetStamp: ShowObjectAuthorityStamp<"preset">;
	valuesScope: number;
	captureModeScope: number;
	programmingScope: number;
	request: PresetRecallRequest;
}

/** One exact-authority Preset recall with no client-authored values expansion. */
export class PresetRecallWriter implements PresetRecallActions {
	private active = false;
	private stopped = false;

	constructor(private readonly options: PresetRecallWriterOptions) {}

	async recall(input: RecallPresetInput) {
		if (this.stopped) return null;
		if (this.active)
			return this.refuse("A Preset recall is already in progress");
		let authority: RecallAuthority;
		try {
			authority = this.capture(input);
		} catch (reason) {
			return this.refuse(asError(reason).message);
		}
		this.active = true;
		try {
			const outcome = await this.send(authority);
			if (!this.isCurrent(authority)) return null;
			assertOutcome(authority.request, outcome, this.options.scope.userId);
			if (!(await this.reconcile(authority, outcome))) return null;
			this.options.onError?.(
				outcome.warning ? new Error(outcome.warning) : null,
			);
			return outcome;
		} catch (reason) {
			return this.fail(asError(reason), authority);
		} finally {
			this.active = false;
		}
	}

	stop() {
		this.stopped = true;
	}

	private capture(input: RecallPresetInput): RecallAuthority {
		const show = this.options.showStore.getSnapshot();
		const preset = show.presets.find(({ id }) => id === input.objectId);
		const showRevision = show.showRevision;
		if (!this.showReady(show, preset, input))
			throw new Error("Authoritative Preset authority is unavailable");
		if (showRevision === null)
			throw new Error("Authoritative Show revision is unavailable");
		const valuesScope = this.options.valuesStore.captureScope();
		const captureModeScope = this.options.captureModeStore.captureScope();
		const programmingScope = this.options.programmingStore.captureScope();
		const values = this.readyValues(valuesScope);
		const captureMode = this.readyCaptureMode(captureModeScope);
		const selection = this.readySelection(programmingScope);
		const presetStamp = this.options.showStore.captureObjectAuthority(
			this.options.scope.showId,
			"preset",
			input.objectId,
		);
		if (!values || !captureMode || !selection || !presetStamp || !preset)
			throw new Error("Preset recall authority is still loading");
		return {
			showGeneration: show.authorityGeneration,
			presetStamp,
			valuesScope,
			captureModeScope,
			programmingScope,
			request: request(
				input,
				preset,
				showRevision,
				values,
				captureMode,
				selection,
			),
		};
	}

	private showReady(
		show: ReturnType<ShowObjectsStore["getSnapshot"]>,
		preset: ShowObject<"preset"> | undefined,
		input: RecallPresetInput,
	) {
		return Boolean(
			show.showId === this.options.scope.showId &&
				show.showRevision !== null &&
				show.readyCollections.has("preset") &&
				preset &&
				preset.body.number === input.address.number &&
				normalizePresetFamily(preset.body.family) === input.address.family &&
				!show.pendingObjectKeys.has(`preset:${input.objectId}`),
		);
	}

	private readyValues(scope: number): ProgrammerValuesProjection | null {
		const state = this.options.valuesStore.getSnapshot();
		if (
			state.showId !== this.options.scope.showId ||
			state.userId !== this.options.scope.userId ||
			state.status !== "ready" ||
			state.repairRequired ||
			state.pendingRequestIds.length > 0 ||
			!state.projection ||
			!this.options.valuesStore.isScopeCurrent(scope)
		)
			return null;
		return state.projection;
	}

	private readyCaptureMode(
		scope: number,
	): ProgrammerCaptureModeProjection | null {
		const state = this.options.captureModeStore.getSnapshot();
		if (
			state.showId !== this.options.scope.showId ||
			state.userId !== this.options.scope.userId ||
			state.status !== "ready" ||
			state.repairRequired ||
			!state.projection ||
			!this.options.captureModeStore.isScopeCurrent(scope) ||
			capturesProgrammerWrites(state.projection)
		)
			return null;
		return state.projection;
	}

	private readySelection(scope: number) {
		const state = this.options.programmingStore.getSnapshot();
		if (
			state.showId !== this.options.scope.showId ||
			state.deskId !== this.options.scope.deskId ||
			state.status !== "ready" ||
			!state.selection ||
			state.selection.selected.length === 0 ||
			state.pendingCapabilities.has("selection") ||
			!this.options.programmingStore.isScopeCurrent(scope)
		)
			return null;
		return state.selection;
	}

	private async send(authority: RecallAuthority) {
		try {
			return await this.options.transport.recall(
				this.options.scope,
				authority.request,
			);
		} catch (reason) {
			if (
				!(reason instanceof PresetRecallTransportError) ||
				!reason.retryable ||
				!this.isCurrent(authority)
			)
				throw reason;
			return this.options.transport.recall(
				this.options.scope,
				authority.request,
			);
		}
	}

	private async reconcile(
		authority: RecallAuthority,
		outcome: PresetRecallOutcome,
	) {
		this.options.showStore.installShowRevision(
			this.options.scope.showId,
			outcome.showRevision,
			authority.showGeneration,
		);
		if (!(await this.reconcileValues(authority, outcome))) return false;
		if (!(await this.reconcileSelection(authority, outcome))) return false;
		return this.isCurrent(authority);
	}

	private async reconcileValues(
		authority: RecallAuthority,
		outcome: PresetRecallOutcome,
	) {
		if (!outcome.projection || outcome.eventSequence === null) return true;
		try {
			if (
				!this.options.valuesStore.applyProjection(
					outcome.projection,
					outcome.eventSequence,
					authority.valuesScope,
				)
			)
				return false;
		} catch (reason) {
			await this.options.repairValues(asError(reason));
		}
		return this.valuesRevisionAtLeast(authority, outcome.programmerRevision);
	}

	private async reconcileSelection(
		authority: RecallAuthority,
		outcome: PresetRecallOutcome,
	) {
		if (outcome.interactionEventSequence === null) return true;
		await Promise.resolve();
		if (this.selectionObserved(authority, outcome)) return true;
		await this.options.repairSelection(
			new Error("Preset recall selection event requires snapshot repair"),
		);
		return this.selectionObserved(authority, outcome);
	}

	private valuesRevisionAtLeast(authority: RecallAuthority, revision: number) {
		return (
			this.isCurrent(authority) &&
			(this.options.valuesStore.authoritativeRevision(authority.valuesScope) ??
				-1) >= revision
		);
	}

	private selectionObserved(
		authority: RecallAuthority,
		outcome: PresetRecallOutcome,
	) {
		if (!this.isCurrent(authority)) return false;
		const state = this.options.programmingStore.getSnapshot();
		return (
			(state.eventSequence ?? -1) >= (outcome.interactionEventSequence ?? 0) &&
			(state.selection?.revision ?? -1) >= outcome.selectionRevision
		);
	}

	private async fail(error: Error, authority: RecallAuthority) {
		if (!this.isCurrent(authority)) return null;
		if (error instanceof PresetRecallTransportError && error.status === 409)
			await this.repairConflict(error, authority);
		if (!this.isCurrent(authority)) return null;
		this.options.onError?.(error);
		return null;
	}

	private async repairConflict(error: Error, authority: RecallAuthority) {
		await Promise.allSettled([
			this.options.repairValues(error),
			this.options.repairCaptureMode(error),
			this.options.repairSelection(error),
			this.repairPreset(authority),
		]);
	}

	private async repairPreset(authority: RecallAuthority) {
		const snapshot = await this.options.loadPreset(
			this.options.scope.showId,
			authority.request.presetId,
		);
		if (!this.isCurrent(authority)) return;
		this.options.showStore.installObjectIfAuthorityUnchanged(
			authority.presetStamp,
			snapshot.object,
		);
		this.options.showStore.installShowRevision(
			this.options.scope.showId,
			snapshot.showRevision,
			authority.showGeneration,
		);
	}

	private isCurrent(authority: RecallAuthority) {
		const show = this.options.showStore.getSnapshot();
		return (
			!this.stopped &&
			show.showId === this.options.scope.showId &&
			show.authorityGeneration === authority.showGeneration &&
			this.options.valuesStore.isScopeCurrent(authority.valuesScope) &&
			this.options.captureModeStore.isScopeCurrent(
				authority.captureModeScope,
			) &&
			this.options.programmingStore.isScopeCurrent(authority.programmingScope)
		);
	}

	private refuse(message: string): null {
		this.options.onError?.(new Error(message));
		return null;
	}
}

function request(
	input: RecallPresetInput,
	preset: ShowObject<"preset">,
	showRevision: number,
	values: ProgrammerValuesProjection,
	captureMode: ProgrammerCaptureModeProjection,
	selection: { revision: number; selected: readonly string[] },
): PresetRecallRequest {
	return {
		requestId: crypto.randomUUID(),
		presetId: preset.id,
		address: input.address,
		expectedPresetRevision: preset.revision,
		expectedShowRevision: showRevision,
		expectedProgrammerRevision: values.revision,
		expectedCaptureModeRevision: captureMode.revision,
		expectedSelectionRevision: selection.revision,
		selectedFixtureCount: selection.selected.length,
	};
}

function assertOutcome(
	request: PresetRecallRequest,
	outcome: PresetRecallOutcome,
	userId: string,
) {
	if (outcome.requestId !== request.requestId)
		throw new Error("Preset recall response request ID does not match");
	if (outcome.preset.id !== request.presetId)
		throw new Error("Preset recall response object does not match");
	if (outcome.projection && outcome.projection.userId !== userId)
		throw new ProgrammerValuesProtocolError(
			"Preset recall returned another user's Programmer values",
			outcome.eventSequence,
		);
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
