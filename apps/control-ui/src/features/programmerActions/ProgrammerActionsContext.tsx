import { createContext, type PropsWithChildren, useContext } from "react";
import type { GeneratedFixturePresetResult } from "../../api/types";
import type { PresetFamily } from "../../presetFamilies";

/**
 * Scoped one-shot programmer actions for surfaces that must not observe the broad
 * server context: keypad undo, fixture control actions, preset generation, selection
 * alignment, and preload storage.
 */
export interface ProgrammerActions {
	undoProgrammer: () => Promise<void>;
	clearProgrammer: (sessionId: string) => Promise<void>;
	controlFixtureAction: (
		fixtureId: string,
		actionId: string,
		active: boolean,
	) => Promise<void>;
	generateFixturePresets: (
		fixtureIds: string[],
	) => Promise<GeneratedFixturePresetResult | null>;
	alignSelection: (
		attribute: string,
		mode: "left" | "right" | "center" | "out",
	) => Promise<void>;
	storePreload: (
		input: {
			target: "preset" | "cue";
			target_id: string;
			cue_number?: number;
			name?: string;
			mode?: "merge" | "overwrite" | "add_missing_fixtures";
			family?: PresetFamily;
		},
		revision: number,
	) => Promise<boolean>;
}

const ProgrammerActionsContext = createContext<ProgrammerActions | null>(null);

export function ProgrammerActionsProvider({
	children,
	actions,
}: PropsWithChildren<{ actions: ProgrammerActions }>) {
	return (
		<ProgrammerActionsContext.Provider value={actions}>
			{children}
		</ProgrammerActionsContext.Provider>
	);
}

/** Programmer actions, or null outside a mounted desk boundary. */
export function useProgrammerActions(): ProgrammerActions | null {
	return useContext(ProgrammerActionsContext);
}
