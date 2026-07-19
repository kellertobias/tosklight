import { useCallback } from "react";
import { useServer } from "../../../api/ServerContext";
import { commandTargetAfterEnter } from "../../../controlSurface/commandTarget";
import {
	useProgrammingCommandLineActions,
	useProgrammingCommandLineView,
	useProgrammingInteractionStore,
	useProgrammingSelectionView,
} from "../../../features/programmingInteraction/ProgrammingInteractionView";
import { selectedGroupId } from "../../../features/programmingInteraction/contracts";

export function useCommandLineSurface({
	selection = false,
	enabled = true,
	observeCommand = true,
}: {
	selection?: boolean;
	enabled?: boolean;
	observeCommand?: boolean;
} = {}) {
	const server = useServer();
	const projection = useProgrammingCommandLineView(enabled, observeCommand);
	const selectionProjection = useProgrammingSelectionView(selection && enabled);
	const actions = useProgrammingCommandLineActions();
	const store = useProgrammingInteractionStore();
	const text = projection?.text ?? server.commandLine;
	const target = projection?.target ?? server.commandTargetMode;
	const pristine = projection?.pristine ?? server.commandLinePristine;
	const selected = selectionProjection?.selected ?? server.selectedFixtures;
	const read = useCallback(() => {
		const current = store.getSnapshot().commandLine;
		return current && actions
			? {
					text: current.text,
					target: current.target,
					pristine: current.pristine,
				}
			: {
					text: server.commandLine,
					target: server.commandTargetMode,
					pristine: server.commandLinePristine,
				};
	}, [actions, server, store]);
	const replace = useCallback(
		(value: string, legacyPristine?: boolean) => {
			if (store.getSnapshot().commandLine && actions)
				return actions.replace(value);
			if (legacyPristine === undefined) server.setCommandLine(value);
			else server.setCommandLine(value, legacyPristine);
			return Promise.resolve(true);
		},
		[actions, server, store],
	);
	const reset = useCallback(
		() => {
			if (store.getSnapshot().commandLine && actions) return actions.reset();
			server.resetCommandLine();
			return Promise.resolve(true);
		},
		[actions, server, store],
	);
	const execute = useCallback(
		async (value?: string) => {
			const current = read();
			const scoped = Boolean(store.getSnapshot().commandLine && actions);
			const command = value ?? current.text;
			const trimmed = command.trim();
			if (!scoped || !actions) return server.executeCommandLine(command);
			const pristine =
				!trimmed || trimmed.toUpperCase() === current.target;
			const resetTarget =
				commandTargetAfterEnter(command, current.target, pristine) ??
				current.target;
			const result = await actions.executeAfterPendingWrites(() =>
				server.executeCommandLine(command, {
					target: current.target,
					pristine,
				}),
				{
					text: resetTarget,
					target: resetTarget,
					pristine: true,
					pendingChoice: null,
				},
			);
			if (result === "write_failed") reportConcurrentCommandChange();
			if (result === "execution_unknown")
				reportUnreconciledCommand();
			return (
				result === "executed" ||
				result === "execution_unknown"
			);
		},
		[actions, read, server, store],
	);
	const cancelChoice = useCallback(() => {
		if (store.getSnapshot().commandLine && actions) {
			server.dismissCommandChoice();
			return actions.reset();
		}
		server.cancelCommandChoice();
		return Promise.resolve(true);
	}, [actions, server, store]);
	return {
		text,
		target,
		pristine,
		selected,
		selectedGroupId: selectionProjection
			? selectedGroupId(selectionProjection)
			: server.selectedGroupId,
		pendingChoice: projection
			? projection.pendingChoice
			: server.pendingCommandChoice,
		read,
		replace,
		reset,
		execute,
		cancelChoice,
		scoped: Boolean(projection && actions),
	};
}

export type CommandLineSurface = ReturnType<typeof useCommandLineSurface>;

function reportConcurrentCommandChange() {
	window.dispatchEvent(
		new CustomEvent("light:command-error", {
			detail: "The command line changed on another control surface.",
		}),
	);
}

function reportUnreconciledCommand() {
	globalThis.setTimeout(() => {
		window.dispatchEvent(
			new CustomEvent("light:command-error", {
				detail:
					"The command may have applied, but the desk command line could not be synchronized.",
			}),
		);
	}, 0);
}
