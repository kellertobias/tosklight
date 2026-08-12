import type {
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
}

export function timelineItems(definition: TimecodeDefinition): TimelineItem[] {
	const items: TimelineItem[] = definition.markers.map((marker) => ({
		selection: { kind: "marker", itemId: marker.id },
		frame: marker.frame,
		label: marker.name,
		kind: "marker",
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
					return {
						...lane,
						content: {
							...lane.content,
							clips: lane.content.clips.map((clip) => {
								if (clip.id !== selection.itemId) return clip;
								const length = clip.end_frame - clip.start_frame;
								const start = Math.min(
									nextFrame,
									Math.max(0, duration - length),
								);
								return {
									...clip,
									start_frame: start,
									end_frame: start + length,
								};
							}),
						},
					};
				case "speed_group":
					if (selection.kind !== "speed") return lane;
					return {
						...lane,
						content: {
							...lane.content,
							keyframes: lane.content.keyframes.map((keyframe) =>
								keyframe.id === selection.itemId
									? { ...keyframe, frame: nextFrame }
									: keyframe,
							),
						},
					};
				case "audio_volume":
					if (selection.kind !== "volume") return lane;
					return {
						...lane,
						content: {
							...lane.content,
							keyframes: lane.content.keyframes.map((keyframe) =>
								keyframe.id === selection.itemId
									? { ...keyframe, frame: nextFrame }
									: keyframe,
							),
						},
					};
				case "audio_player":
					if (selection.kind !== "clip") return lane;
					return {
						...lane,
						content: {
							...lane.content,
							clips: lane.content.clips.map((clip) => {
								if (clip.id !== selection.itemId) return clip;
								const length = clip.end_frame - clip.start_frame;
								const start = Math.min(
									nextFrame,
									Math.max(0, duration - length),
								);
								const offset = start - clip.start_frame;
								return {
									...clip,
									start_frame: start,
									end_frame: start + length,
									volume_keyframes: clip.volume_keyframes.map((keyframe) => ({
										...keyframe,
										frame: keyframe.frame + offset,
									})),
								};
							}),
						},
					};
			}
			return lane;
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
): number {
	const duration = definition.duration_frame ?? Number.MAX_SAFE_INTEGER;
	const proposed = clampFrame(frame, duration);
	let best = proposed;
	let bestDistance = thresholdPixels / Math.max(pixelsPerFrame, 0.0001);
	for (const candidate of [
		0,
		duration,
		...definition.markers.map((marker) => marker.frame),
	]) {
		const distance = Math.abs(candidate - proposed);
		if (distance <= bestDistance) {
			best = candidate;
			bestDistance = distance;
		}
	}
	return best;
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
