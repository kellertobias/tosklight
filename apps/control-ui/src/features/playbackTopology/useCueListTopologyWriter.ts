import { useCallback } from "react";
import type { CueList, VersionedObject } from "../../api/types";
import { usePlaybackTopologyActions } from "./PlaybackTopologyProvider";

export interface CueListWriteBasis {
	cueListId: string;
	expectedRevision: number;
	expectedObjectId: string;
}

export type SaveCueListTopology = (
	basis: CueListWriteBasis,
	body: CueList,
) => Promise<VersionedObject<CueList> | null>;

export function cueListWriteBasis(
	object: VersionedObject<CueList>,
): CueListWriteBasis {
	return {
		cueListId: object.body.id,
		expectedRevision: object.revision,
		expectedObjectId: object.id,
	};
}

export function useCueListTopologyWriter(): SaveCueListTopology {
	const actions = usePlaybackTopologyActions();
	const saveCueList = actions?.saveCueList;
	return useCallback(
		async (basis, body) => {
			const outcome = await saveCueList?.(
				basis.cueListId,
				basis.expectedRevision,
				basis.expectedObjectId,
				body,
			);
			const object = outcome?.objects.find(
				(candidate) =>
					candidate.kind === "cue_list" && candidate.state === "present",
			);
			if (!object || object.state !== "present") return null;
			return {
				kind: "cue_list",
				id: object.objectId,
				revision: object.objectRevision,
				updated_at: "",
				body: object.body as CueList,
			};
		},
		[saveCueList],
	);
}
