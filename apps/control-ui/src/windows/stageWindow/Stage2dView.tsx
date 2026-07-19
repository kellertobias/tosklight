import type { CSSProperties } from "react";
import { Button } from "../../components/common";
import { useApp } from "../../state/AppContext";
import type {
	StageFixturePresentation,
	StageLayoutModel,
	StageOptionsModel,
} from "./types";
import type { StageSelectionModel } from "./useStageSelection";
import {
	useStageCanvasGestures,
	useStageFixtureGestures,
} from "./useStage2dGestures";

const symbols = ["◉", "◈", "◎", "◐", "◇", "◍"];

function StageFixtureButton({
	fixture,
	index,
	position,
	selected,
	interactions,
}: {
	fixture: StageFixturePresentation;
	index: number;
	position: { x: number; y: number; rotation: number };
	selected: boolean;
	interactions: ReturnType<typeof useStageFixtureGestures>;
}) {
	return (
		<Button
			data-fixture-id={fixture.fixtureId || undefined}
			onClick={(event) => interactions.select(fixture.fixtureId, event)}
			onPointerDown={(event) =>
				interactions.beginMove(fixture.fixtureId, event)
			}
			onPointerMove={(event) => interactions.move(fixture.fixtureId, event)}
			onPointerUp={interactions.finishMove}
			key={fixture.fixtureId || index}
			className={`stage-fixture ${selected ? "selected" : ""}`}
			style={
				{
					left: `${position.x}%`,
					top: `${position.y}%`,
					color: fixture.color,
					"--lamp-fill": `${12 + fixture.dimmer * 0.36}%`,
					"--lamp-ring": `${20 + fixture.dimmer * 0.65}%`,
				} as CSSProperties
			}
			aria-label={`${fixture.name}, ${fixture.dimmer}%`}
		>
			<span>
				{fixture.icon ? (
					<img src={fixture.icon} alt="" />
				) : (
					symbols[index % symbols.length]
				)}
				<i className="lamp-color-dot" style={{ background: fixture.color }} />
			</span>
			<i
				className={`lamp-position-line ${fixture.dimmer > 0 ? "active" : "inactive"}`}
				style={{
					transform: `rotate(${fixture.pan * 360 - 180}deg)`,
					color: fixture.dimmer > 0 ? fixture.color : undefined,
				}}
			>
				<i style={{ left: `${fixture.tilt * 100}%` }} />
			</i>
			<small>{fixture.fixtureNumber}</small>
		</Button>
	);
}

export function Stage2dView({
	compact,
	fixtures,
	layout,
	options,
	selection,
}: {
	compact?: boolean;
	fixtures: StageFixturePresentation[];
	layout: StageLayoutModel;
	options: StageOptionsModel;
	selection: StageSelectionModel;
}) {
	const { state } = useApp();
	const orderedFixtureIds = fixtures
		.map((fixture) => fixture.fixtureId)
		.filter(Boolean);
	const fixtureInteractions = useStageFixtureGestures(
		options.mode,
		orderedFixtureIds,
		layout,
		selection,
	);
	const canvas = useStageCanvasGestures(options.mode, selection);
	const columns = compact ? 6 : 8;
	return (
		<div
			className={`stage-canvas stage-mode-${options.mode}`}
			onPointerDown={canvas.begin}
			onPointerMove={canvas.update}
			onPointerUp={canvas.finish}
			onPointerCancel={canvas.cancel}
		>
			{fixtures.length === 0 && (
				<div className="empty-window-message">
					No fixtures are patched in the active show.
				</div>
			)}
			<div
				className="stage-fixture-layer"
				style={{
					transform: `translate(${canvas.pan.x}px,${canvas.pan.y}px) scale(${canvas.zoom})`,
				}}
			>
				{fixtures.slice(0, compact ? 18 : 24).map((fixture, index) => (
					<StageFixtureButton
						key={fixture.fixtureId || index}
						fixture={fixture}
						index={index}
						position={
							layout.positions[fixture.fixtureId] ?? {
								x: 8 + (index % columns) * (compact ? 15 : 11.5),
								y: 12 + Math.floor(index / columns) * 31,
								rotation: index * 23 - 70,
							}
						}
						selected={
							state.stageShowSelection &&
							selection.fixtureIdSet.has(fixture.fixtureId)
						}
						interactions={fixtureInteractions}
					/>
				))}
			</div>
			{canvas.marquee && (
				<div
					className="selection-marquee"
					style={canvas.marquee}
					aria-hidden="true"
				/>
			)}
		</div>
	);
}
