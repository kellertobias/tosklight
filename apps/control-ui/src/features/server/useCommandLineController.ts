import { useCallback } from "react";
import type { ServerState } from "./useServerState";

export function useCommandLineController(state: ServerState) {
	const {
		client,
		commandLineEpoch,
		commandLineWrite,
		commandTargetModeRef,
		setCommandLinePristine,
		setCommandLineState,
		setError,
		setPendingCommandChoice,
	} = state;
	const persistCommandLine = useCallback(
		(value: string) => {
			const write = commandLineWrite.current
				.catch(() => undefined)
				.then(() => client.setCommandLine(value));
			commandLineWrite.current = write;
			return write;
		},
		[client, commandLineWrite],
	);
	const setCommandLine = useCallback(
		(value: string, pristine = false) => {
			const next = value.trim() ? value : commandTargetModeRef.current;
			commandLineEpoch.current += 1;
			setCommandLineState(next);
			setCommandLinePristine(pristine || !value.trim());
			void persistCommandLine(next).catch((reason) => setError(String(reason)));
		},
		[
			commandLineEpoch,
			commandTargetModeRef,
			persistCommandLine,
			setCommandLinePristine,
			setCommandLineState,
			setError,
		],
	);
	const resetCommandLine = useCallback(
		() => setCommandLine("", true),
		[setCommandLine],
	);
	const dismissCommandChoice = useCallback(
		() => setPendingCommandChoice(null),
		[setPendingCommandChoice],
	);
	const cancelCommandChoice = useCallback(() => {
		dismissCommandChoice();
		resetCommandLine();
	}, [dismissCommandChoice, resetCommandLine]);
	return {
		persistCommandLine,
		setCommandLine,
		resetCommandLine,
		dismissCommandChoice,
		cancelCommandChoice,
	};
}
