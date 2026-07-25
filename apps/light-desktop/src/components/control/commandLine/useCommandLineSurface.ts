import { useCallback } from "react";
import type { CommandTarget } from "../../../features/programmingInteraction/contracts";
import { selectedGroupId } from "../../../features/programmingInteraction/contracts";
import {
	useProgrammingCommandLineActions,
	useProgrammingCommandLineReady,
	useProgrammingCommandLineView,
	useProgrammingInteractionStore,
	useProgrammingSelectionView,
} from "../../../features/programmingInteraction/ProgrammingInteractionView";

/**
 * What a not-ready surface shows instead of retained legacy or bootstrap state.
 *
 * Scoped authority is the only source of command text, so a loading, absent, or
 * replaced authority displays nothing and refuses every write.
 */
const NOT_READY = {
	text: "",
	target: "FIXTURE" as CommandTarget,
	pristine: true,
};

export function useCommandLineSurface({
	selection = false,
	enabled = true,
	observeCommand = true,
}: {
	selection?: boolean;
	enabled?: boolean;
	observeCommand?: boolean;
} = {}) {
	const projection = useProgrammingCommandLineView(enabled, observeCommand);
	const selectionProjection = useProgrammingSelectionView(selection && enabled);
	const actions = useProgrammingCommandLineActions();
	const store = useProgrammingInteractionStore();
	const authoritative = useProgrammingCommandLineReady(enabled);
	const ready = authoritative && actions !== null;
	const text = projection?.text ?? NOT_READY.text;
	const target = projection?.target ?? NOT_READY.target;
	const pristine = projection?.pristine ?? NOT_READY.pristine;
	const selected = selectionProjection?.selected ?? [];
	const read = useCallback(() => {
		const current = store.getSnapshot().commandLine;
		if (!enabled || !current || !actions)
			return { ...NOT_READY, ready: false };
		return {
			text: current.text,
			target: current.target,
			pristine: current.pristine,
			ready: true,
		};
	}, [actions, enabled, store]);
	const writable = useCallback(
		() =>
			enabled && Boolean(actions) && store.getSnapshot().commandLine !== null,
		[actions, enabled, store],
	);
	const replace = useCallback(
		// The scoped writer derives pristine from the authoritative target, so the
		// legacy pristine argument callers still pass is deliberately unused.
		(value: string, _legacyPristine?: boolean) =>
			writable() && actions
				? actions.replace(value)
				: Promise.resolve(false),
		[actions, writable],
	);
	const reset = useCallback(
		() =>
			writable() && actions ? actions.reset() : Promise.resolve(false),
		[actions, writable],
	);
	const execute = useCallback(
		async (value?: string) => {
			if (!writable() || !actions) return false;
			const outcome = await actions.execute(value);
			if (outcome.report === "concurrent_change")
				reportConcurrentCommandChange();
			if (outcome.report === "unreconciled") reportUnreconciledCommand();
			return outcome.executed;
		},
		[actions, writable],
	);
	const cancelChoice = useCallback(
		() =>
			writable() && actions ? actions.reset() : Promise.resolve(false),
		[actions, writable],
	);
	return {
		ready,
		text,
		target,
		pristine,
		selected,
		selectedGroupId: selectedGroupId(selectionProjection),
		read,
		replace,
		reset,
		execute,
		cancelChoice,
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
