import { useEffect, useMemo, useState } from "react";
import { Button, Input, NumberField, SelectField } from "@tosklight/ui";
import type { CueList } from "../../api/types";
import type { CueClipTimingDefaults } from "./cueClipTiming";
import type { CueEncoderContext } from "./timecodeEncoderSlots";
import type { TimecodeDefinition } from "../../api/types/timecode";
import {
	deleteTimelineItem,
	sameSelection,
	scaleCueListTimings,
	type TimecodeEditorSelection,
} from "./editorModel";
import { MarkerActionStrip } from "./TimecodeMarkerActions";
import {
	formatFrame,
	type TimecodeCueListOption,
	type TimelineItem,
	wrappedIndex,
} from "./timecodeEditorShared";
import { laneWithKeyframeValue } from "./timecodeEncoderSlots";
import { TIMECODE_EASINGS } from "./timecodeEditorShared";
import {
	SPEED_GROUP_MAX_BPM,
	SPEED_GROUP_MIN_BPM,
} from "../speedGroupRuntime/contracts";

function laneWithVolumeCurve(
	definition: TimecodeDefinition,
	laneId: string,
	keyframeId: string,
	curve: string,
): TimecodeDefinition {
	return {
		...definition,
		lanes: definition.lanes.map((candidate) =>
			candidate.id !== laneId || candidate.content.kind !== "audio_volume"
				? candidate
				: {
						...candidate,
						content: {
							...candidate.content,
							keyframes: candidate.content.keyframes.map((keyframe) =>
								keyframe.id === keyframeId
									? { ...keyframe, curve: curve as typeof keyframe.curve }
									: keyframe,
							),
						},
					},
		),
	};
}

/// The Cues a selected Cuelist clip covers, from its start Cue to its end Cue.
export function cueRangeOfSelectedClip(
	lane: TimecodeDefinition["lanes"][number] | undefined,
	selection: TimecodeEditorSelection | null,
	cueLists: readonly TimecodeCueListOption[],
): readonly { id?: string; number: string; name: string }[] {
	if (lane?.content.kind !== "cue_list" || selection?.kind !== "clip") return [];
	const content = lane.content;
	const clip = content.clips.find((item) => item.id === selection.itemId);
	const option = cueLists.find((item) => item.id === content.cue_list_id);
	if (!clip || !option) return [];
	const start = option.cues.findIndex((cue) => cue.id === clip.start_cue_id);
	const end = option.cues.findIndex((cue) => cue.id === clip.end_cue_id);
	if (start < 0 || end < start) return [];
	return option.cues.slice(start, end + 1);
}

export function KeyframeActionStrip({
	definition,
	selection,
	laneId,
	frame,
	fps,
	items,
	cueLists,
	selectedCueId,
	onSelectCue,
	onSelection,
	onInsert,
	onCommit,
	onAddClip,
}: {
	definition: TimecodeDefinition;
	selection: TimecodeEditorSelection | null;
	laneId: string | null;
	frame: number;
	fps: number;
	items: readonly TimelineItem[];
	cueLists: readonly TimecodeCueListOption[];
	selectedCueId: string | null;
	onSelectCue(cueId: string | null): void;
	onSelection(item: TimelineItem): void;
	onInsert(): void;
	onCommit(definition: TimecodeDefinition): void;
	onAddClip(): void;
}) {
	// A selected Marker is edited by its own strip, which owns colour and naming as well.
	if (selection?.kind === "marker")
		return (
			<MarkerActionStrip
				{...{ definition, selection, frame, fps, items, onSelection, onCommit }}
			/>
		);
	const { lane, laneItems, selectedIndex, selectedVolume, selectedSpeed } =
		selectedLaneKeyframes(definition, laneId, selection, items);
	const move = (delta: number) => {
		if (!laneItems.length) return;
		const index =
			selectedIndex < 0
				? delta < 0
					? laneItems.length - 1
					: 0
				: Math.max(0, Math.min(laneItems.length - 1, selectedIndex + delta));
		const item = laneItems[index];
		if (item) onSelection(item);
	};
	const updateEasing = (curve: string) => {
		if (!selectedVolume || selection?.kind !== "volume" || !lane) return;
		onCommit(laneWithVolumeCurve(definition, lane.id, selectedVolume.id, curve));
	};
	const updateValue = (value: number) => {
		if (!lane || !selection) return;
		onCommit(laneWithKeyframeValue(definition, lane.id, selection, value));
	};
	const { canInsert, canAddClip, canDelete } = stripCapabilities(
		lane,
		selection,
		cueLists,
	);
	// The Cues a selected Cuelist clip spans, in running order.
	const clipCues = cueRangeOfSelectedClip(lane, selection, cueLists);
	const stepCue = (delta: number) => {
		if (!clipCues.length) return;
		const current = clipCues.findIndex((cue) => cue.id === selectedCueId);
		const index =
			current < 0
				? delta < 0
					? clipCues.length - 1
					: 0
				: wrappedIndex(current + delta, clipCues.length);
		onSelectCue(clipCues[index]?.id ?? null);
	};
	return (
		<div
			className="timecode-keyframe-actions"
			role="group"
			aria-label="Selected lane and keyframe actions"
		>
			<div className="timecode-keyframe-actions-title">
				<strong>{lane?.name ?? "Select a lane"}</strong>
			</div>
			<KeyframeStepButton
				delta={-1}
				enabled={laneItems.length > 0}
				onStep={move}
			/>
			{canInsert && (
				<Button
					className="timecode-keyframe-action"
					aria-label="Insert Keyframe"
					size="compact"
					onClick={onInsert}
					title={`Insert keyframe at ${formatFrame(frame, fps)}`}
				>
					<span>
						Insert
						<br />
						Keyframe
					</span>
				</Button>
			)}
			<Button
				className="timecode-keyframe-action"
				aria-label="Delete Keyframe"
				size="compact"
				disabled={!canDelete}
				onClick={() => {
					if (selection) onCommit(deleteTimelineItem(definition, selection));
				}}
			>
				<span>
					Delete
					<br />
					Keyframe
				</span>
			</Button>
			{/* A Cuelist clip holds Cue sub-clips, so the strip steps through Cues the same way
			    it steps through clips. */}
			{lane?.content.kind === "cue_list" && (
				<CueStepButtons enabled={clipCues.length > 0} onStep={stepCue} />
			)}
			{lane?.content.kind === "cue_list" ||
			lane?.content.kind === "audio_player" ? (
				<Button size="compact" disabled={!canAddClip} onClick={onAddClip}>
					Add clip at {formatFrame(frame, fps)}
				</Button>
			) : null}
			<SelectedKeyframeValue
				speed={selectedSpeed}
				volume={selectedVolume}
				onValue={updateValue}
				onEasing={updateEasing}
			/>
			<KeyframeStepButton
				delta={1}
				enabled={laneItems.length > 0}
				onStep={move}
			/>
		</div>
	);
}

/// What the selected lane currently offers the strip: its items and whichever keyframe is chosen.
function selectedLaneKeyframes(
	definition: TimecodeDefinition,
	laneId: string | null,
	selection: TimecodeEditorSelection | null,
	items: readonly TimelineItem[],
) {
	const lane = definition.lanes.find((candidate) => candidate.id === laneId);
	const laneItems = items.filter((item) => item.laneId === laneId);
	const content = lane?.content;
	const chosen = (candidate: { id: string }) =>
		selection !== null && "itemId" in selection && candidate.id === selection.itemId;
	return {
		lane,
		laneItems,
		selectedIndex: laneItems.findIndex((item) =>
			sameSelection(item.selection, selection),
		),
		selectedVolume:
			selection?.kind === "volume" && content?.kind === "audio_volume"
				? content.keyframes.find(chosen)
				: undefined,
		selectedSpeed:
			selection?.kind === "speed" && content?.kind === "speed_group"
				? content.keyframes.find(chosen)
				: undefined,
	};
}

/// Which of the strip's actions the current lane and selection support.
function stripCapabilities(
	lane: TimecodeDefinition["lanes"][number] | undefined,
	selection: TimecodeEditorSelection | null,
	cueLists: readonly TimecodeCueListOption[],
) {
	const cueListId =
		lane?.content.kind === "cue_list" ? lane.content.cue_list_id : null;
	return {
		canInsert:
			lane?.content.kind === "audio_volume" ||
			lane?.content.kind === "speed_group",
		canAddClip:
			lane?.content.kind === "audio_player" ||
			(cueListId !== null &&
				Boolean(
					cueLists.find((candidate) => candidate.id === cueListId)?.cues.length,
				)),
		canDelete: selection?.kind === "volume" || selection?.kind === "speed",
	};
}

/// Steps the selection to the previous or next keyframe of the selected lane.
function KeyframeStepButton({
	delta,
	enabled,
	onStep,
}: {
	delta: -1 | 1;
	enabled: boolean;
	onStep(delta: number): void;
}) {
	return (
		<Button
			className={`timecode-keyframe-action${delta > 0 ? " timecode-next-keyframe" : ""}`}
			aria-label={delta < 0 ? "Prev Keyframe" : "Next Keyframe"}
			size="compact"
			disabled={!enabled}
			onClick={() => onStep(delta)}
		>
			<span>
				{delta < 0 ? "Prev" : "Next"}
				<br />
				Keyframe
			</span>
		</Button>
	);
}

/// Steps through the Cues of the selected clip, as the strip steps through clips.
function CueStepButtons({
	enabled,
	onStep,
}: {
	enabled: boolean;
	onStep(delta: number): void;
}) {
	return (
		<>
			{([-1, 1] as const).map((delta) => (
				<Button
					key={delta}
					className="timecode-keyframe-action"
					aria-label={delta < 0 ? "Prev Cue" : "Next Cue"}
					size="compact"
					disabled={!enabled}
					onClick={() => onStep(delta)}
				>
					<span>
						{delta < 0 ? "Prev" : "Next"}
						<br />
						Cue
					</span>
				</Button>
			))}
		</>
	);
}

/// The editors for whichever keyframe kind is selected, or nothing when none is.
function SelectedKeyframeValue({
	speed,
	volume,
	onValue,
	onEasing,
}: {
	speed?: { bpm: number };
	volume?: { value: number; curve: string };
	onValue(value: number): void;
	onEasing(curve: string): void;
}) {
	if (speed)
		return (
			<KeyframeValueNumber
				label="BPM"
				value={speed.bpm}
				minimum={SPEED_GROUP_MIN_BPM}
				maximum={SPEED_GROUP_MAX_BPM}
				step={0.1}
				unit=" BPM"
				onChange={onValue}
			/>
		);
	if (!volume) return null;
	return (
		<>
			<KeyframeValueSlider
				label="Volume"
				value={Math.round(volume.value * 100)}
				minimum={0}
				maximum={100}
				unit="%"
				onChange={onValue}
			/>
			<SelectField
				label="Easing"
				value={volume.curve}
				onChange={onEasing}
				options={TIMECODE_EASINGS}
			/>
		</>
	);
}

function KeyframeValueNumber({
	label,
	value,
	minimum,
	maximum,
	step = 1,
	unit,
	onChange,
}: {
	label: string;
	value: number;
	minimum: number;
	maximum: number;
	step?: number;
	unit: string;
	onChange(value: number): void;
}) {
	return (
		<NumberField
			className="timecode-keyframe-value-number"
			label={label}
			aria-label={`${label} value`}
			keyboardLabel={label}
			value={value}
			min={minimum}
			max={maximum}
			step={step}
			allowDecimal={step < 1}
			unit={unit}
			onChange={(event) => {
				const next = Number(event.currentTarget.value);
				if (Number.isFinite(next)) onChange(next);
			}}
		/>
	);
}

function KeyframeValueSlider({
	label,
	value,
	minimum,
	maximum,
	step = 1,
	unit,
	onChange,
}: {
	label: string;
	value: number;
	minimum: number;
	maximum: number;
	step?: number;
	unit: string;
	onChange(value: number): void;
}) {
	return (
		<label className="timecode-keyframe-value-control">
			<span>{label}</span>
			<Input
				type="range"
				aria-label={`${label} value`}
				min={minimum}
				max={maximum}
				step={step}
				value={value}
				onChange={(event) => onChange(Number(event.currentTarget.value))}
			/>
			<output>{`${Number.isInteger(value) ? value : value.toFixed(1)}${unit}`}</output>
		</label>
	);
}


/**
 * The encoder context for the Cue inside a selected Cuelist clip, or nothing when there is none.
 *
 * The deck can only address a Cue when the clip's Cuelist is loaded and writable, so a clip whose
 * Cuelist the editor cannot save leaves the keyframe slots in place.
 */
export function useCueEncoderContext({
	activeLane,
	selection,
	cueLists,
	selectedCueId,
	setSelectedCueId,
	timingDefaults,
	onSaveCueList,
}: {
	activeLane?: TimecodeDefinition["lanes"][number];
	selection: TimecodeEditorSelection | null;
	cueLists: readonly TimecodeCueListOption[];
	selectedCueId: string | null;
	setSelectedCueId(cueId: string | null): void;
	timingDefaults?: CueClipTimingDefaults;
	onSaveCueList?(cueListId: string, body: CueList): Promise<CueList>;
}): CueEncoderContext | undefined {
	return useMemo(() => {
		const clipCues = cueRangeOfSelectedClip(activeLane, selection, cueLists);
		if (!clipCues.length || activeLane?.content.kind !== "cue_list")
			return undefined;
		const content = activeLane.content;
		const option = cueLists.find((item) => item.id === content.cue_list_id);
		if (!option?.body || !option.objectId || !onSaveCueList || !timingDefaults)
			return undefined;
		return {
			cues: clipCues,
			selectedCueId,
			cueListId: option.objectId,
			cueList: option.body,
			timingDefaults,
			setSelectedCueId,
			saveCueList: onSaveCueList,
		};
	}, [
		activeLane,
		cueLists,
		onSaveCueList,
		selectedCueId,
		setSelectedCueId,
		selection,
		timingDefaults,
	]);
}

/**
 * Scales every Cue timing a resized clip drives, in the Cuelist that owns them.
 *
 * The clip lives in the Timecode and the timings live in the Cuelist, so stretching the clip has
 * to be written back through Cuelist authority rather than into the timeline.
 */
export function scaleClipCueTimings({
	definition,
	cueLists,
	selection,
	ratio,
	onSaveCueList,
	onCueTimingError,
}: {
	definition: TimecodeDefinition;
	cueLists: readonly TimecodeCueListOption[];
	selection: TimecodeEditorSelection & { kind: "clip" };
	ratio: number;
	onSaveCueList?(cueListId: string, body: CueList): Promise<CueList>;
	onCueTimingError?(message: string): void;
}): void {
	const lane = definition.lanes.find((item) => item.id === selection.laneId);
	if (lane?.content.kind !== "cue_list") return;
	const content = lane.content;
	const clip = content.clips.find((item) => item.id === selection.itemId);
	const option = cueLists.find((item) => item.id === content.cue_list_id);
	if (!clip || !option?.body || !onSaveCueList) return;
	const scaled = scaleCueListTimings(
		option.body,
		clip.start_cue_id,
		clip.end_cue_id,
		ratio,
	);
	if (scaled === option.body) return;
	void onSaveCueList(option.id, scaled).catch((reason: unknown) =>
		onCueTimingError?.(`Could not scale the Cue timings: ${String(reason)}`),
	);
}

/**
 * The Cue selected inside the selected Cuelist clip.
 *
 * A Cue belongs to one clip, so selecting a different clip clears it rather than carrying a Cue
 * that the new clip may not even contain.
 */
export function useSelectedCue(
	selection: TimecodeEditorSelection | null,
): [string | null, (cueId: string | null) => void] {
	const [selectedCueId, setSelectedCueId] = useState<string | null>(null);
	const clipKey =
		selection?.kind === "clip" ? `${selection.laneId}:${selection.itemId}` : null;
	// biome-ignore lint/correctness/useExhaustiveDependencies: the Cue is cleared by the clip
	// identity, not by the selection object that happens to carry it.
	useEffect(() => {
		setSelectedCueId(null);
	}, [clipKey]);
	return [selectedCueId, setSelectedCueId];
}
