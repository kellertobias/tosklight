import type { RecordUpdateOption } from "../../api/types";

export interface PoolRecordTarget {
	kind: "preset" | "cuelist" | "other";
	exists: boolean;
	cueCount?: number;
}

/** Describes the existing touch Record plan, including Smart's one-Cue question. */
export function poolRecordLabel(
	target: PoolRecordTarget,
	option: RecordUpdateOption,
) {
	if (!target.exists) return "REC";
	if (target.kind === "preset" && option === "merge") return "REC MRG";
	if (
		target.kind === "cuelist" &&
		(option === "add_cue" || (option === "smart" && target.cueCount !== 1))
	) return "REC CUE";
	return "REC";
}
