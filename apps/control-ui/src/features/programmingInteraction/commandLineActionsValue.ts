import { useMemo } from "react";
import {
	type CommandExecutionOutcome,
	createCommandLineExecution,
	type ExecuteCommandLine,
} from "./commandExecution";
import type {
	CommandLineExecutionResult,
	ProgrammingCommandLineWriter,
} from "./commandLineWriter";
import type { CommandLinePatch } from "./contracts";
import type { ProgrammingSelectionWriter } from "./selectionWriter";
import type { ProgrammingInteractionStore } from "./store";

export interface ProgrammingCommandLineActions {
	replace(text: string): Promise<boolean>;
	reset(): Promise<boolean>;
	flush(): Promise<boolean>;
	/** Settles pending edits, then runs the command against scoped authority. */
	execute(value?: string): Promise<CommandExecutionOutcome>;
	executeAfterPendingWrites(
		execute: () => Promise<boolean>,
		optimisticReset: CommandLinePatch,
	): Promise<CommandLineExecutionResult>;
}

export function useProgrammingCommandLineActionsValue(
	commandLineWriter: ProgrammingCommandLineWriter | null,
	selectionWriter: ProgrammingSelectionWriter | null,
	store: ProgrammingInteractionStore,
	executeCommand?: ExecuteCommandLine,
) {
	return useMemo<ProgrammingCommandLineActions | null>(() => {
		if (!commandLineWriter) return null;
		const executeAfterPendingWrites: ProgrammingCommandLineActions["executeAfterPendingWrites"] =
			(execute, optimisticReset) => {
				const run = () =>
					commandLineWriter.executeAfterPendingWrites(execute, optimisticReset);
				return selectionWriter
					? selectionWriter.runAfterPendingWrites(run, "write_failed")
					: run();
			};
		return {
			replace: (text) => commandLineWriter.replace(text),
			reset: () => commandLineWriter.replace(""),
			flush: () => commandLineWriter.flush(),
			execute: createCommandLineExecution({
				store,
				// Selection writes share this barrier, so Enter settles every pending
				// desk edit before command execution.
				executeAfterPendingWrites,
				execute: executeCommand,
			}),
			executeAfterPendingWrites,
		};
	}, [commandLineWriter, executeCommand, selectionWriter, store]);
}
