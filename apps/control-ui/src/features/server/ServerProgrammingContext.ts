import type { StoredGroup } from "../../api/types";
import type { PresetAddress, PresetFamily } from "../../presetFamilies";

export interface ServerProgrammingContext {
	undoProgrammer: () => Promise<void>;
	setSelection: (fixtures: string[]) => Promise<void>;
	selectionGesture: (
		source:
			| { type: "fixture"; fixture_id: string }
			| { type: "live_group"; group_id: string }
			| { type: "dereferenced_group"; group_id: string },
		remove?: boolean,
	) => Promise<void>;
	setProgrammer: (
		fixtureId: string,
		attribute: string,
		value: number,
	) => Promise<void>;
	setProgrammerMany: (
		assignments: Array<{ fixtureId: string; attribute: string; value: number }>,
	) => Promise<boolean>;
	setProgrammerValue: (
		fixtureId: string,
		attribute: string,
		value: import("../../api/types").AttributeValue,
	) => Promise<void>;
	controlFixtureAction: (
		fixtureId: string,
		actionId: string,
		active: boolean,
	) => Promise<void>;
	generateFixturePresets: (
		fixtureIds: string[],
	) => Promise<import("../../api/types").GeneratedFixturePresetResult | null>;
	releaseProgrammer: (fixtureId: string, attribute: string) => Promise<void>;
	setGroupValue: (
		attribute: string,
		value: number | import("../../api/types").AttributeValue,
	) => Promise<void>;
	releaseGroupValue: (attribute: string) => Promise<void>;
	setPreloadGroupValue: (attribute: string, value: number) => Promise<void>;
	applyGroup: (id: string) => Promise<void>;
	selectGroup: (
		id: string,
		frozen?: boolean,
		rule?: Record<string, unknown>,
	) => Promise<void>;
	selectionMacro: (rule: Record<string, unknown>) => Promise<void>;
	alignSelection: (
		attribute: string,
		mode: "left" | "right" | "center" | "out",
	) => Promise<void>;
	preloadAction: (
		action: "enter" | "go" | "clear" | "release",
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
	storeDynamic: (
		speed: number,
		width: number,
		direction: string,
	) => Promise<void>;
	storeGroup: (
		id: string,
		name: string,
		mode?: "merge" | "overwrite",
	) => Promise<void>;
	updateGroup: (
		id: string,
		update: Pick<StoredGroup, "name" | "color" | "icon">,
	) => Promise<boolean>;
	setGroupMaster: (id: string, master: number) => Promise<void>;
	setGroupMasterFlash: (id: string, value: number) => Promise<void>;
	undoGroup: (id: string) => Promise<void>;
	refreshFrozenGroup: (id: string) => Promise<void>;
	detachDerivedGroup: (id: string) => Promise<void>;
	applyPreset: (address: PresetAddress) => Promise<void>;
	storePreset: (
		address: PresetAddress,
		name: string,
		mode: "merge" | "overwrite" | "add_missing_fixtures",
		family?: PresetFamily,
	) => Promise<void>;
}
