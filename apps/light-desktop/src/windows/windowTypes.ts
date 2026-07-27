import type { PresetFamily } from "../presetFamilies";
import type { StageView } from "../types";

export interface WindowProps {
	active?: boolean;
	builtIn?: boolean;
	compact?: boolean;
	paneId?: string;
	showGroupShortcuts?: boolean;
	showCueSidebar?: boolean;
	cueListSource?: "fixed" | "follow-selection";
	fixedCueListNumber?: number;
	stageView?: StageView;
	followPreload?: boolean;
	showBeamGuides?: boolean;
	stageRenderQuality?: import("../types").StageRenderQuality;
	cueListTab?: "pool" | "cues";
	presetFamily?: PresetFamily;
	presetPoolColors?: boolean;
}
