import type { PresetFamily } from "../../presetFamilies";

export interface ServerProgrammingContext {
	undoProgrammer: () => Promise<void>;
	toggleFixtureFreeze: () => Promise<void>;
	controlFixtureAction: (
		fixtureId: string,
		actionId: string,
		active: boolean,
	) => Promise<void>;
	controlFixtureActions: (
		targets: ReadonlyArray<{
			fixtureId: string;
			actionId: string;
			expectedProfileRevision: number;
		}>,
		expectedSelectionRevision: number,
		active: boolean,
	) => Promise<void>;
	generateFixturePresets: (
		fixtureIds: string[],
	) => Promise<import("../../api/types").GeneratedFixturePresetResult | null>;
	alignSelection: (
		mode: "off" | "left" | "right" | "out" | "in",
	) => Promise<void>;
	storePreload: (
		input: {
			target: "preset" | "cue";
			target_id: string;
			cue_number?: string;
			name?: string;
			mode?: "merge" | "overwrite" | "add_missing_fixtures";
			family?: PresetFamily;
		},
		revision: number,
	) => Promise<boolean>;
}
