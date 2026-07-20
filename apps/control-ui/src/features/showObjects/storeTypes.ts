import type {
	ShowObject,
	ShowObjectBodies,
	ShowObjectCollections,
	ShowObjectKind,
} from "./contracts";

export interface ShowObjectsSnapshot {
	showId: string | null;
	authorityGeneration: number;
	showRevision: number | null;
	eventSequence: number | null;
	groups: readonly ShowObject<"group">[];
	presets: readonly ShowObject<"preset">[];
	readyCollections: ReadonlySet<ShowObjectKind>;
	pendingObjectKeys: ReadonlySet<string>;
	status: "idle" | "loading" | "ready" | "error";
	error: Error | null;
}

export type CollectionUpdate<K extends ShowObjectKind> =
	| ShowObjectCollections[K]
	| ((current: ShowObjectCollections[K]) => ShowObjectCollections[K]);

export interface PendingMutation<K extends ShowObjectKind = ShowObjectKind> {
	token: string;
	showId: string;
	kind: K;
	objectId: string;
	body: ShowObjectBodies[K] | null;
	baseEventSequence: number;
}

export interface ShowObjectInstall {
	kind: ShowObjectKind;
	objectId: string;
	object: ShowObject | null;
}
