import {
	canonicalPoolMutationOperation,
	type PoolMutationOperation,
} from "../../features/controlSurfaceInteraction/poolCommandTarget";

export type CueMutationTarget =
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

const CUE_SOURCE =
	/^(CPY|COPY|MOV|MOVE)\s+(SET\s+[1-9]\d*\s+CUE\s+\d+(?:\.\d+)?)\s+AT$/iu;

export function cueMutationTarget(
	commandLine: string,
): CueMutationTarget | null {
	const command = commandLine.trim().replace(/\s+/gu, " ").toUpperCase();
	const bare = OPERATION[command as keyof typeof OPERATION];
	if (bare) return { operation: bare, phase: "source" };
	const destination = CUE_SOURCE.exec(command);
	if (!destination) return null;
	const operation = OPERATION[destination[1] as keyof typeof OPERATION];
	if (!operation || operation === "delete") return null;
	return { operation, phase: "destination", source: destination[2] };
}

export function cueCommandAddress(playbackNumber: number, cueNumber: number) {
	return `SET ${playbackNumber} CUE ${cueNumber}`;
}

export function cueMutationCommand(target: CueMutationTarget, address: string) {
	const operation = canonicalPoolMutationOperation(target.operation);
	if (target.phase === "destination")
		return {
			kind: "execute",
			command: `${operation} ${target.source} AT ${address}`,
		} as const;
	if (target.operation === "delete")
		return { kind: "execute", command: `${operation} ${address}` } as const;
	return { kind: "replace", command: `${operation} ${address} AT` } as const;
}

export function cueMutationLabel(target: CueMutationTarget) {
	const operation = canonicalPoolMutationOperation(target.operation);
	return `${operation[0]}${operation.slice(1).toLowerCase()}`;
}
