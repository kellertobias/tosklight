import {
	createContext,
	type PropsWithChildren,
	useContext,
	useMemo,
} from "react";
import type {
	StageLayoutActionOutcome,
	StageLayoutActionRequest,
	StagePositionAxis,
} from "../../api/stageLayoutTypes";
import type { StoredStageLayout } from "../server/contracts";
import type { StageLayoutObject, StageLayoutStore } from "./store";

export interface StageLayoutActions {
	/** Persists the stage layout against the revision this desk currently observes. */
	saveStageLayout(layout: StoredStageLayout): Promise<void>;
	/** Moves the selected fixtures along one axis via the server-side intent route. */
	moveStageSelection(
		fixtureIds: readonly string[],
		axis: StagePositionAxis,
		delta: number,
	): Promise<void>;
}

interface StageLayoutActionsProviderProps {
	store: StageLayoutStore;
	/** Null until a Show is open, which is when stage positions can be persisted. */
	showId: string | null;
	putStageLayout(
		showId: string,
		layout: StoredStageLayout,
		expectedRevision: number,
	): Promise<void>;
	moveStageSelection(
		request: StageLayoutActionRequest,
	): Promise<StageLayoutActionOutcome>;
	readStageLayout(showId: string): Promise<StageLayoutObject | null>;
	onApplied(layout: StageLayoutObject | null): void;
	onError(message: string | null): void;
}

const StageLayoutActionsContext = createContext<StageLayoutActions | null>(null);

/** Mounting this action boundary performs no reads and no network work. */
export function StageLayoutActionsProvider({
	children,
	store,
	showId,
	putStageLayout,
	moveStageSelection,
	readStageLayout,
	onApplied,
	onError,
}: PropsWithChildren<StageLayoutActionsProviderProps>) {
	const actions = useMemo<StageLayoutActions>(
		() => ({
			saveStageLayout: async (layout) => {
				try {
					if (!showId)
						throw new Error("Open a show before saving stage positions");
					await putStageLayout(
						showId,
						layout,
						store.getSnapshot().layout?.revision ?? 0,
					);
					onApplied(await readStageLayout(showId));
					onError(null);
				} catch (reason) {
					onError(reason instanceof Error ? reason.message : String(reason));
				}
			},
			moveStageSelection: async (fixtureIds, axis, delta) => {
				try {
					if (!showId)
						throw new Error("Open a show before moving stage positions");
					await moveStageSelection({
						request_id: crypto.randomUUID(),
						action: {
							type: "move_selection",
							fixture_ids: [...fixtureIds],
							axis,
							delta,
						},
					});
					onApplied(await readStageLayout(showId));
					onError(null);
				} catch (reason) {
					onError(reason instanceof Error ? reason.message : String(reason));
				}
			},
		}),
		[
			moveStageSelection,
			onApplied,
			onError,
			putStageLayout,
			readStageLayout,
			showId,
			store,
		],
	);
	return (
		<StageLayoutActionsContext.Provider value={actions}>
			{children}
		</StageLayoutActionsContext.Provider>
	);
}

export function useStageLayoutActions(): StageLayoutActions | null {
	return useContext(StageLayoutActionsContext);
}
