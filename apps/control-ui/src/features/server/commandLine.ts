import {
	commandTargetAfterEnter,
	defaultCommandLine,
} from "../../controlSurface/commandTarget";
import type { PendingCommandChoice } from "./contracts";
import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createCommandLineActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	| "refresh"
	| "setCommandLine"
	| "resetCommandLine"
	| "dismissCommandChoice"
	| "cancelCommandChoice"
	| "executeCommandLine"
> {
	const {
		client,
		setError,
		setCommandTargetMode,
		commandTargetModeRef,
		commandLine,
		setCommandLineState,
		commandLinePristine,
		setCommandLinePristine,
		commandLineWrite,
		commandLineEpoch,
		setPendingCommandChoice,
		setSelectedFixtures,
		setSelectedGroupId,
		refresh,
		persistCommandLine,
		setCommandLine,
		resetCommandLine,
		dismissCommandChoice,
		cancelCommandChoice,
	} = model;
	return {
		refresh,
		setCommandLine,
		resetCommandLine,
		dismissCommandChoice,
		cancelCommandChoice,
		executeCommandLine: async (value = commandLine, interaction) => {
			try {
				// Invalidate event refreshes that began while the command was being assembled. A
				// successful execution increments this again when it installs the reset target.
				commandLineEpoch.current += 1;
				// Key presses update the authoritative desk command line. Preserve their order so an
				// older in-flight key cannot arrive after Enter and restore a command that already ran.
				await commandLineWrite.current;
				const activeTarget =
					interaction?.target ?? commandTargetModeRef.current;
				const activePristine =
					interaction?.pristine ?? commandLinePristine;
				if (interaction) {
					commandTargetModeRef.current = activeTarget;
					setCommandTargetMode(activeTarget);
				}
				const toggledTarget = commandTargetAfterEnter(
					value,
					activeTarget,
					activePristine,
				);
				if (toggledTarget) {
					const nextTarget = toggledTarget;
					commandTargetModeRef.current = nextTarget;
					setCommandTargetMode(nextTarget);
					commandLineEpoch.current += 1;
					setCommandLineState(nextTarget);
					setCommandLinePristine(true);
					await client.setCommandTarget(nextTarget);
					await persistCommandLine(nextTarget);
					setError(null);
					return true;
				}
				const result = (await client.executeCommandLine(value)) as
					| {
							programmer?: {
								selected?: string[];
								selection_expression?: {
									type?: string;
									group_id?: string;
								} | null;
							};
							pending_choice?: PendingCommandChoice;
					  }
					| undefined;
				if (result?.pending_choice) {
					setPendingCommandChoice(result.pending_choice);
					setError(null);
					return true;
				}
				if (result?.programmer?.selected) {
					setSelectedFixtures(result.programmer.selected);
					setSelectedGroupId(
						result.programmer.selection_expression?.type === "live_group"
							? (result.programmer.selection_expression.group_id ?? null)
							: null,
					);
				}
				setPendingCommandChoice(null);
				const target = defaultCommandLine(commandTargetModeRef.current);
				commandLineEpoch.current += 1;
				setCommandLineState(target);
				setCommandLinePristine(true);
				if (!interaction) await persistCommandLine(target);
				setError(null);
				return true;
			} catch (reason) {
				const message =
					reason instanceof Error ? reason.message : String(reason);
				setError(message);
				window.dispatchEvent(
					new CustomEvent("light:command-error", { detail: message }),
				);
				return false;
			}
		},
	};
}
