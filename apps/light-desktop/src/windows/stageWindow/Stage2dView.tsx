import { Button } from "@tosklight/ui";
import { type CSSProperties, useRef } from "react";
import type { PatchedFixture } from "../../api/types";
import type { VisualizationRuntimeLane } from "../../features/visualizationRuntime/contracts";
import { useVisualizationRuntimeSnapshotSubscription } from "../../features/visualizationRuntime/VisualizationRuntimeView";
import type {
	StageFixturePresentation,
	StageLayoutModel,
	StageOptionsModel,
} from "./types";
import {
	useStageCanvasGestures,
	useStageFixtureGestures,
} from "./useStage2dGestures";
import type { StageSelectionModel } from "./useStageSelection";
import { fixturePresentation } from "./useStageVisualization";

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
	patchedFixtures = [],
	patchSelectionPreview = false,
	patchPreviewFixtures = [],
	visualizationLane = "normal",
	visualizationActive = false,
	interactive = true,
}: {
	compact?: boolean;
	fixtures: StageFixturePresentation[];
	layout: StageLayoutModel;
	options: StageOptionsModel;
	selection: StageSelectionModel;
	patchedFixtures?: readonly PatchedFixture[];
	patchSelectionPreview?: boolean;
	patchPreviewFixtures?: readonly string[];
	visualizationLane?: VisualizationRuntimeLane;
	visualizationActive?: boolean;
	interactive?: boolean;
}) {
	const rootRef = useRef<HTMLDivElement>(null);
	useVisualizationRuntimeSnapshotSubscription(
		visualizationLane,
		visualizationActive,
		(snapshot) => {
			const root = rootRef.current;
			if (!root) return;
			const buttons = new Map(
				[
					...root.querySelectorAll<HTMLElement>(
						".stage-fixture[data-fixture-id]",
					),
				].map((button) => [button.dataset.fixtureId, button]),
			);
			patchedFixtures.forEach((fixture, index) => {
				const button = buttons.get(fixture.fixture_id);
				if (!button) return;
				applyFixtureVisualization(
					button,
					fixturePresentation(
						fixture,
						index,
						snapshot,
						patchSelectionPreview &&
							patchPreviewFixtures.includes(fixture.fixture_id),
					),
				);
			});
		},
	);
	const orderedFixtureIds = fixtures
		.map((fixture) => fixture.fixtureId)
		.filter(Boolean);
	const fixtureInteractions = useStageFixtureGestures(
		options.mode,
		orderedFixtureIds,
		selection,
		interactive,
	);
	const canvas = useStageCanvasGestures(options.mode, selection, interactive);
	const columns = compact ? 6 : 8;
	return (
		<div
			className={`stage-canvas stage-mode-${options.mode}`}
			data-interaction={interactive ? "enabled" : "view-only"}
			ref={rootRef}
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
							options.showSelection &&
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

function applyFixtureVisualization(
	button: HTMLElement,
	fixture: StageFixturePresentation,
) {
	button.style.color = fixture.color;
	button.style.setProperty("--lamp-fill", `${12 + fixture.dimmer * 0.36}%`);
	button.style.setProperty("--lamp-ring", `${20 + fixture.dimmer * 0.65}%`);
	button.setAttribute("aria-label", `${fixture.name}, ${fixture.dimmer}%`);
	const dot = button.querySelector<HTMLElement>(".lamp-color-dot");
	if (dot) dot.style.background = fixture.color;
	const line = button.querySelector<HTMLElement>(".lamp-position-line");
	if (!line) return;
	line.classList.toggle("active", fixture.dimmer > 0);
	line.classList.toggle("inactive", fixture.dimmer <= 0);
	line.style.transform = `rotate(${fixture.pan * 360 - 180}deg)`;
	line.style.color = fixture.dimmer > 0 ? fixture.color : "";
	const tilt = line.querySelector<HTMLElement>("i");
	if (tilt) tilt.style.left = `${fixture.tilt * 100}%`;
}
