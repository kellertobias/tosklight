import { useState } from "react";
import {
	Button,
	CheckboxField,
	NumberField,
	SelectField,
	ModalLayer,
	ModalTitleBar,
} from "@tosklight/ui";
import type { TimecodeDefinition } from "../../api/types/timecode";
import {
	copyTimelineItem,
	deleteTimelineItem,
	sameSelection,
	type TimecodeEditorSelection,
} from "./editorModel";
import {
	type TimecodeCueListOption,
	type TimelineItem,
} from "./timecodeEditorShared";

/// Additional properties of the selected item, alongside its transport and value controls.

interface SelectionInspectorProps {
	definition: TimecodeDefinition;
	selection: TimecodeEditorSelection | null;
	selectedLabel?: string;
	cueLists: readonly TimecodeCueListOption[];
	fps: number;
	onCommit(definition: TimecodeDefinition): void;
}

export function SelectionProperties(
	props: SelectionInspectorProps & {
		items: readonly TimelineItem[];
		onSelect(selection: TimecodeEditorSelection | null): void;
	},
) {
	const { definition, selection, cueLists, fps, items, onCommit, onSelect } =
		props;
	if (
		!selection ||
		!items.some((item) => sameSelection(item.selection, selection))
	)
		return null;
	const activeLane =
		"laneId" in selection
			? definition.lanes.find((lane) => lane.id === selection.laneId)
			: undefined;
	const canCopySelection = (() => {
		if (
			selection?.kind !== "clip" ||
			(activeLane?.content.kind !== "cue_list" &&
				activeLane?.content.kind !== "audio_player")
		)
			return true;
		const clips = [...activeLane.content.clips].sort(
			(left, right) => left.start_frame - right.start_frame,
		);
		const selected = clips.find((clip) => clip.id === selection.itemId);
		if (!selected) return false;
		const length = selected.end_frame - selected.start_frame;
		let end = 0;
		for (const clip of clips) {
			if (clip.start_frame - end >= length) return true;
			end = Math.max(end, clip.end_frame);
		}
		return (definition.duration_frame ?? Infinity) - end >= length;
	})();
	return (
		<>
			{selection.kind === "clip" ? (
				<ClipSettingsButton key={selection.itemId} {...props} />
			) : selection.kind !== "marker" ? (
				<SelectionInspector {...props} />
			) : null}

			<Button
				size="compact"
				disabled={!canCopySelection}
				title={
					canCopySelection
						? "Copy selected item"
						: "No free space for a copy of this clip"
				}
				onClick={() => {
					const copied = copyTimelineItem(
						definition,
						selection,
						crypto.randomUUID(),
						fps,
					);
					onCommit(copied.definition);
					onSelect(copied.selection);
				}}
			>
				Copy{" "}
				{selection.kind === "clip"
					? "clip"
					: selection.kind === "marker"
						? "marker"
						: "keyframe"}
			</Button>
			{(selection.kind === "clip" || selection.kind === "marker") && (
				<Button
					size="compact"
					onClick={() => {
						onCommit(deleteTimelineItem(definition, selection));
						onSelect(null);
					}}
				>
					Delete {selection.kind}
				</Button>
			)}
		</>
	);
}

function ClipSettingsButton(props: SelectionInspectorProps) {
	const [open, setOpen] = useState(false);
	return (
		<>
			<Button size="compact" onClick={() => setOpen(true)}>
				Clip settings
			</Button>
			{open && (
				<ModalLayer
					ariaLabel="Clip settings"
					dialogClassName="timecode-selection-settings-dialog"
					onClose={() => setOpen(false)}
				>
					<ModalTitleBar
						title="Clip settings"
						onClose={() => setOpen(false)}
						closeLabel="Close clip settings"
					/>
					<SelectedClipInspector {...props} />
				</ModalLayer>
			)}
		</>
	);
}

function SelectionInspector({
	definition,
	selection,
	selectedLabel,
	cueLists,
	fps,
	onCommit,
}: SelectionInspectorProps) {
	if (!selection)
		return (
			<div className="timecode-selection-inspector">
				<span>
					Select a clip, keyframe, or marker to inspect, copy, move, or delete
					it.
				</span>
			</div>
		);
	if (selection.kind === "marker") return null;
	const lane = definition.lanes.find(
		(candidate) => candidate.id === selection.laneId,
	);
	if (!lane) return null;
	if (selection.kind === "speed" && lane.content.kind === "speed_group") {
		const content = lane.content;
		const keyframe = content.keyframes.find(
			(candidate) => candidate.id === selection.itemId,
		);
		if (!keyframe) return null;
		return (
			<SpeedInspector
				phase={keyframe.phase}
				onPhase={(phase) =>
					onCommit(
						updateLane(definition, lane.id, {
							...content,
							keyframes: content.keyframes.map((candidate) =>
								candidate.id === keyframe.id
									? { ...candidate, phase }
									: candidate,
							),
						}),
					)
				}
			/>
		);
	}
	if (selection.kind === "volume" && lane.content.kind === "audio_volume") {
		const content = lane.content;
		const keyframe = content.keyframes.find(
			(candidate) => candidate.id === selection.itemId,
		);
		if (!keyframe) return null;
		const update = (patch: Partial<typeof keyframe>) =>
			onCommit(
				updateLane(definition, lane.id, {
					...content,
					keyframes: content.keyframes.map((candidate) =>
						candidate.id === keyframe.id
							? { ...candidate, ...patch }
							: candidate,
					),
				}),
			);
		return <VolumeInspector keyframe={keyframe} update={update} />;
	}
	return (
		<SelectedClipInspector
			{...{ definition, selection, selectedLabel, cueLists, fps, onCommit }}
		/>
	);
}

function SelectedClipInspector({
	definition,
	selection,
	selectedLabel,
	cueLists,
	onCommit,
}: SelectionInspectorProps) {
	if (!selection || selection.kind !== "clip") return null;
	const lane = definition.lanes.find(
		(candidate) => candidate.id === selection.laneId,
	);
	if (!lane) return null;
	if (selection.kind === "clip" && lane.content.kind === "cue_list") {
		const content = lane.content;
		const clip = content.clips.find(
			(candidate) => candidate.id === selection.itemId,
		);
		if (!clip) return null;
		const cues =
			cueLists
				.find((candidate) => candidate.id === content.cue_list_id)
				?.cues.flatMap((cue) => (cue.id ? [{ ...cue, id: cue.id }] : [])) ?? [];
		const update = (patch: Partial<typeof clip>) =>
			onCommit(
				updateLane(definition, lane.id, {
					...content,
					clips: content.clips.map((candidate) =>
						candidate.id === clip.id
							? (() => {
									const next = { ...candidate, ...patch };
									const length = next.end_frame - next.start_frame;
									return {
										...next,
										in_fade_frames: Math.min(next.in_fade_frames, length),
										out_fade_frames: Math.min(next.out_fade_frames, length),
										cue_starts: next.cue_starts.map((start) => ({
											...start,
											offset_frame: Math.min(start.offset_frame, length),
										})),
									};
								})()
							: candidate,
					),
				}),
			);
		return (
			<ClipInspector
				label={selectedLabel}
				clip={clip}
				cues={cues}
				duration={Math.min(
					definition.duration_frame ?? Infinity,
					...content.clips
						.filter(
							(other) =>
								other.id !== clip.id && other.start_frame >= clip.end_frame,
						)
						.map((other) => other.start_frame),
				)}
				minimumStart={Math.max(
					0,
					...content.clips
						.filter(
							(other) =>
								other.id !== clip.id && other.end_frame <= clip.start_frame,
						)
						.map((other) => other.end_frame),
				)}
				update={update}
			/>
		);
	}
	if (selection.kind === "clip" && lane.content.kind === "audio_player") {
		const content = lane.content;
		const clip = content.clips.find(
			(candidate) => candidate.id === selection.itemId,
		);
		if (!clip) return null;
		const update = (patch: Partial<typeof clip>) =>
			onCommit(
				updateLane(definition, lane.id, {
					...content,
					clips: content.clips.map((candidate) =>
						candidate.id === clip.id ? { ...candidate, ...patch } : candidate,
					),
				}),
			);
		return (
			<AudioPlayerClipInspector
				label={selectedLabel}
				clip={clip}
				duration={Math.min(
					definition.duration_frame ?? Infinity,
					...content.clips
						.filter(
							(other) =>
								other.id !== clip.id && other.start_frame >= clip.end_frame,
						)
						.map((other) => other.start_frame),
				)}
				minimumStart={Math.max(
					0,
					...content.clips
						.filter(
							(other) =>
								other.id !== clip.id && other.end_frame <= clip.start_frame,
						)
						.map((other) => other.end_frame),
				)}
				update={update}
			/>
		);
	}
	return null;
}

type AudioPlayerClip = Extract<
	TimecodeDefinition["lanes"][number]["content"],
	{ kind: "audio_player" }
>["clips"][number];

function AudioPlayerClipInspector({
	label,
	clip,
	duration,
	minimumStart,
	update,
}: {
	label?: string;
	clip: AudioPlayerClip;
	duration?: number | null;
	minimumStart: number;
	update(patch: Partial<AudioPlayerClip>): void;
}) {
	return (
		<div className="timecode-selection-inspector">
			<strong>{label}</strong>
			<InspectorNumber
				label="Start frame"
				value={clip.start_frame}
				min={minimumStart}
				max={clip.end_frame - 1}
				onValue={(start_frame) => {
					const offset = start_frame - clip.start_frame;
					update({
						start_frame,
						volume_keyframes: clip.volume_keyframes.map((keyframe) => ({
							...keyframe,
							frame: Math.min(
								clip.end_frame - 1,
								Math.max(start_frame, keyframe.frame + offset),
							),
						})),
					});
				}}
			/>
			<InspectorNumber
				label="End frame"
				value={clip.end_frame}
				min={clip.start_frame + 1}
				max={duration ?? undefined}
				onValue={(end_frame) =>
					update({
						end_frame,
						volume_keyframes: clip.volume_keyframes.map((keyframe) => ({
							...keyframe,
							frame: Math.min(keyframe.frame, end_frame - 1),
						})),
					})
				}
			/>
			<InspectorNumber
				label="Audio Folder"
				value={clip.folder}
				min={0}
				max={255}
				onValue={(folder) => update({ folder })}
			/>
			<InspectorNumber
				label="Audio File"
				value={clip.file}
				min={0}
				max={255}
				onValue={(file) => update({ file })}
			/>
			<CheckboxField
				label="Repeat"
				stateLabel="Repeat clip"
				checked={clip.repeat}
				onChange={(event) => update({ repeat: event.currentTarget.checked })}
			/>
			<AudioPlayerVolumePoints clip={clip} update={update} />
		</div>
	);
}

function AudioPlayerVolumePoints({
	clip,
	update,
}: {
	clip: AudioPlayerClip;
	update(patch: Partial<AudioPlayerClip>): void;
}) {
	const updateVolume = (
		id: string,
		patch: Partial<AudioPlayerClip["volume_keyframes"][number]>,
	) =>
		update({
			volume_keyframes: clip.volume_keyframes
				.map((keyframe) =>
					keyframe.id === id ? { ...keyframe, ...patch } : keyframe,
				)
				.sort((left, right) => left.frame - right.frame),
		});
	const addVolumePoint = () => {
		const previous = clip.volume_keyframes.at(-1);
		const frame = Math.min(
			clip.end_frame - 1,
			Math.max(
				clip.start_frame,
				previous
					? previous.frame + 1
					: Math.round((clip.start_frame + clip.end_frame) / 2),
			),
		);
		update({
			volume_keyframes: [
				...clip.volume_keyframes,
				{
					id: crypto.randomUUID(),
					frame,
					value: previous?.value ?? 1,
					fade_frames: 0,
					curve: "linear" as const,
				},
			].sort((left, right) => left.frame - right.frame),
		});
	};
	return (
		<>
			{clip.volume_keyframes.map((keyframe, index) => (
				<div className="timecode-audio-player-volume-point" key={keyframe.id}>
					<strong>Volume point {index + 1}</strong>
					<InspectorNumber
						label="Volume frame"
						value={keyframe.frame}
						min={clip.start_frame}
						max={clip.end_frame - 1}
						onValue={(frame) => updateVolume(keyframe.id, { frame })}
					/>
					<InspectorNumber
						label="Volume %"
						value={Math.round(keyframe.value * 100)}
						min={0}
						max={100}
						onValue={(value) =>
							updateVolume(keyframe.id, { value: value / 100 })
						}
					/>
					<InspectorNumber
						label="Fade frames"
						value={keyframe.fade_frames}
						min={0}
						onValue={(fade_frames) =>
							updateVolume(keyframe.id, { fade_frames })
						}
					/>
					<SelectField
						label="Volume curve"
						value={keyframe.curve}
						onChange={(curve) => updateVolume(keyframe.id, { curve })}
						options={[
							{ value: "linear", label: "Linear" },
							{ value: "ease_in", label: "Ease in" },
							{ value: "ease_out", label: "Ease out" },
							{ value: "ease_in_out", label: "Ease in/out" },
						]}
					/>
					<Button
						size="compact"
						disabled={clip.volume_keyframes.length === 1}
						onClick={() =>
							update({
								volume_keyframes: clip.volume_keyframes.filter(
									(candidate) => candidate.id !== keyframe.id,
								),
							})
						}
					>
						Remove volume point
					</Button>
				</div>
			))}
			<Button size="compact" onClick={addVolumePoint}>
				Add volume point
			</Button>
		</>
	);
}

function SpeedInspector({
	phase,
	onPhase,
}: {
	phase: number;
	onPhase(value: number): void;
}) {
	return (
		<InspectorNumber
			label="Phase"
			value={phase}
			min={0}
			max={0.99}
			step={0.01}
			onValue={onPhase}
		/>
	);
}

type VolumeKeyframe = Extract<
	TimecodeDefinition["lanes"][number]["content"],
	{ kind: "audio_volume" }
>["keyframes"][number];
function VolumeInspector({
	keyframe,
	update,
}: {
	keyframe: VolumeKeyframe;
	update(patch: Partial<VolumeKeyframe>): void;
}) {
	return (
		<InspectorNumber
			label="Fade frames"
			value={keyframe.fade_frames}
			min={0}
			onValue={(fade_frames) => update({ fade_frames })}
		/>
	);
}

type CueClip = Extract<
	TimecodeDefinition["lanes"][number]["content"],
	{ kind: "cue_list" }
>["clips"][number];
function ClipInspector({
	label,
	clip,
	cues,
	duration,
	minimumStart,
	update,
}: {
	label?: string;
	clip: CueClip;
	cues: readonly { id: string; number: string; name: string }[];
	duration?: number | null;
	minimumStart: number;
	update(patch: Partial<CueClip>): void;
}) {
	// Offering only Cues that keep the clip order valid removes the "end Cue is
	// before its start Cue" error instead of reporting it after the fact.
	const startIndex = Math.max(
		0,
		cues.findIndex((cue) => cue.id === clip.start_cue_id),
	);
	const endIndex =
		cues.findIndex((cue) => cue.id === clip.end_cue_id) < 0
			? cues.length - 1
			: cues.findIndex((cue) => cue.id === clip.end_cue_id);
	return (
		<div className="timecode-selection-inspector">
			<strong>{label}</strong>
			<InspectorNumber
				label="Start frame"
				value={clip.start_frame}
				min={minimumStart}
				max={clip.end_frame - 1}
				onValue={(start_frame) => update({ start_frame })}
			/>
			<InspectorNumber
				label="End frame"
				value={clip.end_frame}
				min={clip.start_frame + 1}
				max={duration ?? undefined}
				onValue={(end_frame) => update({ end_frame })}
			/>
			<CueSelect
				label="Start Cue"
				value={clip.start_cue_id}
				cues={cues.slice(0, endIndex + 1)}
				onValue={(start_cue_id) => update({ start_cue_id })}
			/>
			<CueSelect
				label="End Cue"
				value={clip.end_cue_id}
				cues={cues.slice(startIndex)}
				onValue={(end_cue_id) => update({ end_cue_id })}
			/>
			<SelectField
				label="Start behavior"
				ariaLabel="Start behavior"
				value={clip.start_behavior}
				onChange={(start_behavior) => update({ start_behavior })}
				options={[
					{ value: "state", label: "State Start" },
					{ value: "cue", label: "Cue Start" },
				]}
			/>
			<SelectField
				label="End behavior"
				ariaLabel="End behavior"
				value={clip.end_behavior}
				onChange={(end_behavior) => update({ end_behavior })}
				options={[
					{ value: "release", label: "Release" },
					{ value: "hold", label: "Hold" },
				]}
			/>
		</div>
	);
}

function InspectorNumber({
	label,
	value,
	min,
	max,
	step,
	onValue,
}: {
	label: string;
	value: number;
	min?: number;
	max?: number;
	step?: number;
	onValue(value: number): void;
}) {
	return (
		<NumberField
			label={label}
			value={value}
			min={min}
			max={Number.isFinite(max) ? max : undefined}
			step={step}
			allowDecimal={Boolean(step && step < 1)}
			onChange={(event) => {
				const text = event.currentTarget.value.trim();
				const next = Number(text);
				if (!text || !Number.isFinite(next)) return;
				const rounded = step && step < 1 ? next : Math.round(next);
				onValue(Math.max(min ?? -Infinity, Math.min(max ?? Infinity, rounded)));
			}}
		/>
	);
}

function CueSelect({
	label,
	value,
	cues,
	onValue,
}: {
	label: string;
	value: string;
	cues: readonly { id: string; number: string; name: string }[];
	onValue(value: string): void;
}) {
	return (
		<SelectField
			ariaLabel={label}
			label={label}
			value={value}
			onChange={onValue}
			options={cues.map((cue) => ({
				value: cue.id,
				label: `${cue.number} · ${cue.name}`,
			}))}
		/>
	);
}

function updateLane(
	definition: TimecodeDefinition,
	laneId: string,
	content: TimecodeDefinition["lanes"][number]["content"],
): TimecodeDefinition {
	return {
		...definition,
		lanes: definition.lanes.map((lane) =>
			lane.id === laneId ? { ...lane, content } : lane,
		),
	};
}
