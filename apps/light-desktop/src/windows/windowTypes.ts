import type { PresetFamily } from "../presetFamilies";
import type {
	ChannelDisplayMode,
	FixtureSheetColumn,
	FixtureSheetCompactMode,
	StageView,
	VisualizationRow,
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
	cueListCompactRows?: boolean;
	cueInformationBlock?: "off" | "current" | "next";
	cueListSource?: "fixed" | "follow-selection";
	fixedCueListNumber?: number;
	fixedCueListId?: string;
	stageView?: StageView;
	stage2dSide?: import("../types").Stage2dSide;
	followPreload?: boolean;
	cueListTab?: "pool" | "cues";
	patchView?: "fixtures" | "media";
	presetFamily?: PresetFamily;
	presetPoolColors?: boolean;
	poolColumns?: number;
	schedulerShowList?: boolean;
	schedulerShowCalendar?: boolean;
	onSchedulerLayoutChange?: (layout: {
		showList: boolean;
		showCalendar: boolean;
	}) => void;
	mediaPaneState?: {
		serverId?: string;
		layerId?: string;
		browserMode?: "media" | "mask";
		sourceFilter?: "media" | "visualizers" | "text";
		controlSectionId?: string;
		mainSectionId?: string;
		rightPaneVisible?: boolean;
	};
	onMediaPaneStateChange?: (
		state: NonNullable<WindowProps["mediaPaneState"]>,
	) => void;
	runningFilter?: "all" | "cue_list" | "dynamic" | "timecode" | "macro";
	onRunningFilterChange?: (
		filter: NonNullable<WindowProps["runningFilter"]>,
	) => void;
	channelDisplayMode?: ChannelDisplayMode;
	visualizationRows?: VisualizationRow[];
}
