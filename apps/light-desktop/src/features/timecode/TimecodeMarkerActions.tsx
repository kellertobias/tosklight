import { Button, InputModal } from "@tosklight/ui";
import { useEffect, useState } from "react";
import type { TimecodeDefinition } from "../../api/types/timecode";
import type { TimecodeEditorSelection } from "./editorModel";
import { sameSelection } from "./editorModel";
import {
	formatFrame,
	MarkerColorButton,
	parseTimelineFrame,
	type TimelineItem,
} from "./timecodeEditorShared";

type Marker = TimecodeDefinition["markers"][number];

function StackedButton({
	label,
	top,
	bottom,
	className = "timecode-keyframe-action",
	disabled,
	title,
	onClick,
}: {
	label: string;
	top: string;
	bottom: string;
	className?: string;
	disabled?: boolean;
	title?: string;
	onClick(): void;
}) {
	return (
		<Button
			className={className}
			aria-label={label}
			size="compact"
			disabled={disabled}
			title={title}
			onClick={onClick}
		>
			<span>
				{top}
				<br />
				{bottom}
			</span>
		</Button>
	);
}

/// Opens the timecode keypad for the selected marker and refuses a position
/// outside the timeline instead of committing one.
function MarkerMoveToButton({
	marker,
	fps,
	lastFrame,
	update,
}: {
	marker: Marker;
	fps: number;
	lastFrame: number;
	update(patch: Partial<Marker>): void;
}) {
	const [open, setOpen] = useState(false);
	const [invalid, setInvalid] = useState(false);
	useEffect(() => {
		setOpen(false);
		setInvalid(false);
	}, [marker.id]);
	const position = formatFrame(marker.frame, fps);
	return (
		<>
			<StackedButton
				label="Move To"
				top="Move"
				bottom="To"
				onClick={() => {
					setInvalid(false);
					setOpen(true);
				}}
			/>
			{open && (
				<InputModal
					kind="text"
					label="Marker timecode"
					value={position}
					initialCaret={position.length}
					error={invalid ? "Invalid timecode" : undefined}
					onDraftChange={() => setInvalid(false)}
					onCancel={() => {
						setInvalid(false);
						setOpen(false);
					}}
					onCommit={(candidate) => {
						const target = parseTimelineFrame(candidate, fps);
						if (target === null || target < 0 || target > lastFrame) {
							setInvalid(true);
							return;
						}
						setInvalid(false);
						setOpen(false);
						if (target !== marker.frame) update({ frame: target });
					}}
				/>
			)}
		</>
	);
}

function MarkerNameButton({
	marker,
	update,
}: {
	marker: Marker;
	update(patch: Partial<Marker>): void;
}) {
	const [open, setOpen] = useState(false);
	useEffect(() => setOpen(false), [marker.id]);
	return (
		<>
			<StackedButton
				label="Set Name"
				top="Set"
				bottom="Name"
				onClick={() => setOpen(true)}
			/>
			{open && (
				<InputModal
					kind="text"
					label="Marker name"
					value={marker.name}
					initialCaret={marker.name.length}
					onCancel={() => setOpen(false)}
					onCommit={(candidate) => {
						const next = candidate.trim() || marker.name;
						if (next !== marker.name) update({ name: next });
						setOpen(false);
					}}
				/>
			)}
		</>
	);
}

export function MarkerActionStrip({
	definition,
	selection,
	frame,
	fps,
	items,
	onSelection,
	onCommit,
}: {
	definition: TimecodeDefinition;
	selection: Extract<TimecodeEditorSelection, { kind: "marker" }>;
	frame: number;
	fps: number;
	items: readonly TimelineItem[];
	onSelection(item: TimelineItem): void;
	onCommit(definition: TimecodeDefinition): void;
}) {
	const marker = definition.markers.find(
		(candidate) => candidate.id === selection.itemId,
	);
	const markerItems = items
		.filter((item) => item.kind === "marker")
		.sort((left, right) => left.frame - right.frame);
	const selectedIndex = markerItems.findIndex((item) =>
		sameSelection(item.selection, selection),
	);
	if (!marker) return null;
	const update = (patch: Partial<Marker>) =>
		onCommit({
			...definition,
			markers: definition.markers.map((candidate) =>
				candidate.id === marker.id ? { ...candidate, ...patch } : candidate,
			),
		});
	const move = (delta: number) => {
		if (markerItems.length < 2) return;
		const next =
			markerItems[
				(selectedIndex + delta + markerItems.length) % markerItems.length
			];
		if (next) onSelection(next);
	};
	const lastFrame = Math.max(0, definition.duration_frame ?? 0);
	const playheadFrame = Math.max(0, Math.min(lastFrame, Math.round(frame)));
	return (
		<div
			className="timecode-keyframe-actions timecode-marker-actions"
			role="group"
			aria-label="Selected marker actions"
		>
			<div className="timecode-keyframe-actions-title">
				<strong>{marker.name}</strong>
			</div>
			<StackedButton
				label="Previous Marker"
				top="Previous"
				bottom="Marker"
				disabled={markerItems.length < 2}
				onClick={() => move(-1)}
			/>
			<MarkerNameButton marker={marker} update={update} />
			<MarkerColorButton
				color={marker.color}
				onChange={(color) => update({ color })}
			/>
			<StackedButton
				label="Place Marker"
				top="Place"
				bottom="Marker"
				title={`Place ${marker.name} at ${formatFrame(playheadFrame, fps)}`}
				disabled={playheadFrame === marker.frame}
				onClick={() => {
					if (playheadFrame !== marker.frame) update({ frame: playheadFrame });
				}}
			/>
			<MarkerMoveToButton
				marker={marker}
				fps={fps}
				lastFrame={lastFrame}
				update={update}
			/>
			<StackedButton
				label="Next Marker"
				top="Next"
				bottom="Marker"
				className="timecode-keyframe-action timecode-next-keyframe"
				disabled={markerItems.length < 2}
				onClick={() => move(1)}
			/>
		</div>
	);
}
