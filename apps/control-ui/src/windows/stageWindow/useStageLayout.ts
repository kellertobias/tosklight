import { useEffect, useRef, useState } from "react";
import type { StagePosition3d } from "../../api/ServerContext";
import { useStageLayoutActions } from "../../features/stageLayout/StageLayoutActionsProvider";
import {
	useStagePositions,
	useStagePositions3d,
} from "../../features/stageLayout/StageLayoutState";
import { useServer } from "../../api/ServerContext";
import type { StageLayoutModel } from "./types";

type Position2d = { x: number; y: number; rotation: number };

export function useStageLayout(): StageLayoutModel {
	const server = useServer();
	const [positions, setPositions] = useState<Record<string, Position2d>>({});
	const [positions3d, setPositions3d] = useState<
		Record<string, StagePosition3d>
	>({});
	const positionsRef = useRef(positions);
	const positions3dRef = useRef(positions3d);
	const storedPositions = useStagePositions();
	const storedPositions3d = useStagePositions3d();
	const stageLayoutActions = useStageLayoutActions();
	useEffect(() => {
		positionsRef.current = positions;
	}, [positions]);
	useEffect(() => {
		positions3dRef.current = positions3d;
	}, [positions3d]);
	useEffect(() => {
		positionsRef.current = storedPositions;
		setPositions(storedPositions);
		positions3dRef.current = storedPositions3d;
		setPositions3d(storedPositions3d);
	}, [storedPositions, storedPositions3d]);
	const updatePosition2d = (fixtureId: string, position: Position2d) => {
		setPositions((current) => {
			const next = { ...current, [fixtureId]: position };
			positionsRef.current = next;
			return next;
		});
	};
	const updatePosition3d = (fixtureId: string, position: StagePosition3d) => {
		setPositions3d((current) => {
			const next = { ...current, [fixtureId]: position };
			positions3dRef.current = next;
			return next;
		});
	};
	const save = async () => {
		await stageLayoutActions?.saveStageLayout({
			version: 2,
			positions: positionsRef.current,
			positions3d: positions3dRef.current,
		});
	};
	const savePosition3d = (fixtureId: string, position: StagePosition3d) => {
		const next = { ...positions3dRef.current, [fixtureId]: position };
		positions3dRef.current = next;
		return (
			stageLayoutActions?.saveStageLayout({
				version: 2,
				positions: positionsRef.current,
				positions3d: next,
			}) ?? Promise.resolve()
		);
	};
	return {
		positions,
		positions3d,
		updatePosition2d,
		updatePosition3d,
		save,
		savePosition3d,
	};
}
