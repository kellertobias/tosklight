import type {
	FileManagerOperationKind,
	FileManagerSelection,
	FileOperationState,
} from "./types";

export const fileOperationOwnership: {
	pending: FileManagerOperationKind | null;
	claimed: string | null;
} = { pending: null, claimed: null };

export function emptyOperation(
	kind: FileManagerOperationKind,
	sources: FileManagerSelection[] = [],
): FileOperationState {
	return {
		kind,
		sources,
		renameDraft:
			kind === "rename" && sources.length === 1 ? sources[0].entry.name : "",
		confirming: kind === "delete" && sources.length > 0,
	};
}
