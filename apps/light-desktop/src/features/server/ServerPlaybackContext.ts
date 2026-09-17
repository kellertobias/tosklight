import type { NetworkEndpointsSnapshot } from "../dmxDiagnostics/networkEndpoints";
import type {
	DmxSnapshot,
	OutputHealth,
	OutputRoute,
	OutputRouteRangeIntent,
	VisualizationSnapshot,
} from "../../api/types";

export interface ServerPlaybackContext {
	readDmx: () => Promise<DmxSnapshot>;
	readOutputHealth: () => Promise<OutputHealth>;
	readNetworkEndpoints: () => Promise<NetworkEndpointsSnapshot>;
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
	createOutputRouteRange: (range: OutputRouteRangeIntent) => Promise<boolean>;
	deleteOutputRoute: (id: string, revision: number) => Promise<boolean>;
}
