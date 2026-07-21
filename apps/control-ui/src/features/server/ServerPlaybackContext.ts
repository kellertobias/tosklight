import type {
	DmxSnapshot,
	OutputRoute,
	VisualizationSnapshot,
} from "../../api/types";

export interface ServerPlaybackContext {
	playbackAction: (
		cueListId: string,
		action: "go" | "back" | "pause" | "release",
	) => Promise<void>;
	readDmx: () => Promise<DmxSnapshot>;
	readVisualization: (preload?: boolean) => Promise<VisualizationSnapshot>;
	setDmxOverride: (
		universe: number,
		address: number,
		value: number | null,
	) => Promise<void>;
	saveOutputRoute: (
		id: string,
		route: OutputRoute,
		revision: number,
	) => Promise<boolean>;
	deleteOutputRoute: (id: string, revision: number) => Promise<boolean>;
}
