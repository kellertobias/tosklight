import { useEffect, useMemo, useRef, useState } from "react";
import type {
	SelectiveImportApplyRequest,
	SelectiveImportCatalog,
	SelectiveImportConflictResolution,
	SelectiveImportObjectKey,
	SelectiveImportOutcome,
	SelectiveImportPreview,
	SelectiveImportProfileConflictResolution,
	SelectiveImportSelection,
} from "../../api/selectiveImportModels";
import type { ShowEntry } from "../../api/types";
import {
	buildSelection,
	objectKeyId,
	operatorError,
	profileKeyId,
	toggledSet,
	updatedMap,
} from "./selectiveImportHelpers";

export interface SelectiveImportWorkflowOptions {
	activeShow: ShowEntry;
	shows: ShowEntry[];
	onClose: () => void;
	loadCatalog: (
		targetShowId: string,
		sourceShowId: string,
		signal?: AbortSignal,
	) => Promise<SelectiveImportCatalog>;
	previewImport: (
		targetShowId: string,
		sourceShowId: string,
		selection: SelectiveImportSelection,
		signal?: AbortSignal,
	) => Promise<SelectiveImportPreview>;
	applyImport: (
		targetShowId: string,
		sourceShowId: string,
		request: SelectiveImportApplyRequest,
	) => Promise<SelectiveImportOutcome>;
}

type Phase = "idle" | "catalog" | "preview" | "apply";

interface WorkflowState {
	sourceId: string;
	catalog: SelectiveImportCatalog | null;
	selected: Set<string>;
	objectChoices: Map<string, SelectiveImportConflictResolution>;
	profileChoices: Map<string, SelectiveImportProfileConflictResolution>;
	preview: SelectiveImportPreview | null;
	previewKey: string;
	outcome: SelectiveImportOutcome | null;
	phase: Phase;
	error: string | null;
}

type StateUpdate = Partial<WorkflowState> | ((current: WorkflowState) => Partial<WorkflowState>);
type PatchState = (update: StateUpdate) => void;

function initialState(): WorkflowState {
	return {
		sourceId: "",
		catalog: null,
		selected: new Set(),
		objectChoices: new Map(),
		profileChoices: new Map(),
		preview: null,
		previewKey: "",
		outcome: null,
		phase: "idle",
		error: null,
	};
}

function useWorkflowState(): [WorkflowState, PatchState] {
	const [state, setState] = useState(initialState);
	const patch = (update: StateUpdate) => setState((current) => ({
		...current,
		...(typeof update === "function" ? update(current) : update),
	}));
	return [state, patch];
}

function useAbortableRequest(patch: PatchState) {
	const current = useRef<AbortController | null>(null);
	function abort() {
		const controller = current.current;
		current.current = null;
		controller?.abort();
	}
	useEffect(() => abort, []);
	async function run<T>(
		phase: "catalog" | "preview",
		operation: (signal: AbortSignal) => Promise<T>,
		receive: (value: T) => void,
	) {
		abort();
		const controller = new AbortController();
		current.current = controller;
		patch({ phase, error: null });
		try {
			receive(await operation(controller.signal));
		} catch (reason) {
			if (!controller.signal.aborted) patch({ error: operatorError(reason) });
		} finally {
			if (current.current === controller) {
				current.current = null;
				patch({ phase: "idle" });
			}
		}
	}
	return { abort, run };
}

export function useSelectiveImportWorkflow(options: SelectiveImportWorkflowOptions) {
	const [state, patch] = useWorkflowState();
	const requests = useAbortableRequest(patch);
	const sources = options.shows.filter((show) => show.id !== options.activeShow.id);
	const selection = useMemo(
		() => buildSelection(
			state.catalog,
			state.selected,
			state.objectChoices,
			state.profileChoices,
		),
		[state.catalog, state.selected, state.objectChoices, state.profileChoices],
	);
	const selectionKey = JSON.stringify(selection);
	const previewCurrent = state.preview !== null && state.previewKey === selectionKey;

	async function chooseSource(sourceId: string) {
		requests.abort();
		patch({ ...initialState(), sourceId });
		if (!sourceId) return;
		await requests.run(
			"catalog",
			(signal) => options.loadCatalog(options.activeShow.id, sourceId, signal),
			(catalog) => patch({ catalog }),
		);
	}

	async function inspectSelection() {
		if (!state.sourceId || selection.selectedObjects.length === 0) return;
		patch({ previewKey: "" });
		await requests.run(
			"preview",
			(signal) => options.previewImport(
				options.activeShow.id,
				state.sourceId,
				selection,
				signal,
			),
			(preview) => patch({ preview, previewKey: selectionKey }),
		);
	}

	async function apply() {
		if (!previewCurrent || !state.preview?.canApply || !state.sourceId) return;
		patch({ phase: "apply", error: null });
		try {
			const outcome = await options.applyImport(options.activeShow.id, state.sourceId, {
				requestId: crypto.randomUUID(),
				expectedSourceRevision: state.preview.sourceRevision,
				expectedTargetRevision: state.preview.targetRevision,
				...selection,
			});
			patch({ outcome });
		} catch (reason) {
			patch({ error: operatorError(reason), previewKey: "" });
		} finally {
			patch({ phase: "idle" });
		}
	}

	function close() {
		if (state.phase === "apply") return;
		requests.abort();
		options.onClose();
	}

	return {
		...state,
		apply,
		chooseSource,
		close,
		inspectSelection,
		previewCurrent,
		selection,
		setObjectChoice: (
			key: SelectiveImportObjectKey,
			value: SelectiveImportConflictResolution | null,
		) =>
			patch((current) => ({
				objectChoices: updatedMap(current.objectChoices, objectKeyId(key), value),
				previewKey: "",
			})),
		setProfileChoice: (
			key: { profileId: string; revision: number },
			value: SelectiveImportProfileConflictResolution | null,
		) => patch((current) => ({
			profileChoices: updatedMap(current.profileChoices, profileKeyId(key), value),
			previewKey: "",
		})),
		sources,
		toggleObject: (key: SelectiveImportObjectKey, checked: boolean) =>
			patch((current) => ({
				selected: toggledSet(current.selected, objectKeyId(key), checked),
				objectChoices: new Map(),
				profileChoices: new Map(),
				previewKey: "",
				outcome: null,
			})),
	};
}

export type SelectiveImportWorkflow = ReturnType<typeof useSelectiveImportWorkflow>;
