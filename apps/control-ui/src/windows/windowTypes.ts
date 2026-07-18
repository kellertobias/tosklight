import type { DevelopmentView, StageView } from "../types";
import type { PresetFamily } from "../presetFamilies";

export interface WindowProps { builtIn?: boolean; compact?: boolean; paneId?: string; showGroupShortcuts?: boolean; stageView?: StageView; followPreload?: boolean; showBeamGuides?: boolean; cueListTab?: "pool" | "cues"; presetFamily?: PresetFamily; presetPoolColors?: boolean; developmentView?: DevelopmentView }
