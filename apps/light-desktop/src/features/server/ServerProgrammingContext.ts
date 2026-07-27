import type { PresetFamily } from "../../presetFamilies";

export interface ServerProgrammingContext {
	undoProgrammer: () => Promise<void>;
	controlFixtureAction: (
		fixtureId: string,
		actionId: string,
		active: boolean,
	) => Promise<void>;
	generateFixturePresets: (
		fixtureIds: string[],
	) => Promise<import("../../api/types").GeneratedFixturePresetResult | null>;
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
