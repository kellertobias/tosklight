import type {
	FileManagerOperationKind,
	FileManagerSelection,
	FileOperationState,
} from "./types";

let pending: FileManagerOperationKind | null = null;
let claimed: string | null = null;
let revision = 0;
const listeners = new Set<() => void>();

function changed() {
	revision += 1;
	for (const listener of listeners) listener();
}

export const fileOperationOwnership = {
	get pending() {
		return pending;
	},
	set pending(value: FileManagerOperationKind | null) {
		if (pending === value) return;
		pending = value;
		changed();
	},
	get claimed() {
		return claimed;
	},
	set claimed(value: string | null) {
		if (claimed === value) return;
		claimed = value;
		changed();
	},
};

export function subscribeFileOperationOwnership(listener: () => void) {
	listeners.add(listener);
	return () => listeners.delete(listener);
}

export function fileOperationOwnershipRevision() {
	return revision;
}

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
