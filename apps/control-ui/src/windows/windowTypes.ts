import type { StageView } from "../types";
export interface WindowProps { compact?: boolean; paneId?: string; showGroupShortcuts?: boolean; stageView?: StageView; followPreload?: boolean; playbackTab?: "pool" | "cues"; presetFamily?: "All" | "Intensity" | "Color" | "Position" | "Beam"; presetPoolColors?: boolean }
