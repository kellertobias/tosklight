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
	refreshCompatibilityState: () => Promise<void>;
	reportError: (error: string | null) => void;
}

export type SelectiveImportCapability = Pick<
	SelectiveImportSource,
	"catalog" | "preview" | "apply"
>;

const SelectiveImportContext = createContext<SelectiveImportCapability | null>(null);

export function SelectiveImportProvider({ source, children }: PropsWithChildren<{
	source: SelectiveImportSource;
}>) {
	const catalog = useStableCallback(source.catalog);
	const preview = useStableCallback(source.preview);
	const applyCommand = useStableCallback(source.apply);
	const refresh = useStableCallback(source.refreshCompatibilityState);
	const reportError = useStableCallback(source.reportError);
	const apply = useCallback(async (...args: Parameters<typeof source.apply>) => {
		try {
			const outcome = await applyCommand(...args);
			if (outcome.changed) await refresh();
			reportError(null);
			return outcome;
		} catch (reason) {
			reportError(reason instanceof Error ? reason.message : String(reason));
			throw reason;
		}
	}, [applyCommand, refresh, reportError]);
	const value = useMemo(() => ({ catalog, preview, apply }), [catalog, preview, apply]);
	return (
		<SelectiveImportContext.Provider value={value}>
			{children}
		</SelectiveImportContext.Provider>
	);
}

export function useSelectiveImport() {
	const capability = useContext(SelectiveImportContext);
	if (!capability) {
		throw new Error("useSelectiveImport must be used inside SelectiveImportProvider");
	}
	return capability;
}
