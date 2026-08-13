import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useMemo,
} from "react";
import type {
	StageLayoutActionOutcome,
	StageLayoutApiClient,
	StageProjection2d,
} from "../../api/client/stageLayout";

interface StageLayoutActions {
	canWrite: boolean;
	regenerate2d(
		projection: StageProjection2d,
	): Promise<StageLayoutActionOutcome>;
	setCrowdFootprint(
		fixtureId: string,
		widthMetres: number,
		depthMetres: number,
	): Promise<StageLayoutActionOutcome>;
}

const StageLayoutActionsContext = createContext<StageLayoutActions | null>(
	null,
);

export function StageLayoutActionsProvider({
	children,
	client,
	showId,
	canWrite,
}: PropsWithChildren<{
	client: StageLayoutApiClient;
	showId: string | null;
	canWrite: boolean;
}>) {
	const regenerate2d = useCallback(
		(projection: StageProjection2d) => {
			if (!canWrite || !showId)
				return Promise.reject(
					new Error("The primary desk is not ready to edit the Stage layout"),
				);
			return client.regenerate2d(showId, projection);
		},
		[canWrite, client, showId],
	);
	const setCrowdFootprint = useCallback(
		(fixtureId: string, widthMetres: number, depthMetres: number) => {
			if (!canWrite || !showId)
				return Promise.reject(
					new Error("The primary desk is not ready to edit the Stage layout"),
				);
			return client.setCrowdFootprint(
				showId,
				fixtureId,
				widthMetres,
				depthMetres,
			);
		},
		[canWrite, client, showId],
	);
	const actions = useMemo(
		() => ({
			canWrite: canWrite && Boolean(showId),
			regenerate2d,
			setCrowdFootprint,
		}),
		[canWrite, regenerate2d, setCrowdFootprint, showId],
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
