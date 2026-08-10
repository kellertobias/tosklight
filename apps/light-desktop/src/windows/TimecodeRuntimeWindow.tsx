import {
	Button,
	CheckboxField,
	Input,
	NumberField,
	TextField,
} from "@tosklight/ui";
import {
	PoolCard,
	PoolGrid,
	type PoolSlotViewModel,
} from "@tosklight/ui/pools";
import { WindowHeader, WindowScrollArea } from "@tosklight/ui/window-kit";
import { useCallback, useEffect, useMemo, useState } from "react";
import { createLightApi } from "../api/client/api";
import {
	type TimecodeObjectRecord,
	TimecodesApiClient,
} from "../api/client/timecodes";
import type {
	TimecodeDefinition,
	TimecodeTransportAction,
	TimecodeTransportSnapshot,
} from "../api/generated/light-wire";
import { useActiveShowId } from "../features/deskSnapshot/DeskSnapshotState";
import { useCueLists } from "../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../features/showObjects/ShowObjectsView";
import { useTimecodeActions } from "../features/timecode/TimecodeActionsContext";
import { TimecodeTimelineEditor } from "../features/timecode/TimecodeTimelineEditor";
import { useTimecodeEditorHistory } from "../features/timecode/useTimecodeEditorHistory";
import type { WindowProps } from "./windowTypes";
import "./TimecodeRuntimeWindow.css";

const FPS = 44;
const TIMECODE_POOL_SIZE = 100;

export function TimecodeRuntimeWindow({
	active = true,
	compact = false,
}: WindowProps) {
	const showId = useActiveShowId();
	const cueLists = useCueLists(active);
	useShowObjectView("cue_list", active);
	const timelineCueLists = useMemo(
		() =>
			cueLists.map((cueList) => ({
				id: cueList.body.id,
				name: cueList.body.name,
				cues: cueList.body.cues.flatMap((cue) =>
					cue.id ? [{ id: cue.id, number: cue.number, name: cue.name }] : [],
				),
			})),
		[cueLists],
	);
	const fallback = useMemo(
		() =>
			new TimecodesApiClient(createLightApi().runtime.capabilityTransport()),
		[],
	);
	const configured = useTimecodeActions();
	const api = configured?.api ?? fallback;
	const [objects, setObjects] = useState<TimecodeObjectRecord[]>([]);
	const [runtime, setRuntime] = useState<
		Map<string, TimecodeTransportSnapshot>
	>(new Map());
	const [editing, setEditing] = useState<
		TimecodeObjectRecord | NewTimecode | null
	>(null);
	const [error, setError] = useState<string | null>(null);

	const refresh = useCallback(async () => {
		if (!showId) return;
		const [collection, snapshots] = await Promise.all([
			api.objects(showId),
			api.runtime(showId),
		]);
		setObjects(collection.objects);
		setRuntime((current) => mergeTimecodeSnapshots(current, snapshots));
	}, [api, showId]);

	useEffect(() => {
		if (!active || !showId) return;
		let cancelled = false;
		const update = () =>
			void refresh().catch((reason) => !cancelled && setError(String(reason)));
		const unsubscribe = configured?.events?.onEvent((event) => {
			if (event.type !== "timecode_runtime_changed") return;
			setRuntime((current) =>
				mergeTimecodeSnapshots(current, [event.snapshot]),
			);
		});
		update();
		return () => {
			cancelled = true;
			unsubscribe?.();
		};
	}, [active, configured?.events, refresh, showId]);

	if (editing) {
		return (
			<TimecodeEditor
				showId={showId}
				item={editing}
				api={api}
				snapshot={runtime.get(editing.definition.id)}
				cueLists={timelineCueLists}
				onClose={() => setEditing(null)}
				onSaved={async () => {
					await refresh();
					setEditing(null);
				}}
			/>
		);
	}

	const byNumber = new Map(
		objects.map((object) => [object.definition.number, object]),
	);
	const slots: PoolSlotViewModel<number>[] = objects.map((object) => ({
		id: object.definition.number,
		position: object.definition.number - 1,
		card: { number: object.definition.number, primary: object.definition.name },
	}));
	return (
		<section className="timecode-window">
			{!compact && (
				<WindowHeader
					title="Timecode"
					info={{ primary: `${objects.length} Timecodes` }}
					actions={[]}
				/>
			)}
			{error && (
				<p className="timecode-error" role="alert">
					{error}
				</p>
			)}
			<WindowScrollArea>
				<PoolGrid
					slots={slots}
					slotCount={Math.max(
						TIMECODE_POOL_SIZE,
						...objects.map((object) => object.definition.number),
					)}
					emptySlot={(index) => ({
						id: index + 1,
						position: index,
						card: { number: index + 1, primary: "Empty", states: ["empty"] },
					})}
					renderSlot={(_, index) => {
						const number = index + 1;
						const item = byNumber.get(number);
						const snapshot = item ? runtime.get(item.definition.id) : undefined;
						return (
							<PoolCard
								key={number}
								aria-label={
									item
										? `Timecode ${number} ${item.definition.name}`
										: `Empty Timecode ${number}`
								}
								model={{
									number,
									primary: item?.definition.name ?? "Empty",
									secondary: snapshot
										? `${formatFrame(snapshot.frame)} · ${snapshot.state}`
										: item
											? "Not running"
											: "Tap to create",
									color: "#9365d8",
									states: [
										...(!item ? ["empty" as const] : []),
										...(snapshot?.state === "playing"
											? ["active" as const]
											: []),
									],
								}}
								onClick={() => setEditing(item ?? newTimecode(number))}
							/>
						);
					}}
				/>
			</WindowScrollArea>
		</section>
	);
}

function mergeTimecodeSnapshots(
	current: ReadonlyMap<string, TimecodeTransportSnapshot>,
	incoming: readonly TimecodeTransportSnapshot[],
): Map<string, TimecodeTransportSnapshot> {
	const next = new Map(current);
	for (const snapshot of incoming) {
		const previous = next.get(snapshot.timecode_id);
		if (!previous || snapshot.revision >= previous.revision) {
			next.set(snapshot.timecode_id, snapshot);
		}
	}
	return next;
}

interface NewTimecode {
	revision: 0;
	definition: TimecodeDefinition;
	isNew: true;
}

function newTimecode(number: number): NewTimecode {
	return {
		revision: 0,
		isNew: true,
		definition: {
			id: crypto.randomUUID(),
			number,
			name: `Timecode ${number}`,
			duration_frame: FPS * 60,
			transport_offset_frame: 0,
			auto_start: false,
			markers: [],
			lanes: [],
		},
	};
}

function TimecodeEditor({
	showId,
	item,
	api,
	snapshot,
	cueLists,
	onClose,
	onSaved,
}: {
	showId: string | null;
	item: TimecodeObjectRecord | NewTimecode;
	api: TimecodesApiClient;
	snapshot?: TimecodeTransportSnapshot;
	cueLists: Array<{
		id: string;
		name: string;
		cues: Array<{ id: string; number: number; name: string }>;
	}>;
	onClose(): void;
	onSaved(): Promise<void>;
}) {
	const {
		draft,
		commit: setDraft,
		preview: previewDraft,
		beginGesture,
		endGesture,
		undo,
		redo,
		canUndo,
		canRedo,
	} = useTimecodeEditorHistory(item.definition);
	const [busy, setBusy] = useState(false);
	const [audioImporting, setAudioImporting] = useState(false);
	const [waveformPeaks, setWaveformPeaks] = useState<number[] | undefined>();
	const [editorFrame, setEditorFrame] = useState(snapshot?.frame ?? 0);
	const [error, setError] = useState<string | null>(null);
	const isNew = "isNew" in item;
	useEffect(() => {
		if (!showId || isNew || !draft.audio || waveformPeaks) return;
		let cancelled = false;
		void api
			.waveform(showId, draft.id)
			.then((waveform) => !cancelled && setWaveformPeaks(waveform.peaks))
			.catch((reason) => !cancelled && setError(String(reason)));
		return () => {
			cancelled = true;
		};
	}, [api, draft.audio, draft.id, isNew, showId, waveformPeaks]);
	const act = async (action: TimecodeTransportAction) => {
		if (!showId || isNew) return;
		setBusy(true);
		setError(null);
		try {
			await api.transportAction(showId, draft.id, action);
		} catch (reason) {
			setError(String(reason));
		} finally {
			setBusy(false);
		}
	};
	const save = async () => {
		if (!showId) return;
		setBusy(true);
		setError(null);
		try {
			if (isNew) await api.create(showId, draft);
			else
				await api.update(showId, draft.id, item.revision, {
					number: draft.number,
					name: draft.name,
					duration_frame: draft.duration_frame,
					transport_offset_frame: draft.transport_offset_frame,
					auto_start: draft.auto_start,
					audio: draft.audio ?? null,
					markers: draft.markers,
					lanes: draft.lanes,
				});
			await onSaved();
		} catch (reason) {
			setError(String(reason));
		} finally {
			setBusy(false);
		}
	};
	const importAudio = async (file: File) => {
		if (!showId) return;
		setAudioImporting(true);
		setError(null);
		try {
			const [imported, peaks] = await Promise.all([
				api.importAudio(showId, file),
				decodeAudioPeaks(file).catch(() => undefined),
			]);
			const audioDuration = Math.ceil(
				(imported.sample_frames * FPS) / imported.sample_rate,
			);
			setDraft({
				...draft,
				duration_frame: audioDuration,
				audio: {
					asset_id: imported.asset_id,
					asset_revision: imported.asset_revision,
				},
			});
			setWaveformPeaks(peaks);
		} catch (reason) {
			setError(reason instanceof Error ? reason.message : String(reason));
		} finally {
			setAudioImporting(false);
		}
	};
	const remove = async () => {
		if (!showId || isNew) return;
		setBusy(true);
		try {
			await api.delete(showId, draft.id, item.revision);
			await onSaved();
		} catch (reason) {
			setError(String(reason));
			setBusy(false);
		}
	};
	const duration = draft.duration_frame ?? 0;
	const frame = Math.min(editorFrame, duration);
	return (
		<section className="timecode-window timecode-editor" aria-busy={busy}>
			<WindowHeader
				title={`Timecode ${draft.number}`}
				info={{
					primary: snapshot
						? `${formatFrame(snapshot.frame)} · ${snapshot.state}`
						: "Stopped",
				}}
				actions={[]}
			/>
			{error && (
				<p className="timecode-error" role="alert">
					{error}
				</p>
			)}
			<div className="timecode-editor-toolbar">
				<Button onClick={onClose}>Back</Button>
				<Button onClick={() => void save()} disabled={busy}>
					Save
				</Button>
				<Button onClick={undo} disabled={!canUndo || busy}>
					Undo
				</Button>
				<Button onClick={redo} disabled={!canRedo || busy}>
					Redo
				</Button>
				{!isNew && (
					<Button onClick={() => void remove()} disabled={busy}>
						Delete
					</Button>
				)}
			</div>
			<fieldset className="timecode-transport" aria-label="Timecode transport">
				<Button
					onClick={() => void act({ type: "go" })}
					disabled={isNew || busy}
				>
					Go
				</Button>
				<Button
					onClick={() => void act({ type: "pause" })}
					disabled={isNew || busy}
				>
					Pause
				</Button>
				<Button
					onClick={() => void act({ type: "stop" })}
					disabled={isNew || busy}
				>
					Stop
				</Button>
				<Button
					onClick={() => void act({ type: "rewind" })}
					disabled={isNew || busy}
				>
					Rewind
				</Button>
				<Button
					onClick={() => void act({ type: "seek", frame })}
					disabled={isNew || busy}
				>
					Seek runtime to playhead
				</Button>
				<strong>{formatFrame(snapshot?.frame ?? 0)}</strong>
			</fieldset>
			<div className="timecode-fields">
				<NumberField
					label="Number"
					min={1}
					value={draft.number}
					onChange={(event) =>
						setDraft({ ...draft, number: Number(event.currentTarget.value) })
					}
				/>
				<TextField
					label="Name"
					value={draft.name}
					onChange={(event) =>
						setDraft({ ...draft, name: event.currentTarget.value })
					}
				/>
				<NumberField
					label="Duration frames"
					min={1}
					value={duration}
					onChange={(event) =>
						setDraft({
							...draft,
							duration_frame: Number(event.currentTarget.value),
						})
					}
				/>
				<NumberField
					label="Transport offset"
					min={0}
					value={draft.transport_offset_frame}
					onChange={(event) =>
						setDraft({
							...draft,
							transport_offset_frame: Number(event.currentTarget.value),
						})
					}
				/>
				<CheckboxField
					className="timecode-checkbox"
					label="Auto-start"
					checked={draft.auto_start}
					onChange={(event) =>
						setDraft({ ...draft, auto_start: event.currentTarget.checked })
					}
				/>
				<label className="timecode-audio-import" htmlFor="timecode-audio-file">
					<span>Audio file</span>
					<Input
						id="timecode-audio-file"
						type="file"
						accept="audio/wav,audio/x-wav,audio/mpeg,.wav,.mp3"
						disabled={busy || audioImporting}
						onChange={(event) => {
							const file = event.currentTarget.files?.[0];
							if (file) void importAudio(file);
						}}
					/>
					<small>
						{audioImporting
							? "Importing and normalizing…"
							: draft.audio
								? `Managed audio ${draft.audio.asset_id}`
								: "WAV or MP3; MP3 is normalized to managed WAV."}
					</small>
				</label>
			</div>
			<TimecodeTimelineEditor
				definition={draft}
				frame={frame}
				fps={FPS}
				cueLists={cueLists}
				waveformPeaks={waveformPeaks}
				onScrub={setEditorFrame}
				onCommit={setDraft}
				onPreview={previewDraft}
				onBeginGesture={beginGesture}
				onEndGesture={endGesture}
			/>
		</section>
	);
}

async function decodeAudioPeaks(file: File, count = 220): Promise<number[]> {
	const AudioContextConstructor = window.AudioContext;
	const context = new AudioContextConstructor();
	try {
		const buffer = await context.decodeAudioData(await file.arrayBuffer());
		const peaks = Array.from({ length: count }, () => 0);
		for (let channel = 0; channel < buffer.numberOfChannels; channel += 1) {
			const samples = buffer.getChannelData(channel);
			for (let index = 0; index < count; index += 1) {
				const start = Math.floor((index * samples.length) / count);
				const end = Math.max(
					start + 1,
					Math.floor(((index + 1) * samples.length) / count),
				);
				let peak = 0;
				for (
					let sample = start;
					sample < end;
					sample += Math.max(1, Math.floor((end - start) / 128))
				)
					peak = Math.max(peak, Math.abs(samples[sample] ?? 0));
				peaks[index] = Math.max(peaks[index], peak);
			}
		}
		return peaks;
	} finally {
		void context.close();
	}
}

export function formatFrame(frame: number): string {
	const seconds = Math.floor(frame / FPS);
	return `${String(Math.floor(seconds / 3600)).padStart(2, "0")}:${String(Math.floor(seconds / 60) % 60).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}:${String(frame % FPS).padStart(2, "0")}`;
}
