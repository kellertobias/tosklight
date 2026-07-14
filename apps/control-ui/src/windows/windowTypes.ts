import type { DevelopmentView, StageView } from "../types";
export interface WindowProps { builtIn?: boolean; compact?: boolean; paneId?: string; showGroupShortcuts?: boolean; stageView?: StageView; followPreload?: boolean; cueListTab?: "pool" | "cues"; presetFamily?: "All" | "Intensity" | "Color" | "Position" | "Beam"; presetPoolColors?: boolean; developmentView?: DevelopmentView }
