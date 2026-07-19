import type { HydrationTarget } from "./scope";
import type {
	ShowObjectCollectionLoader,
	ShowObjectLoader,
} from "./hydration";
import type { ShowObjectsStore } from "./store";
import type { ShowObjectsEventTransport } from "./transport";

export interface ShowObjectsSessionOptions {
	showId: string;
	store: ShowObjectsStore;
	transport: ShowObjectsEventTransport | null;
	loadCollection: ShowObjectCollectionLoader;
	loadObject: ShowObjectLoader;
	onError?: (error: Error | null) => void;
}

export interface HydrationRun {
	token: symbol;
	target: HydrationTarget;
	floor: number;
}

export interface SnapshotBoundary {
	generation: number;
	cursor: number;
	repair: boolean;
	targets: Set<string>;
}
