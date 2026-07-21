import { commandTargetAfterEnter } from "../../controlSurface/commandTarget";
import type { CommandLineExecutionResult } from "./commandLineWriter";
import type { CommandLinePatch, CommandTarget } from "./contracts";
import type { ProgrammingInteractionStore } from "./store";

/**
 * The desk interaction a completed command carries into the shared execution
 * transport. The feature owns this shape so the command surface never has to
 * reach into the global server context to run a command.
 */
export interface CommandExecutionRequest {
	command: string;
	target: CommandTarget;
	pristine: boolean;
}

/** The one injected command-execution dependency of the command-line surface. */
export type ExecuteCommandLine = (
	request: CommandExecutionRequest,
) => Promise<boolean>;

/** What the operator should be told after an execution settled. */
export type CommandExecutionReport =
	| "none"
	| "concurrent_change"
	| "unreconciled";

export interface CommandExecutionOutcome {
	executed: boolean;
	report: CommandExecutionReport;
}

/** The one writer capability an execution needs: settle pending edits, then run. */
export type ExecuteAfterPendingWrites = (
	execute: () => Promise<boolean>,
	optimisticReset: CommandLinePatch,
) => Promise<CommandLineExecutionResult>;

const REFUSED: CommandExecutionOutcome = { executed: false, report: "none" };

/**
 * Runs one command against the scoped authority.
 *
 * Pending command edits settle first, so Enter can never overtake a keystroke
 * that is still in flight. The captured store scope is rechecked afterwards: a
 * replaced show, desk, session, or server owns the visible command line by
 * then, and a late completion must neither reset it nor report against it.
 */
export function createCommandLineExecution({
	store,
	executeAfterPendingWrites,
	execute,
}: {
	store: ProgrammingInteractionStore;
	executeAfterPendingWrites: ExecuteAfterPendingWrites;
	execute: ExecuteCommandLine | undefined;
}) {
	return async (value?: string): Promise<CommandExecutionOutcome> => {
		if (!execute) return REFUSED;
		const scope = store.captureScope();
		const current = store.getSnapshot().commandLine;
		if (!current) return REFUSED;
		const command = value ?? current.text;
		const trimmed = command.trim();
		const pristine = !trimmed || trimmed.toUpperCase() === current.target;
		const resetTarget =
			commandTargetAfterEnter(command, current.target, pristine) ??
			current.target;
		const result = await executeAfterPendingWrites(
			() => execute({ command, target: current.target, pristine }),
			{
				text: resetTarget,
				target: resetTarget,
				pristine: true,
				pendingChoice: null,
			},
		);
		if (!store.isScopeCurrent(scope)) return REFUSED;
		return {
			executed: result === "executed" || result === "execution_unknown",
			report: reportFor(result),
		};
	};
}

function reportFor(result: CommandLineExecutionResult): CommandExecutionReport {
	if (result === "write_failed") return "concurrent_change";
	if (result === "execution_unknown") return "unreconciled";
	return "none";
}
