import type { GroupRecordOperation } from "./contracts";

/** Groups normally merge; one-off Smart keeps the occupied-target choice dialog. */
export function groupRecordOperation(text: string): GroupRecordOperation | null {
	const option = /^\s*(?:RECORD|REC)\s+(SMART|MERGE|\+|\-)(?=\s|$)/i.exec(text)?.[1]?.toUpperCase();
	if (option === "SMART") return null;
	if (option === "-") return "subtract";
	return "merge";
}

export function groupRecordLabel(hasMembers: boolean, text: string) {
	return hasMembers && groupRecordOperation(text) === "merge" ? "REC MRG" : "REC";
}
