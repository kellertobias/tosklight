import { type ScalarDynamicLane } from "../../features/dynamics/laneModel";
import {
	dynamicLaneDomain,
	editableLane,
	graphLane,
	retargetDynamicLane,
} from "../../features/dynamics/editableLane";
import {
	type DynamicLaneDomain,
	formatLaneValue,
} from "../../features/dynamics/laneDomain";
import { ProgrammingLaneRow } from "./ProgrammingLaneView";
import {
	Button,
	CyclingValueToggle,
	FadedDivider,
	GroupedSelectionField,
} from "@tosklight/ui";
import type {
	CSSProperties,
	ReactNode,
	PointerEvent as ReactPointerEvent,
} from "react";
import type {
	DynamicLaneProjection,
	DynamicPeriodicFunctionProjection,
	DynamicScalarSourceProjection,
	DynamicUpdateIntent,
} from "../../api/types";
import type { ShowObject } from "../../features/showObjects/contracts";
import { curveFunctionSelectionGroups } from "./CurveEncoderSlots";
import { addKeyframeToLane, deleteKeyframeFromLane } from "./CurvesView";
import {
	curveComposerMethods,
	keyframeName,
	keyframePreviewPercent,
	keyframePreviewTop,
	LaneAttributeModal,
	lanePreview,
	laneShapeLabel,
	laneSpeedLabel,
	modeLabel,
} from "./DynamicsEditor";

type DynamicObject = ShowObject<"dynamic">;

interface DraggingKeyframe {
	laneId: string;
	index: number;
	pointerId: number;
	mutationGroup: string;
	grabOffsetX: number;
}

interface CurvesSurfaceProps {
	dynamic: DynamicObject;
	lane: ScalarDynamicLane;
	domain: DynamicLaneDomain;
	selectedLanes: ReadonlySet<string>;
	shiftArmed: boolean;
	attributes: readonly { id: string; label: string; family: string }[];
	keyframeIndex: number;
	previewPhase: number | null;
	contentSidebar?: ReactNode;
	attributeLane: DynamicLaneProjection | undefined;
	openLaneMenuId: string | null;
	draggingKeyframe: DraggingKeyframe | null;
	displayedMethod: "keyframes" | "max_min" | "middle_amplitude";
	selectedFunction: DynamicPeriodicFunctionProjection | "random";
	onPrimaryKeyframeIndex(index: number): void;
	onSelect(id: string, additive: boolean): void;
	onReplace(next: ScalarDynamicLane): Promise<void>;
	onMutate(
		dynamic: DynamicObject,
		intent: DynamicUpdateIntent,
		mutationGroup?: string,
	): Promise<void>;
	onAttributeLane(id: string | null): void;
	onToggleLaneMenu(id: string): void;
	onCloseLaneMenu(): void;
	onDraggingKeyframe(value: DraggingKeyframe | null): void;
	onMoveKeyframe(
		lane: ScalarDynamicLane,
		index: number,
		clientX: number,
		timeline: HTMLElement,
		mutationGroup: string,
		repetitions: number,
		grabOffsetX: number,
	): void;
	onChooseMethod(mode: "keyframes" | "max_min" | "middle_amplitude"): void;
	onChooseFunction(
		functionName: DynamicPeriodicFunctionProjection | "random",
	): void;
}

export function CurvesViewSurface(props: CurvesSurfaceProps) {
	return (
		<div
			className={`dynamic-curves-view ${props.contentSidebar ? "has-content-sidebar" : ""}`}
		>
			<LaneOverviewList {...props} />
			{props.contentSidebar}
			<AttributeLaneEditor {...props} />
			<CurveComposer {...props} />
		</div>
	);
}

function LaneOverviewList(props: CurvesSurfaceProps) {
	return (
		<ul className="dynamic-lane-overview-list" aria-label="Dynamic lanes">
			{props.dynamic.body.lanes.map((lane, index) => {
				const candidate = editableLane(lane);
				const domain = dynamicLaneDomain(lane);
				return candidate && domain ? (
					<LaneOverviewRow
						key={candidate.id}
						{...props}
						candidate={candidate}
						candidateDomain={domain}
						index={index}
					/>
				) : (
					<ProgrammingLaneRow
						key={lane.id}
						lane={lane}
						index={index}
						selected={props.selectedLanes.has(lane.id)}
						onSelect={props.onSelect}
					/>
				);
			})}
		</ul>
	);
}

function LaneOverviewRow({
	candidate,
	candidateDomain,
	index,
	...props
}: CurvesSurfaceProps & {
	candidate: ScalarDynamicLane;
	candidateDomain: DynamicLaneDomain;
	index: number;
}) {
	const attribute =
		props.attributes.find((item) => item.id === candidate.attribute) ?? null;
	const selected = props.selectedLanes.has(candidate.id);
	// Curves draw on the lane's own descriptor range: degrees for an Angle, 0–100 % for a level.
	const graph = graphLane(candidate, candidateDomain);
	const preview = lanePreview(graph, props.dynamic.body.lanes);
	const label =
		attribute?.label ??
		(candidateDomain.address ? candidateDomain.label : candidate.attribute);
	return (
		<li
			aria-current={candidate.id === props.lane.id}
			className={`dynamic-lane-overview ${candidate.id === props.lane.id ? "primary" : ""} ${selected ? "selected" : ""}`}
		>
			<div className="dynamic-lane-content">
				<Button
					className="dynamic-lane-identity-select"
					aria-label={`Select lane ${index + 1}, ${label}`}
					aria-pressed={selected}
					onClick={(event) =>
						props.onSelect(candidate.id, event.shiftKey || props.shiftArmed)
					}
				>
					<span className="dynamic-lane-identity">
						<small>Lane {index + 1}</small>
						<strong>{label}</strong>
						<span>
							{modeLabel(candidate.mode)}
							{laneSpeedLabel(candidate)}
						</span>
					</span>
				</Button>
				<LaneCurve
					{...props}
					candidate={candidate}
					graph={graph}
					label={label}
					selected={selected}
					preview={preview}
				/>
			</div>
			<div className="dynamic-lane-row-actions">
				<Button
					iconOnly
					className="dynamic-lane-settings-trigger"
					icon="⚙"
					aria-label={`${label} lane settings`}
					aria-expanded={props.openLaneMenuId === candidate.id}
					onClick={() => props.onToggleLaneMenu(candidate.id)}
				/>
			</div>
			{props.openLaneMenuId === candidate.id && (
				<LaneMenu {...props} candidate={candidate} label={label} />
			)}
		</li>
	);
}

type LanePreview = ReturnType<typeof lanePreview>;

function LaneCurve({
	candidate,
	graph,
	label,
	selected,
	preview,
	...props
}: CurvesSurfaceProps & {
	candidate: ScalarDynamicLane;
	graph: ScalarDynamicLane;
	label: string;
	selected: boolean;
	preview: LanePreview;
}) {
	return (
		<div className="dynamic-lane-curve">
			<Button
				className="dynamic-lane-curve-select"
				aria-label={`Select ${label} lane from curve`}
				aria-pressed={selected}
				onClick={(event) =>
					props.onSelect(candidate.id, event.shiftKey || props.shiftArmed)
				}
			>
				<svg
					viewBox="0 0 1000 200"
					preserveAspectRatio="none"
					role="img"
					aria-label={`${label}: ${modeLabel(candidate.mode)}`}
				>
					<title>{modeLabel(candidate.mode)}</title>
					<path
						className="grid"
						d="M0 50H1000M0 100H1000M0 150H1000M250 0V200M500 0V200M750 0V200"
					/>
					<path className="curve" d={preview.primaryPath} />
					{preview.repeatedPath && (
						<path className="curve repeated" d={preview.repeatedPath} />
					)}
					{preview.repetitions > 1 && (
						<path
							className="repeat-boundary"
							d={`M${1000 / preview.repetitions} 0V200`}
						/>
					)}
				</svg>
				<LaneCurveAnnotations preview={preview} phase={props.previewPhase} />
			</Button>
			{candidate.mode === "keyframes" && (
				<KeyframeMarks
					{...props}
					candidate={candidate}
					graph={graph}
					label={label}
					preview={preview}
				/>
			)}
		</div>
	);
}

function LaneCurveAnnotations({
	preview,
	phase,
}: {
	preview: LanePreview;
	phase: number | null;
}) {
	return (
		<>
			{preview.repetitions > 1 && (
				<span
					className="dynamic-repeat-label"
					style={{ left: `${100 / preview.repetitions}%` }}
				>
					repeat
				</span>
			)}
			{phase !== null && (
				<i
					className="dynamic-preview-playhead"
					style={{ left: `${phase * 100}%` }}
				/>
			)}
			<span className="dynamic-lane-axis start">0%</span>
			<span className="dynamic-lane-axis middle">50%</span>
			<span className="dynamic-lane-axis end">100%</span>
		</>
	);
}

function KeyframeMarks({
	candidate,
	graph,
	label,
	preview,
	...props
}: CurvesSurfaceProps & {
	candidate: ScalarDynamicLane;
	graph: ScalarDynamicLane;
	label: string;
	preview: LanePreview;
}) {
	return (
		<span className="dynamic-keyframe-marks">
			{candidate.keyframes.points.map((point, pointIndex) => (
				<Button
					key={`${candidate.id}-${pointIndex}`}
					aria-label={`${label} keyframe ${keyframeName(pointIndex)}`}
					className={
						candidate.id === props.lane.id && pointIndex === props.keyframeIndex
							? "selected"
							: ""
					}
					style={
						{
							left: `${keyframePreviewPercent(point.position, preview.repetitions)}%`,
							top: `${keyframePreviewTop(graph.keyframes.points[pointIndex]?.source)}%`,
						} as CSSProperties
					}
					onPointerDown={(event) =>
						beginKeyframeDrag(event, candidate, pointIndex, props)
					}
					onPointerMove={(event) =>
						continueKeyframeDrag(event, candidate, pointIndex, preview, props)
					}
					onPointerUp={(event) => {
						if (props.draggingKeyframe?.pointerId === event.pointerId)
							props.onDraggingKeyframe(null);
					}}
					onPointerCancel={() => props.onDraggingKeyframe(null)}
				>
					<span>{keyframeName(pointIndex)}</span>
				</Button>
			))}
			<i
				className="loop-close"
				style={{
					left: `${keyframePreviewPercent(1, preview.repetitions)}%`,
					top: `${keyframePreviewTop(graph.keyframes.points[0]?.source)}%`,
				}}
			>
				<span>A′</span>
			</i>
		</span>
	);
}

function beginKeyframeDrag(
	event: ReactPointerEvent<HTMLButtonElement>,
	candidate: ScalarDynamicLane,
	index: number,
	props: CurvesSurfaceProps,
) {
	event.preventDefault();
	event.stopPropagation();
	props.onSelect(candidate.id, false);
	props.onPrimaryKeyframeIndex(index);
	if (index === 0) return;
	event.currentTarget.setPointerCapture(event.pointerId);
	const bounds = event.currentTarget.getBoundingClientRect();
	props.onDraggingKeyframe({
		laneId: candidate.id,
		index,
		pointerId: event.pointerId,
		mutationGroup: crypto.randomUUID(),
		grabOffsetX: event.clientX - (bounds.left + bounds.width / 2),
	});
}

function continueKeyframeDrag(
	event: ReactPointerEvent<HTMLButtonElement>,
	candidate: ScalarDynamicLane,
	index: number,
	preview: LanePreview,
	props: CurvesSurfaceProps,
) {
	const dragging = props.draggingKeyframe;
	if (
		!dragging ||
		dragging.laneId !== candidate.id ||
		dragging.index !== index ||
		dragging.pointerId !== event.pointerId
	)
		return;
	const timeline = event.currentTarget.parentElement;
	if (timeline)
		props.onMoveKeyframe(
			candidate,
			index,
			event.clientX,
			timeline,
			dragging.mutationGroup,
			preview.repetitions,
			dragging.grabOffsetX,
		);
}

function LaneMenu({
	candidate,
	label,
	...props
}: CurvesSurfaceProps & {
	candidate: ScalarDynamicLane;
	label: string;
}) {
	return (
		<div
			className="dynamic-lane-menu"
			role="menu"
			aria-label={`${label} lane menu`}
		>
			<Button
				role="menuitem"
				onClick={() => {
					props.onCloseLaneMenu();
					props.onAttributeLane(candidate.id);
				}}
			>
				<span aria-hidden="true">✎</span>
				Change attribute
			</Button>
			<Button
				role="menuitem"
				variant="danger"
				disabled={props.dynamic.body.lanes.length <= 1}
				title={
					props.dynamic.body.lanes.length <= 1
						? "A Dynamic requires at least one lane."
						: undefined
				}
				onClick={() => {
					props.onCloseLaneMenu();
					void props.onMutate(props.dynamic, {
						type: "delete_lane",
						lane_id: candidate.id,
					});
				}}
			>
				<span aria-hidden="true">⌫</span>
				Delete lane
			</Button>
			<Button
				iconOnly
				className="dynamic-lane-menu-close"
				icon="×"
				aria-label="Close lane settings"
				onClick={props.onCloseLaneMenu}
			/>
		</div>
	);
}

function AttributeLaneEditor(props: CurvesSurfaceProps) {
	if (!props.attributeLane) return null;
	return (
		<LaneAttributeModal
			id={`change-lane-attribute-${props.attributeLane.id}`}
			title="Change lane attribute"
			details="Choose the attribute controlled by this lane"
			currentAttribute={editableLane(props.attributeLane)?.attribute}
			attributes={props.attributes}
			onClose={() => props.onAttributeLane(null)}
			onChoose={(attribute) => {
				const target = props.dynamic.body.lanes.find(
					(candidate) => candidate.id === props.attributeLane?.id,
				);
				if (!target) return;
				props.onAttributeLane(null);
				void props.onMutate(props.dynamic, {
					type: "replace_lane",
					lane_id: target.id,
					lane: retargetDynamicLane(target, attribute),
				});
			}}
		/>
	);
}

function CurveComposer(props: CurvesSurfaceProps) {
	const lane = props.lane;
	return (
		<section className="dynamic-lane-bottom-editor" aria-label="Curve Composer">
			<CyclingValueToggle
				className="dynamic-curve-method-cycle"
				ariaLabel="Curve method"
				value={props.displayedMethod}
				options={curveComposerMethods}
				onChange={props.onChooseMethod}
			/>
			<FadedDivider
				orientation="vertical"
				className="dynamic-curve-composer-divider"
			/>
			{props.displayedMethod === "keyframes" ? (
				<KeyframeChoices {...props} />
			) : (
				<GroupedSelectionField
					className="dynamic-composer-choice"
					ariaLabel={`Curve function: ${laneShapeLabel(lane)}`}
					dialogTitle="Choose curve function"
					value={props.selectedFunction}
					groups={curveFunctionSelectionGroups()}
					onChange={props.onChooseFunction}
				/>
			)}
			{props.displayedMethod === "keyframes" && <KeyframeActions {...props} />}
		</section>
	);
}

function KeyframeChoices(props: CurvesSurfaceProps) {
	return (
		<fieldset className="dynamic-keyframe-choice-list">
			<legend className="dynamic-keyframe-choice-legend">
				Selected keyframe
			</legend>
			{props.lane.keyframes.points.map((point, index) => (
				<Button
					key={`${props.lane.id}-${index}`}
					className="dynamic-keyframe-choice"
					active={index === props.keyframeIndex}
					aria-pressed={index === props.keyframeIndex}
					aria-label={`${keyframeName(index)}, ${Math.round(point.position * 100)}%, ${sourceLabel(point.source, props.domain)}`}
					onClick={() => props.onPrimaryKeyframeIndex(index)}
				>
					<b aria-hidden="true">{keyframeName(index)}</b>
					<span aria-hidden="true">{Math.round(point.position * 100)}%</span>
					<small aria-hidden="true">
						{sourceLabel(point.source, props.domain)}
					</small>
				</Button>
			))}
			<span
				className="dynamic-keyframe-choice loop-close"
				aria-label="A prime, 100%, alias of A"
				role="note"
			>
				<b aria-hidden="true">A′</b>
				<span aria-hidden="true">100%</span>
				<small aria-hidden="true">Alias of A</small>
			</span>
		</fieldset>
	);
}

function KeyframeActions(props: CurvesSurfaceProps) {
	const lane = props.lane;
	return (
		<>
			<Button
				size="compact"
				variant="danger"
				disabled={
					props.keyframeIndex === 0 || lane.keyframes.points.length <= 2
				}
				aria-label={`Delete keyframe ${keyframeName(props.keyframeIndex)}`}
				title={
					props.keyframeIndex === 0
						? "The first keyframe also closes the loop and cannot be deleted."
						: lane.keyframes.points.length <= 2
							? "A curve requires at least two keyframes."
							: undefined
				}
				onClick={() => {
					void props.onReplace(
						deleteKeyframeFromLane(lane, props.keyframeIndex),
					);
					props.onPrimaryKeyframeIndex(Math.max(0, props.keyframeIndex - 1));
				}}
			>
				Delete Keyframe
			</Button>
			<Button
				size="compact"
				onClick={() => {
					const positions = new Set(
						lane.keyframes.points.map((point) => point.position),
					);
					const next = addKeyframeToLane(lane);
					const index = next.keyframes.points.findIndex(
						(point) => !positions.has(point.position),
					);
					void props.onReplace(next);
					if (index >= 0) props.onPrimaryKeyframeIndex(index);
				}}
			>
				+ Keyframe
			</Button>
		</>
	);
}

/** A keyframe source in descriptor units: `45°`, `80%`, `Current` or `Preset`. */
export function sourceLabel(
	source: DynamicScalarSourceProjection | undefined,
	domain: DynamicLaneDomain,
) {
	if (!source) return "—";
	if (source.type === "current") return "Current";
	if (source.type === "preset") return "Preset";
	return formatLaneValue(domain, source.value);
}
