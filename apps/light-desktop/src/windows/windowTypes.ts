import type { PresetFamily } from "../presetFamilies";
import type {
	ChannelDisplayMode,
	FixtureSheetColumn,
	FixtureSheetCompactMode,
	StageView,
} from "../types";

export interface WindowProps {
	active?: boolean;
	builtIn?: boolean;
	compact?: boolean;
	viewOnly?: boolean;
	paneId?: string;
	showGroupShortcuts?: boolean;
	fixtureSheetIncludedHeads?: "all" | "no-sub-heads" | "no-master-heads";
	fixtureSheetOrder?: "fixture-id" | "active";
	fixtureSheetActiveOnly?: boolean;
	fixtureSheetCompactMode?: FixtureSheetCompactMode;
	fixtureSheetCueListId?: string | null;
	fixtureSheetColumns?: FixtureSheetColumn[];
	fixtureSheetShowType?: boolean;
	showCueSidebar?: boolean;
	cueListSource?: "fixed" | "follow-selection";
	fixedCueListNumber?: number;
	fixedCueListId?: string;
	stageView?: StageView;
	followPreload?: boolean;
	showBeamGuides?: boolean;
	stageRenderQuality?: import("../types").StageRenderQuality;
	cueListTab?: "pool" | "cues";
	presetFamily?: PresetFamily;
	presetPoolColors?: boolean;
	poolColumns?: number;
	schedulerShowList?: boolean;
	schedulerShowCalendar?: boolean;
	onSchedulerLayoutChange?: (layout: {
		showList: boolean;
		showCalendar: boolean;
	}) => void;
	channelDisplayMode?: ChannelDisplayMode;
}
