import type { CueList } from "../../api/types";
import type {
	TimecodeCueListClip,
	TimecodeDefinition,
	TimecodeMarker,
} from "../../api/types/timecode";

export type TimecodeEditorSelection =
	| { kind: "marker"; itemId: string }
	| { kind: "clip" | "speed" | "volume"; laneId: string; itemId: string };

export interface TimelineItem {
	selection: TimecodeEditorSelection;
	frame: number;
	endFrame?: number;
	label: string;
	laneId?: string;
	laneName?: string;
	kind: TimecodeEditorSelection["kind"];
	color?: string;
	valueLabel?: string;
}

export function timelineItems(definition: TimecodeDefinition): TimelineItem[] {
	const items: TimelineItem[] = definition.markers.map((marker) => ({
		selection: { kind: "marker", itemId: marker.id },
		frame: marker.frame,
		label: marker.name,
		kind: "marker",
		color: marker.color ?? "#ffffff",
	}));
	for (const lane of definition.lanes) {
		switch (lane.content.kind) {
			case "cue_list":
				for (const clip of lane.content.clips) {
					items.push({
						selection: { kind: "clip", laneId: lane.id, itemId: clip.id },
						frame: clip.start_frame,
						endFrame: clip.end_frame,
						label: `${lane.name} · ${clip.start_behavior} start → ${clip.end_behavior}`,
						laneId: lane.id,
						laneName: lane.name,
						kind: "clip",
					});
				}
				break;
			case "speed_group":
				for (const keyframe of lane.content.keyframes) {
					items.push({
						selection: { kind: "speed", laneId: lane.id, itemId: keyframe.id },
						frame: keyframe.frame,
						label: `${lane.name} · ${keyframe.bpm} BPM · phase ${keyframe.phase}`,
						laneId: lane.id,
						laneName: lane.name,
						kind: "speed",
						valueLabel: `${keyframe.bpm} BPM`,
					});
				}
				break;
			case "audio_volume":
				for (const keyframe of lane.content.keyframes) {
					items.push({
						selection: { kind: "volume", laneId: lane.id, itemId: keyframe.id },
						frame: keyframe.frame,
						label: `${lane.name} · ${Math.round(keyframe.value * 100)}% · ${keyframe.curve}`,
						laneId: lane.id,
						laneName: lane.name,
						kind: "volume",
						valueLabel: `${Math.round(keyframe.value * 100)}%`,
					});
				}
				break;
			case "audio_player":
				for (const clip of lane.content.clips) {
					items.push({
						selection: { kind: "clip", laneId: lane.id, itemId: clip.id },
						frame: clip.start_frame,
						endFrame: clip.end_frame,
						label: `${lane.name} · ${padAddress(clip.folder)}.${padAddress(clip.file)}${clip.repeat ? " · repeat" : ""}`,
						laneId: lane.id,
						laneName: lane.name,
						kind: "clip",
					});
				}
				break;
		}
	}
	return items.sort((left, right) => left.frame - right.frame);
}

export function reconcileAutomaticAudioLane(
	definition: TimecodeDefinition,
	createId: () => string = () => crypto.randomUUID(),
): TimecodeDefinition {
	const audioLanes = definition.lanes.filter(
		(lane) => lane.content.kind === "audio_volume",
	);
	if (!definition.audio) {
		if (!audioLanes.length) return definition;
		return {
			...definition,
			lanes: definition.lanes.filter(
				(lane) => lane.content.kind !== "audio_volume",
			),
		};
	}
	if (audioLanes.length === 1) return definition;
	if (audioLanes.length > 1) {
		const keep = audioLanes[0]?.id;
		return {
			...definition,
			lanes: definition.lanes.filter(
				(lane) => lane.content.kind !== "audio_volume" || lane.id === keep,
			),
		};
	}
	return {
		...definition,
		lanes: [
			{
				id: createId(),
				name: "Audio",
				content: {
					kind: "audio_volume",
					keyframes: [
						{
							id: createId(),
							frame: 0,
							value: 1,
							fade_frames: 0,
							curve: "linear",
						},
					],
				},
			},
			...definition.lanes,
		],
	};
}

export function sameSelection(
	left: TimecodeEditorSelection | null,
	right: TimecodeEditorSelection | null,
): boolean {
	if (
		!left ||
		!right ||
		left.kind !== right.kind ||
		left.itemId !== right.itemId
	)
		return false;
	return (
		("laneId" in left ? left.laneId : null) ===
		("laneId" in right ? right.laneId : null)
	);
}

export function reorderTimelineLane(
	definition: TimecodeDefinition,
	laneId: string,
	targetLaneId: string,
): TimecodeDefinition {
	const sourceIndex = definition.lanes.findIndex((lane) => lane.id === laneId);
	const targetIndex = definition.lanes.findIndex(
		(lane) => lane.id === targetLaneId,
	);
	if (sourceIndex < 0 || targetIndex < 0 || sourceIndex === targetIndex)
		return definition;
	const lanes = [...definition.lanes];
	const [lane] = lanes.splice(sourceIndex, 1);
	if (!lane) return definition;
	lanes.splice(targetIndex, 0, lane);
	return { ...definition, lanes };
}

export function moveTimelineItem(
	definition: TimecodeDefinition,
	selection: TimecodeEditorSelection,
	frame: number,
): TimecodeDefinition {
	const duration = definition.duration_frame ?? Number.MAX_SAFE_INTEGER;
	const nextFrame = clampFrame(frame, duration);
	if (selection.kind === "marker") {
		return {
			...definition,
			markers: definition.markers.map((marker) =>
				marker.id === selection.itemId
					? { ...marker, frame: nextFrame }
					: marker,
			),
		};
	}
	return {
		...definition,
		lanes: definition.lanes.map((lane) => {
			if (lane.id !== selection.laneId) return lane;
			switch (lane.content.kind) {
				case "cue_list":
					if (selection.kind !== "clip") return lane;
					{
						const selected = lane.content.clips.find(
							(clip) => clip.id === selection.itemId,
						);
						if (!selected) return lane;
						const length = selected.end_frame - selected.start_frame;
						const start = closestNonOverlappingStart(
							nextFrame,
							length,
							lane.content.clips.filter((clip) => clip.id !== selected.id),
							duration,
						);
						return {
							...lane,
							content: {
								...lane.content,
								clips: lane.content.clips
									.map((clip) =>
										clip.id === selection.itemId
											? {
													...clip,
													start_frame: start,
													end_frame: start + length,
												}
											: clip,
									)
									.sort(byStartFrame),
							},
						};
					}
				case "speed_group":
					if (selection.kind !== "speed") return lane;
					return {
						...lane,
						content: {
							...lane.content,
							keyframes: lane.content.keyframes
								.map((keyframe) =>
									keyframe.id === selection.itemId
										? { ...keyframe, frame: nextFrame }
										: keyframe,
								)
								.sort(byFrame),
						},
					};
				case "audio_volume":
					if (selection.kind !== "volume") return lane;
					return {
						...lane,
						content: {
							...lane.content,
							keyframes: lane.content.keyframes
								.map((keyframe) =>
									keyframe.id === selection.itemId
										? { ...keyframe, frame: nextFrame }
										: keyframe,
								)
								.sort(byFrame),
						},
					};
				case "audio_player":
					if (selection.kind !== "clip") return lane;
					{
						const selected = lane.content.clips.find(
							(clip) => clip.id === selection.itemId,
						);
						if (!selected) return lane;
						const length = selected.end_frame - selected.start_frame;
						const start = closestNonOverlappingStart(
							nextFrame,
							length,
							lane.content.clips.filter((clip) => clip.id !== selected.id),
							duration,
						);
						return {
							...lane,
							content: {
								...lane.content,
								clips: lane.content.clips
									.map((clip) => {
										if (clip.id !== selection.itemId) return clip;
										const offset = start - clip.start_frame;
										return {
											...clip,
											start_frame: start,
											end_frame: start + length,
											volume_keyframes: clip.volume_keyframes.map(
												(keyframe) => ({
													...keyframe,
													frame: keyframe.frame + offset,
												}),
											),
										};
									})
									.sort(byStartFrame),
							},
						};
					}
			}
			return lane;
		}),
	};
}

export function resizeTimelineClip(
	definition: TimecodeDefinition,
	selection: TimecodeEditorSelection & { kind: "clip" },
	edge: "start" | "end",
	frame: number,
): TimecodeDefinition {
	const duration = definition.duration_frame ?? Number.MAX_SAFE_INTEGER;
	const nextFrame = clampFrame(frame, duration);
	return {
		...definition,
		lanes: definition.lanes.map((lane) => {
			if (lane.id !== selection.laneId) return lane;
			if (lane.content.kind === "cue_list") {
				const bounds = resizeBounds(
					lane.content.clips,
					selection.itemId,
					edge,
					nextFrame,
				);
				return {
					...lane,
					content: {
						...lane.content,
						clips: lane.content.clips
							.map((clip) =>
								clip.id !== selection.itemId
									? clip
									: scaledCueListClip(clip, bounds),
							)
							.sort(byStartFrame),
					},
				};
			}
			if (lane.content.kind === "audio_player") {
				const bounds = resizeBounds(
					lane.content.clips,
					selection.itemId,
					edge,
					nextFrame,
				);
				return {
					...lane,
					content: {
						...lane.content,
						clips: lane.content.clips
							.map((clip) =>
								clip.id !== selection.itemId
									? clip
									: edge === "start"
										? {
												...clip,
												start_frame: bounds.start,
												volume_keyframes: clip.volume_keyframes.filter(
													(keyframe) => keyframe.frame >= bounds.start,
												),
											}
										: {
												...clip,
												end_frame: bounds.end,
												volume_keyframes: clip.volume_keyframes.filter(
													(keyframe) => keyframe.frame <= bounds.end,
												),
											},
							)
							.sort(byStartFrame),
					},
				};
			}
			return lane;
		}),
	};
}

/// Scales a Cuelist clip and everything it positions inside itself.
///
/// Dragging an edge of the clip handle stretches or compresses the whole thing, so a placed
/// transition keeps its position relative to the clip rather than staying at a fixed frame and
/// drifting out of the clip it belongs to.
function scaledCueListClip(
	clip: TimecodeCueListClip,
	bounds: { start: number; end: number },
): TimecodeCueListClip {
	const previousLength = Math.max(1, clip.end_frame - clip.start_frame);
	const nextLength = Math.max(1, bounds.end - bounds.start);
	const ratio = nextLength / previousLength;
	return {
		...clip,
		start_frame: bounds.start,
		end_frame: bounds.end,
		cue_starts: clip.cue_starts.map((start) => ({
			...start,
			offset_frame: Math.max(
				0,
				Math.min(nextLength, Math.round(start.offset_frame * ratio)),
			),
		})),
	};
}

/// The factor an edge drag applies to every timing inside a Cuelist clip.
export function cueListClipScale(
	clip: { start_frame: number; end_frame: number },
	nextStartFrame: number,
	nextEndFrame: number,
): number {
	const previousLength = Math.max(1, clip.end_frame - clip.start_frame);
	const nextLength = Math.max(1, nextEndFrame - nextStartFrame);
	return nextLength / previousLength;
}

/// Applies a clip scale to every Cue timing the clip drives.
///
/// A clip that is stretched to twice its length must play the same way over twice the time, so
/// each delay and fade in its Cue range scales with it. Timings the Cue inherits from desk
/// defaults are left inherited rather than being written out at a scaled value.
export function scaleCueListTimings(
	cueList: CueList,
	startCueId: string,
	endCueId: string,
	ratio: number,
): CueList {
	if (!Number.isFinite(ratio) || ratio <= 0 || ratio === 1) return cueList;
	const startIndex = cueList.cues.findIndex((cue) => cue.id === startCueId);
	const endIndex = cueList.cues.findIndex((cue) => cue.id === endCueId);
	if (startIndex < 0 || endIndex < startIndex) return cueList;
	const scale = (value: number | undefined) =>
		value === undefined ? undefined : Math.max(0, Math.round(value * ratio));
	return {
		...cueList,
		cues: cueList.cues.map((cue, index) => {
			if (index < startIndex || index > endIndex) return cue;
			return {
				...cue,
				delay_millis: scale(cue.delay_millis) ?? cue.delay_millis,
				fade_millis: scale(cue.fade_millis) ?? cue.fade_millis,
				out_delay_millis: scale(cue.out_delay_millis),
				out_fade_millis: scale(cue.out_fade_millis),
			};
		}),
	};
}

export function deleteTimelineItem(
	definition: TimecodeDefinition,
	selection: TimecodeEditorSelection,
): TimecodeDefinition {
	if (selection.kind === "marker")
		return {
			...definition,
			markers: definition.markers.filter(
				(marker) => marker.id !== selection.itemId,
			),
		};
	return {
		...definition,
		lanes: definition.lanes.map((lane) => {
			if (lane.id !== selection.laneId) return lane;
			if (lane.content.kind === "cue_list" && selection.kind === "clip")
				return {
					...lane,
					content: {
						...lane.content,
						clips: lane.content.clips.filter(
							(clip) => clip.id !== selection.itemId,
						),
					},
				};
			if (lane.content.kind === "speed_group" && selection.kind === "speed")
				return {
					...lane,
					content: {
						...lane.content,
						keyframes: lane.content.keyframes.filter(
							(keyframe) => keyframe.id !== selection.itemId,
						),
					},
				};
			if (lane.content.kind === "audio_volume" && selection.kind === "volume")
				return {
					...lane,
					content: {
						...lane.content,
						keyframes: lane.content.keyframes.filter(
							(keyframe) => keyframe.id !== selection.itemId,
						),
					},
				};
			if (lane.content.kind === "audio_player" && selection.kind === "clip")
				return {
					...lane,
					content: {
						...lane.content,
						clips: lane.content.clips.filter(
							(clip) => clip.id !== selection.itemId,
						),
					},
				};
			return lane;
		}),
	};
}

export function copyTimelineItem(
	definition: TimecodeDefinition,
	selection: TimecodeEditorSelection,
	newId: string,
	offsetFrames = 44,
): { definition: TimecodeDefinition; selection: TimecodeEditorSelection } {
	const item = timelineItems(definition).find((candidate) =>
		sameSelection(candidate.selection, selection),
	);
	if (!item) return { definition, selection };
	const copiedSelection = {
		...selection,
		itemId: newId,
	} as TimecodeEditorSelection;
	let next = definition;
	if (selection.kind === "marker") {
		const source = definition.markers.find(
			(marker) => marker.id === selection.itemId,
		);
		if (source)
			next = {
				...definition,
				markers: [
					...definition.markers,
					{ ...source, id: newId, name: `${source.name} copy` },
				],
			};
	} else {
		next = {
			...definition,
			lanes: definition.lanes.map((lane) => {
				if (lane.id !== selection.laneId) return lane;
				if (lane.content.kind === "cue_list" && selection.kind === "clip") {
					const source = lane.content.clips.find(
						(clip) => clip.id === selection.itemId,
					);
					return source
						? {
								...lane,
								content: {
									...lane.content,
									clips: [...lane.content.clips, { ...source, id: newId }],
								},
							}
						: lane;
				}
				if (lane.content.kind === "speed_group" && selection.kind === "speed") {
					const source = lane.content.keyframes.find(
						(keyframe) => keyframe.id === selection.itemId,
					);
					return source
						? {
								...lane,
								content: {
									...lane.content,
									keyframes: [
										...lane.content.keyframes,
										{ ...source, id: newId },
									],
								},
							}
						: lane;
				}
				if (
					lane.content.kind === "audio_volume" &&
					selection.kind === "volume"
				) {
					const source = lane.content.keyframes.find(
						(keyframe) => keyframe.id === selection.itemId,
					);
					return source
						? {
								...lane,
								content: {
									...lane.content,
									keyframes: [
										...lane.content.keyframes,
										{ ...source, id: newId },
									],
								},
							}
						: lane;
				}
				if (lane.content.kind === "audio_player" && selection.kind === "clip") {
					const source = lane.content.clips.find(
						(clip) => clip.id === selection.itemId,
					);
					return source
						? {
								...lane,
								content: {
									...lane.content,
									clips: [
										...lane.content.clips,
										{
											...source,
											id: newId,
											volume_keyframes: source.volume_keyframes.map(
												(keyframe) => ({
													...keyframe,
													id: crypto.randomUUID(),
												}),
											),
										},
									],
								},
							}
						: lane;
				}
				return lane;
			}),
		};
	}
	return {
		definition: moveTimelineItem(
			next,
			copiedSelection,
			item.frame + offsetFrames,
		),
		selection: copiedSelection,
	};
}

export function snapTimelineFrame(
	frame: number,
	definition: TimecodeDefinition,
	pixelsPerFrame: number,
	thresholdPixels = 12,
	options?: {
		selection?: TimecodeEditorSelection;
		movingClip?: boolean;
	},
): number {
	const duration = definition.duration_frame ?? Number.MAX_SAFE_INTEGER;
	const proposed = clampFrame(frame, duration);
	let best = proposed;
	let bestDistance = thresholdPixels / Math.max(pixelsPerFrame, 0.0001);
	const selectedItem = options?.selection
		? timelineItems(definition).find((item) =>
				sameSelection(item.selection, options.selection ?? null),
			)
		: undefined;
	const movingLength =
		options?.movingClip && selectedItem?.endFrame !== undefined
			? selectedItem.endFrame - selectedItem.frame
			: 0;
	const clipEdges = timelineItems(definition).flatMap((item) =>
		item.kind === "clip" &&
		item.endFrame !== undefined &&
		!sameSelection(item.selection, options?.selection ?? null)
			? [item.frame, item.endFrame]
			: [],
	);
	for (const candidate of [
		0,
		duration,
		...definition.markers.map((marker) => marker.frame),
		...clipEdges,
	]) {
		for (const anchored of [candidate, candidate - movingLength]) {
			const distance = Math.abs(anchored - proposed);
			if (distance <= bestDistance) {
				best = anchored;
				bestDistance = distance;
			}
		}
	}
	return clampFrame(best, duration);
}

function byStartFrame<T extends { start_frame: number }>(left: T, right: T) {
	return left.start_frame - right.start_frame;
}

function byFrame<T extends { frame: number }>(left: T, right: T) {
	return left.frame - right.frame;
}

function closestNonOverlappingStart<
	T extends { start_frame: number; end_frame: number },
>(
	proposed: number,
	length: number,
	otherClips: readonly T[],
	duration: number,
): number {
	const gaps: Array<[number, number]> = [];
	let gapStart = 0;
	for (const clip of [...otherClips].sort(byStartFrame)) {
		if (clip.start_frame - gapStart >= length)
			gaps.push([gapStart, clip.start_frame - length]);
		gapStart = Math.max(gapStart, clip.end_frame);
	}
	if (duration - gapStart >= length)
		gaps.push([gapStart, Math.max(gapStart, duration - length)]);
	if (!gaps.length) return clampFrame(proposed, Math.max(0, duration - length));
	return gaps.reduce((best, [minimum, maximum]) => {
		const candidate = Math.max(minimum, Math.min(maximum, proposed));
		return Math.abs(candidate - proposed) < Math.abs(best - proposed)
			? candidate
			: best;
	}, gaps[0][0]);
}

function resizeBounds<
	T extends { id: string; start_frame: number; end_frame: number },
>(clips: readonly T[], id: string, edge: "start" | "end", frame: number) {
	const selected = clips.find((clip) => clip.id === id);
	if (!selected) return { start: frame, end: frame };
	const others = clips.filter((clip) => clip.id !== id);
	const previousEnd = others.reduce(
		(maximum, clip) =>
			clip.end_frame <= selected.start_frame
				? Math.max(maximum, clip.end_frame)
				: maximum,
		0,
	);
	const nextStart = others.reduce(
		(minimum, clip) =>
			clip.start_frame >= selected.end_frame
				? Math.min(minimum, clip.start_frame)
				: minimum,
		Number.MAX_SAFE_INTEGER,
	);
	return edge === "start"
		? {
				start: Math.max(previousEnd, Math.min(frame, selected.end_frame - 1)),
				end: selected.end_frame,
			}
		: {
				start: selected.start_frame,
				end: Math.min(nextStart, Math.max(frame, selected.start_frame + 1)),
			};
}

export function parseMarkerCsv(
	source: string,
	fps: number,
	duration: number,
): TimecodeMarker[] {
	const markers: TimecodeMarker[] = [];
	for (const [index, raw] of source.split(/\r?\n/).entries()) {
		const line = raw.trim();
		if (!line || (index === 0 && /^position(?:,|$)/i.test(line))) continue;
		const cells = csvCells(line);
		if (!cells[0]?.trim())
			throw new Error(`Line ${index + 1}: position is required`);
		const frame = parsePosition(cells[0].trim(), fps);
		if (!Number.isInteger(frame) || frame < 0 || frame > duration)
			throw new Error(`Line ${index + 1}: position is outside this Timecode`);
		markers.push({
			id: crypto.randomUUID(),
			frame,
			name: cells[1]?.trim() || `Marker ${markers.length + 1}`,
			...(cells[2]?.trim() ? { color: cells[2].trim() } : {}),
		});
	}
	if (!markers.length) throw new Error("CSV contains no markers");
	return markers;
}

function parsePosition(value: string, fps: number): number {
	if (/^\d+$/.test(value)) return Number(value);
	const match = /^(\d+):(\d{1,2}):(\d{1,2}):(\d{1,2})$/.exec(value);
	if (!match) return Number.NaN;
	const [, hours, minutes, seconds, frames] = match.map(Number);
	if (minutes > 59 || seconds > 59 || frames >= fps) return Number.NaN;
	return (hours * 60 * 60 + minutes * 60 + seconds) * fps + frames;
}

function csvCells(line: string): string[] {
	const cells: string[] = [];
	let cell = "";
	let quoted = false;
	for (let index = 0; index < line.length; index += 1) {
		const character = line[index];
		if (character === '"') {
			if (quoted && line[index + 1] === '"') {
				cell += '"';
				index += 1;
			} else quoted = !quoted;
		} else if (character === "," && !quoted) {
			cells.push(cell);
			cell = "";
		} else cell += character;
	}
	if (quoted) throw new Error("CSV contains an unterminated quoted value");
	cells.push(cell);
	return cells;
}

function clampFrame(frame: number, duration: number): number {
	return Math.max(0, Math.min(duration, Math.round(frame)));
}

function padAddress(value: number): string {
	return String(value).padStart(3, "0");
}

/// Stores the lane-owned start of a Cue that waits for a manual GO.
///
/// A placed transition replaces any earlier one for the same Cue, so the clip keeps exactly one
/// answer for where that Cue begins.
export function withPlacedCueStart(
	definition: TimecodeDefinition,
	laneId: string,
	clipId: string,
	cueId: string,
	offsetFrame: number,
): TimecodeDefinition {
	return {
		...definition,
		lanes: definition.lanes.map((lane) =>
			lane.id !== laneId || lane.content.kind !== "cue_list"
				? lane
				: {
						...lane,
						content: {
							...lane.content,
							clips: lane.content.clips.map((clip) =>
								clip.id !== clipId
									? clip
									: {
											...clip,
											cue_starts: [
												...(clip.cue_starts ?? []).filter(
													(placed) => placed.cue_id !== cueId,
												),
												{ cue_id: cueId, offset_frame: offsetFrame },
											],
										},
							),
						},
					},
		),
	};
}
