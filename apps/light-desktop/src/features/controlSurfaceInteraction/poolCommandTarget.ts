import type { PoolCardState } from "@tosklight/ui/pools";

export type PoolMutationOperation = "copy" | "move" | "delete";

export type PoolMutationTarget =
	| { operation: PoolMutationOperation; phase: "source" }
	| {
			operation: Exclude<PoolMutationOperation, "delete">;
			phase: "destination";
			source: string;
		};

const OPERATION = {
	CPY: "copy",
	COPY: "copy",
	MOV: "move",
	MOVE: "move",
	DEL: "delete",
	DELETE: "delete",
} as const;

/**
 * Resolves the command phase that pool cards can safely target. A partially
 * typed address is deliberately not actionable: only the bare operation or a
 * complete source followed by AT owns card presses.
 */
export function poolMutationTarget(commandLine: string): PoolMutationTarget | null {
	const tokens = commandLine.trim().toUpperCase().split(/\s+/u);
	const operation = OPERATION[tokens[0] as keyof typeof OPERATION];
	if (!operation) return null;
	if (tokens.length === 1) return { operation, phase: "source" };
	if (
		operation !== "delete" &&
		tokens.length === 3 &&
		tokens[2] === "AT"
	)
		return { operation, phase: "destination", source: tokens[1] };
	return null;
}

export function poolMutationTargetState(
	target: PoolMutationTarget | null,
): PoolCardState | null {
	return target ? `${target.operation}-target` : null;
}

export function canonicalPoolMutationOperation(
	operation: PoolMutationOperation,
) {
	return operation.toUpperCase();
}
