import { normalizePresetFamily } from "../../presetFamilies";
import {
	capturesProgrammerWrites,
	type ProgrammerCaptureModeProjection,
} from "../programmerCaptureMode/contracts";
import type { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import type { ProgrammerPreloadValuesProjection } from "../programmerPreloadValues/contracts";
import type { ProgrammerPreloadValuesStore } from "../programmerPreloadValues/store";
import { ProgrammerPreloadValuesProtocolError } from "../programmerPreloadValues/transport";
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
	preloadValuesStore: ProgrammerPreloadValuesStore;
	captureModeStore: ProgrammerCaptureModeStore;
	programmingStore: ProgrammingInteractionStore;
	transport: PresetRecallTransport;
	loadPreset(
		showId: string,
		objectId: string,
	): Promise<PresetAuthoritySnapshot>;
	repairValues(error: Error): Promise<void>;
	repairPreloadValues(error: Error): Promise<void>;
	repairCaptureMode(error: Error): Promise<void>;
	repairSelection(error: Error): Promise<void>;
	onError?: (error: Error | null) => void;
}

interface RecallAuthority {
	showGeneration: number;
	presetStamp: ShowObjectAuthorityStamp<"preset">;
	valuesScope: number;
	preloadValuesScope: number;
	captureModeScope: number;
	programmingScope: number;
	target: "programmer" | "preload";
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
		this.active = true;
		let authority: RecallAuthority | null = null;
		let repairedConflict = false;
		try {
			for (let attempt = 0; attempt < 2; attempt += 1) {
				authority = await this.captureWhenReady(input);
				try {
					const outcome = await this.send(authority);
					if (!this.isCurrent(authority)) return null;
					assertOutcome(authority.request, outcome, authority.target);
					if (!(await this.reconcile(authority, outcome))) return null;
					this.options.onError?.(
						outcome.warning ? new Error(outcome.warning) : null,
					);
					return outcome;
				} catch (reason) {
					const error = asError(reason);
					if (
						attempt === 0 &&
						error instanceof PresetRecallTransportError &&
						error.kind === "conflict"
					) {
						if (error.currentRelatedRevision !== null) {
							this.options.showStore.installShowRevision(
								this.options.scope.showId,
								error.currentRelatedRevision,
								authority.showGeneration,
							);
						} else {
							await this.repairConflict(error, authority);
						}
						repairedConflict = true;
						authority = null;
						continue;
					}
					throw error;
				}
			}
			return null;
		} catch (reason) {
			if (!authority) return this.refuse(asError(reason).message);
			if (repairedConflict) {
				this.options.onError?.(asError(reason));
				return null;
			}
			return this.fail(asError(reason), authority);
		} finally {
			this.active = false;
		}
	}

	private async captureWhenReady(input: RecallPresetInput) {
		const deadline = performance.now() + 1_000;
		for (;;) {
			try {
				return this.capture(input);
			} catch (reason) {
				const error = asError(reason);
				if (
					error.message !== "Preset recall authority is still loading" ||
					performance.now() >= deadline ||
					this.stopped
				)
					throw error;
				await new Promise((resolve) => setTimeout(resolve, 25));
			}
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
		const preloadValuesScope = this.options.preloadValuesStore.captureScope();
		const captureModeScope = this.options.captureModeStore.captureScope();
		const programmingScope = this.options.programmingStore.captureScope();
		const captureMode = this.readyCaptureMode(captureModeScope);
		const target = capturesProgrammerWrites(captureMode)
			? "preload"
			: "programmer";
		const values = this.readyValues(valuesScope);
		const preloadValues =
			target === "preload" ? this.readyPreloadValues(preloadValuesScope) : null;
		const selection = this.readySelection(programmingScope);
		const presetStamp = this.options.showStore.captureObjectAuthority(
			this.options.scope.showId,
			"preset",
			input.objectId,
		);
		if (
			!values ||
			(target === "preload" && !preloadValues) ||
			!captureMode ||
			!selection ||
			!presetStamp ||
			!preset
		)
			throw new Error("Preset recall authority is still loading");
		return {
			showGeneration: show.authorityGeneration,
			presetStamp,
			valuesScope,
			preloadValuesScope,
			captureModeScope,
			programmingScope,
			target,
			request: request(
				input,
				preset,
				showRevision,
				values,
				preloadValues,
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
			state.sessionId !== this.options.scope.sessionId ||
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
			state.sessionId !== this.options.scope.sessionId ||
			state.status !== "ready" ||
			state.repairRequired ||
			!state.projection ||
			!this.options.captureModeStore.isScopeCurrent(scope)
		)
			return null;
		return state.projection;
	}

	private readyPreloadValues(
		scope: number,
	): ProgrammerPreloadValuesProjection | null {
		const state = this.options.preloadValuesStore.getSnapshot();
		if (
			state.showId !== this.options.scope.showId ||
			state.sessionId !== this.options.scope.sessionId ||
			state.status !== "ready" ||
			state.repairRequired ||
			state.pendingRequestIds.length > 0 ||
			!state.projection ||
			!this.options.preloadValuesStore.isScopeCurrent(scope)
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
			state.pendingCapabilities.has("selection") ||
			!this.options.programmingStore.isScopeCurrent(scope)
		)
			return null;
		return state.selection;
	}

	private async send(authority: RecallAuthority) {
		return this.options.transport.recall(this.options.scope, authority.request);
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
		if (outcome.target === "preload")
			return this.reconcilePreloadValues(authority, outcome);
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

	private async reconcilePreloadValues(
		authority: RecallAuthority,
		outcome: PresetRecallOutcome & { target: "preload"; status: "changed" },
	) {
		if (!outcome.projection || outcome.eventSequence === null) return true;
		try {
			if (
				!this.options.preloadValuesStore.applyProjection(
					outcome.projection,
					outcome.eventSequence,
					authority.preloadValuesScope,
				)
			)
				return false;
		} catch (reason) {
			await this.options.repairPreloadValues(asError(reason));
		}
		return this.preloadValuesRevisionAtLeast(
			authority,
			outcome.preloadValuesRevision ?? -1,
		);
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

	private preloadValuesRevisionAtLeast(
		authority: RecallAuthority,
		revision: number,
	) {
		return (
			this.isCurrent(authority) &&
			(this.options.preloadValuesStore.authoritativeRevision(
				authority.preloadValuesScope,
			) ?? -1) >= revision
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
		await this.repairConflict(error, authority);
		if (!this.isCurrent(authority)) return null;
		this.options.onError?.(error);
		return null;
	}

	private async repairConflict(error: Error, authority: RecallAuthority) {
		await Promise.allSettled([
			this.options.repairValues(error),
			...(authority.target === "preload"
				? [this.options.repairPreloadValues(error)]
				: []),
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
			(authority.target !== "preload" ||
				this.options.preloadValuesStore.isScopeCurrent(
					authority.preloadValuesScope,
				)) &&
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
	preloadValues: ProgrammerPreloadValuesProjection | null,
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
		expectedPreloadValuesRevision: preloadValues?.revision ?? null,
		expectedCaptureModeRevision: captureMode.revision,
		expectedSelectionRevision: selection.revision,
		selectedFixtureCount: selection.selected.length,
	};
}

function assertOutcome(
	request: PresetRecallRequest,
	outcome: PresetRecallOutcome,
	target: "programmer" | "preload",
) {
	if (outcome.preset.id !== request.presetId)
		throw new Error("Preset recall response object does not match");
	if (outcome.target !== target)
		throw new Error("Preset recall response values target does not match");
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
