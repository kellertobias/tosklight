import { type PointerEvent, useEffect, useMemo, useRef, useState } from "react";
import { useServer } from "../../../api/ServerContext";
import { Button } from "../../common";
import {
	moveLampPositions,
	resolveLampPositions,
	returnHomeAssignments,
} from "../specialPosition";
import { normalizedPointerPosition } from "./pointer";

interface LampPosition {
	pan: number;
	tilt: number;
}

interface PositionDialogController {
	pan: number;
	tilt: number;
	joystick: React.RefObject<{ x: number; y: number }>;
	trackball: React.RefObject<HTMLDivElement | null>;
	homeDisabled: boolean;
	movePosition: (event: PointerEvent<HTMLDivElement>) => void;
	releasePosition: () => void;
	returnHome: () => Promise<void>;
}

function averagePosition(positions: Map<string, LampPosition>) {
	const values = [...positions.values()];
	if (!values.length) return null;
	return {
		pan: values.reduce((sum, value) => sum + value.pan, 0) / values.length,
		tilt: values.reduce((sum, value) => sum + value.tilt, 0) / values.length,
	};
}

export function usePositionDialog(
	active: boolean,
	selectedFixtureKey: string,
): PositionDialogController {
	const server = useServer();
	const [pan, setPan] = useState(0.5);
	const [tilt, setTilt] = useState(0.5);
	const trackball = useRef<HTMLDivElement>(null);
	const joystick = useRef({ x: 0, y: 0 });
	const fixturePositions = useRef(new Map<string, LampPosition>());
	const homeAssignments = useMemo(
		() =>
			returnHomeAssignments(
				server.selectedFixtures,
				server.patch?.fixtures ?? [],
			),
		[server.patch, server.selectedFixtures],
	);

	const updateAverages = (positions: Map<string, LampPosition>) => {
		const averages = averagePosition(positions);
		if (!averages) return;
		setPan(averages.pan);
		setTilt(averages.tilt);
	};

	const movePosition = (event: PointerEvent<HTMLDivElement>) => {
		const next = normalizedPointerPosition(event, trackball);
		joystick.current = { x: (next.x - 0.5) * 2, y: (next.y - 0.5) * 2 };
	};

	const releasePosition = () => {
		joystick.current = { x: 0, y: 0 };
	};

	const returnHome = async () => {
		if (!(await server.setProgrammerMany(homeAssignments))) return;
		const positions = new Map(fixturePositions.current);
		for (const assignment of homeAssignments) {
			const position = positions.get(assignment.fixtureId);
			if (position) position[assignment.attribute] = assignment.value;
		}
		fixturePositions.current = positions;
		updateAverages(positions);
	};

	useEffect(() => {
		if (!active) return;
		let cancelled = false;
		void server
			.readVisualization()
			.then((snapshot) => {
				if (cancelled) return;
				const origins = resolveLampPositions(
					server.selectedFixtures,
					server.patch?.fixtures ?? [],
					snapshot,
				);
				fixturePositions.current = origins;
				updateAverages(origins);
			})
			.catch(() => undefined);
		return () => {
			cancelled = true;
		};
	}, [active, selectedFixtureKey]);

	useEffect(() => {
		if (!active) return;
		const timer = window.setInterval(() => {
			const vector = joystick.current;
			const magnitude = Math.min(1, Math.hypot(vector.x, vector.y));
			if (magnitude < 0.04) return;
			const speed = 0.002 + magnitude * magnitude * 0.028;
			const positions = fixturePositions.current;
			if (!positions.size) return;
			const updates: Promise<void>[] = [];
			moveLampPositions(positions, vector.x, vector.y, speed);
			for (const [fixture, position] of positions) {
				updates.push(server.setProgrammer(fixture, "pan", position.pan));
				updates.push(server.setProgrammer(fixture, "tilt", position.tilt));
			}
			void Promise.all(updates);
			updateAverages(positions);
		}, 32);
		return () => window.clearInterval(timer);
	}, [active, selectedFixtureKey]);

	return {
		pan,
		tilt,
		joystick,
		trackball,
		homeDisabled: homeAssignments.length === 0,
		movePosition,
		releasePosition,
		returnHome,
	};
}

export function PositionDialog({
	pan,
	tilt,
	joystick,
	trackball,
	homeDisabled,
	movePosition,
	releasePosition,
	returnHome,
}: PositionDialogController) {
	return (
		<div className="position-trackball-layout">
			<div
				ref={trackball}
				className="position-trackball"
				onPointerDown={(event) => {
					event.currentTarget.setPointerCapture(event.pointerId);
					movePosition(event);
				}}
				onPointerMove={(event) => {
					if (event.currentTarget.hasPointerCapture(event.pointerId))
						movePosition(event);
				}}
				onPointerUp={releasePosition}
				onPointerCancel={releasePosition}
				onLostPointerCapture={releasePosition}
			>
				<i
					className="joystick-handle"
					style={{
						left: `${50 + joystick.current.x * 38}%`,
						top: `${50 + joystick.current.y * 38}%`,
					}}
				/>
			</div>
			<span className="position-trackball-readout">
				Relative move
				<br />
				<b>Avg Pan {Math.round(pan * 100)}%</b>
				<b>Avg Tilt {Math.round(tilt * 100)}%</b>
				<Button disabled={homeDisabled} onClick={() => void returnHome()}>
					Return Home
				</Button>
			</span>
		</div>
	);
}
