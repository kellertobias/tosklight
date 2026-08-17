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
export function poolMutationTarget(
	commandLine: string,
): PoolMutationTarget | null {
	const tokens = commandLine.trim().toUpperCase().split(/\s+/u);
	const operation = OPERATION[tokens[0] as keyof typeof OPERATION];
	if (!operation) return null;
	if (tokens.length === 1) return { operation, phase: "source" };
	if (operation !== "delete" && tokens.at(-1) === "AT" && tokens.length > 3)
		return {
			operation,
			phase: "destination",
			source: tokens.slice(1, -1).join(" "),
		};
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

export function poolObjectMutationCommand(
	target: PoolMutationTarget | null,
	family: "GROUP" | "CUELIST",
	number: string | number,
	occupied: boolean,
) {
	if (!target) return null;
	const address = `${family} ${number}`;
	const operation = canonicalPoolMutationOperation(target.operation);
	if (target.phase === "source") {
		if (!occupied) return null;
		return target.operation === "delete"
			? ({ kind: "execute", command: `${operation} ${address}` } as const)
			: ({ kind: "replace", command: `${operation} ${address} AT` } as const);
	}
	if (!target.source.startsWith(`${family} `)) return null;
	if (occupied) return null;
	return {
		kind: "execute",
		command: `${operation} ${target.source} AT ${address}`,
	} as const;
}
