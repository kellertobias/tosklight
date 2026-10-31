import { formatErrorDetails } from "@tosklight/ui";
import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useMemo,
} from "react";
import type {
	SelectiveImportApplyRequest,
	SelectiveImportCatalog,
	SelectiveImportOutcome,
	SelectiveImportPreview,
	SelectiveImportSelection,
} from "../../api/selectiveImportModels";
import type {
	ShowMergeApplyResult,
	ShowMergePreview,
	ShowMergeResolution,
} from "../../api/client/showMerge";
import type { ShowEntry } from "../../api/types";
import { useStableCallback } from "../shared/useStableCallback";

export interface SelectiveImportSource {
	catalog: (
		targetShowId: string,
		sourceShowId: string,
		signal?: AbortSignal,
	) => Promise<SelectiveImportCatalog>;
	preview: (
		targetShowId: string,
		sourceShowId: string,
		selection: SelectiveImportSelection,
		signal?: AbortSignal,
	) => Promise<SelectiveImportPreview>;
	apply: (
		targetShowId: string,
		sourceShowId: string,
		request: SelectiveImportApplyRequest,
	) => Promise<SelectiveImportOutcome>;
	/** Offline copies of the open show and merging them back (TL-858). */
	takeOfflineCopy?: (
		name: string,
		rootId: string,
		path: string,
	) => Promise<ShowEntry>;
	previewMerge?: (
		targetShowId: string,
		copyShowId: string,
		resolutions: ShowMergeResolution[],
		signal?: AbortSignal,
	) => Promise<ShowMergePreview>;
	applyMerge?: (
		targetShowId: string,
		copyShowId: string,
		expectedTargetRevision: number,
		resolutions: ShowMergeResolution[],
	) => Promise<ShowMergeApplyResult>;
	refreshCompatibilityState: () => Promise<void>;
	reportError: (error: string | null) => void;
}

export type SelectiveImportCapability = Pick<
	SelectiveImportSource,
	| "catalog"
	| "preview"
	| "apply"
	| "takeOfflineCopy"
	| "previewMerge"
	| "applyMerge"
>;

const SelectiveImportContext = createContext<SelectiveImportCapability | null>(
	null,
);

export function SelectiveImportProvider({
	source,
	children,
}: PropsWithChildren<{
	source: SelectiveImportSource;
}>) {
	const catalog = useStableCallback(source.catalog);
	const preview = useStableCallback(source.preview);
	const applyCommand = useStableCallback(source.apply);
	const refresh = useStableCallback(source.refreshCompatibilityState);
	const reportError = useStableCallback(source.reportError);
	const apply = useCallback(
		async (...args: Parameters<typeof source.apply>) => {
			try {
				const outcome = await applyCommand(...args);
				if (outcome.changed) await refresh();
				reportError(null);
				return outcome;
			} catch (reason) {
				reportError(formatErrorDetails(reason));
				throw reason;
			}
		},
		[applyCommand, refresh, reportError],
	);
	const takeOfflineCopy = source.takeOfflineCopy;
	const previewMerge = source.previewMerge;
	const applyMergeCommand = source.applyMerge;
	const applyMerge = useMemo(
		() =>
			applyMergeCommand &&
			(async (...args: Parameters<typeof applyMergeCommand>) => {
				const result = await applyMergeCommand(...args);
				if (result.changed) await refresh();
				return result;
			}),
		[applyMergeCommand, refresh],
	);
	const value = useMemo(
		() => ({ catalog, preview, apply, takeOfflineCopy, previewMerge, applyMerge }),
		[catalog, preview, apply, takeOfflineCopy, previewMerge, applyMerge],
	);
	return (
		<SelectiveImportContext.Provider value={value}>
			{children}
		</SelectiveImportContext.Provider>
	);
}

export function useSelectiveImport() {
	const capability = useContext(SelectiveImportContext);
	if (!capability) {
		throw new Error(
			"useSelectiveImport must be used inside SelectiveImportProvider",
		);
	}
	return capability;
}

/** Partial Show Load capability, or null outside a connected desk boundary. */
export function useOptionalSelectiveImport() {
	return useContext(SelectiveImportContext);
}
