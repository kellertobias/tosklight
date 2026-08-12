import { Button, CheckboxField, SelectField, TextField } from "@tosklight/ui";
import {
	PoolCard,
	PoolGrid,
	type PoolSlotViewModel,
} from "@tosklight/ui/pools";
import {
	WindowHeader,
	WindowScrollArea,
	WindowSettings,
} from "@tosklight/ui/window-kit";
import {
	type RefObject,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { createPortal } from "react-dom";
import { createLightApi } from "../api/client/api";
import {
	type TimecodeDefinition,
	type TimecodeObjectRecord,
	TimecodesApiClient,
	type TimecodeTransportAction,
	type TimecodeTransportSnapshot,
} from "../api/client/timecodes";
import { RootConfinedFilePickerButton } from "../components/files/RootConfinedFilePickerButton";
import { useActiveShowId } from "../features/deskSnapshot/DeskSnapshotState";
import { useCueLists } from "../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../features/showObjects/ShowObjectsView";
import { parseMarkerCsv } from "../features/timecode/editorModel";
import { useTimecodeActions } from "../features/timecode/TimecodeActionsContext";
import { TimecodeAutosaveWriter } from "../features/timecode/TimecodeAutosaveWriter";
import {
	TimecodeTimelineEditor,
	type TimecodeTimelineEditorHandle,
} from "../features/timecode/TimecodeTimelineEditor";
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
		const unsubscribe = configured?.events?.onRuntimeChanged((snapshot) => {
			setRuntime((current) => mergeTimecodeSnapshots(current, [snapshot]));
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
				onClose={async () => {
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

function useTimecodeWaveform(
	showId: string | null,
	isNew: boolean,
	draft: TimecodeDefinition,
	api: TimecodesApiClient,
) {
	const [waveformPeaks, setWaveformPeaks] = useState<number[] | undefined>();
	const [waveformError, setWaveformError] = useState<string | null>(null);
	useEffect(() => {
		if (!showId || isNew || !draft.audio || waveformPeaks) return;
		let cancelled = false;
		void api
			.waveform(showId, draft.id)
			.then((waveform) => !cancelled && setWaveformPeaks(waveform.peaks))
			.catch((reason) => !cancelled && setWaveformError(String(reason)));
		return () => {
			cancelled = true;
		};
	}, [api, draft.audio, draft.id, isNew, showId, waveformPeaks]);
	return { waveformPeaks, setWaveformPeaks, waveformError };
}

export function TimecodeEditor({
	showId,
	item,
	api,
	snapshot,
	cueLists,
	onClose,
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
	onClose(): Promise<void>;
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
	const [editorFrame, setEditorFrame] = useState(snapshot?.frame ?? 0);
	const [error, setError] = useState<string | null>(null);
	const [actionBusy, setActionBusy] = useState(false);
	const [audioImporting, setAudioImporting] = useState(false);
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
	const [settingsOpen, setSettingsOpen] = useState(false);
	const [csvMode, setCsvMode] = useState<"append" | "replace">("append");
	const [csvError, setCsvError] = useState<string | null>(null);
	const timelineRef = useRef<TimecodeTimelineEditorHandle>(null);
	const initialRecord = "isNew" in item ? null : item;
	const writer = useMemo(
		() =>
			showId ? new TimecodeAutosaveWriter(showId, initialRecord, api) : null,
		[api, item, showId],
	);
	const [record, setRecord] = useState<TimecodeObjectRecord | null>(
		initialRecord,
	);
	const [saving, setSaving] = useState(Boolean(writer && !initialRecord));
	useEffect(() => {
		if (!writer) return;
		let current = true;
		setSaving(true);
		void writer
			.enqueue(draft)
			.then((saved) => {
				if (!current) return;
				setRecord(saved);
				setError(null);
			})
			.catch((reason) => {
				if (current)
					setError(
						`Autosave failed: ${reason instanceof Error ? reason.message : String(reason)}`,
					);
			})
			.finally(() => current && setSaving(false));
		return () => {
			current = false;
		};
	}, [draft, writer]);
	const isNew = !record;
	const { waveformPeaks, setWaveformPeaks, waveformError } =
		useTimecodeWaveform(showId, isNew, draft, api);
	useEffect(() => {
		if (waveformError) setError(waveformError);
	}, [waveformError]);
	const duration = draft.duration_frame ?? 0;
	const frame = Math.min(editorFrame, duration);
	const busy = saving || actionBusy;
	const act = async (action: TimecodeTransportAction) => {
		if (!showId || !record) return;
		setActionBusy(true);
		setError(null);
		try {
			await api.transportAction(showId, draft.id, action);
		} catch (reason) {
			setError(String(reason));
		} finally {
			setActionBusy(false);
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
			setDraft({
				...draft,
				duration_frame: Math.ceil(
					(imported.sample_frames * FPS) / imported.sample_rate,
				),
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
	const close = async () => {
		try {
			await writer?.flush();
			await onClose();
		} catch (reason) {
			setError(`Could not close before autosave completed: ${String(reason)}`);
		}
	};
	const remove = async () => {
		if (!showId || !writer) return;
		setActionBusy(true);
		try {
			const saved = await writer.flush();
			if (!saved) return;
			await api.delete(showId, saved.definition.id, saved.revision);
			await onClose();
		} catch (reason) {
			setError(String(reason));
		} finally {
			setActionBusy(false);
		}
	};
	const importCsv = async (file: File) => {
		try {
			const csvSource = await file.text();
			const imported = parseMarkerCsv(csvSource, FPS, Math.max(1, duration));
			setDraft({
				...draft,
				markers:
					csvMode === "append" ? [...draft.markers, ...imported] : imported,
			});
			setCsvError(null);
		} catch (reason) {
			setCsvError(reason instanceof Error ? reason.message : String(reason));
		}
	};
	return (
		<section className="timecode-window timecode-editor" aria-busy={busy}>
			<WindowHeader
				title={`Timecode ${draft.number}`}
				info={{
					primary: snapshot
						? `${formatFrame(snapshot.frame)} · ${snapshot.state}`
						: "Stopped",
					secondary: saving
						? "Saving changes…"
						: record
							? "Saved"
							: "Creating…",
				}}
				toolbar={
					<TimecodeHeaderToolbar
						{...{ act, busy, isNew, timelineRef, draft, cueLists }}
					/>
				}
				settings
				onSettings={(anchor) => {
					setSettingsAnchor(anchor.getBoundingClientRect());
					setSettingsOpen(true);
				}}
			/>
			{settingsOpen && (
				<WindowSettings
					modal={false}
					anchor={settingsAnchor}
					title="Timecode Settings"
					onClose={() => setSettingsOpen(false)}
					tabs={[
						{
							id: "settings",
							label: "Settings",
							content: (
								<TimecodeSettings
									{...{
										draft,
										setDraft,
										duration,
										busy,
										audioImporting,
										importAudio,
										csvMode,
										setCsvMode,
										csvError,
										importCsv,
									}}
								/>
							),
						},
					]}
				/>
			)}
			{error && (
				<p className="timecode-error" role="alert">
					{error}
				</p>
			)}
			<EditorToolbar
				{...{
					onClose: close,
					undo,
					redo,
					remove,
					canUndo,
					canRedo,
					busy,
					isNew,
				}}
			/>
			<RuntimeSeekControls {...{ act, frame, snapshot, busy, isNew }} />
			<TimecodeTimelineEditor
				ref={timelineRef}
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

function EditorToolbar({
	onClose,
	undo,
	redo,
	remove,
	canUndo,
	canRedo,
	busy,
	isNew,
}: {
	onClose(): Promise<void>;
	undo(): void;
	redo(): void;
	remove(): Promise<void>;
	canUndo: boolean;
	canRedo: boolean;
	busy: boolean;
	isNew: boolean;
}) {
	return (
		<div className="timecode-editor-toolbar">
			<Button onClick={() => void onClose()} disabled={busy}>
				Back
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
	);
}

function RuntimeSeekControls({
	act,
	frame,
	snapshot,
	busy,
	isNew,
}: {
	act(action: TimecodeTransportAction): Promise<void>;
	frame: number;
	snapshot?: TimecodeTransportSnapshot;
	busy: boolean;
	isNew: boolean;
}) {
	const disabled = isNew || busy;
	return (
		<fieldset className="timecode-runtime-seek" aria-label="Runtime seek">
			<Button
				onClick={() => void act({ type: "seek", frame })}
				disabled={disabled}
			>
				Seek runtime to playhead
			</Button>
			<strong>{formatFrame(snapshot?.frame ?? 0)}</strong>
		</fieldset>
	);
}

export function TimecodeSettings({
	draft,
	setDraft,
	duration,
	busy,
	audioImporting,
	importAudio,
	csvMode,
	setCsvMode,
	csvError,
	importCsv,
}: {
	draft: TimecodeDefinition;
	setDraft(value: TimecodeDefinition): void;
	duration: number;
	busy: boolean;
	audioImporting: boolean;
	importAudio(file: File): Promise<void>;
	csvMode: "append" | "replace";
	setCsvMode(value: "append" | "replace"): void;
	csvError: string | null;
	importCsv(file: File): Promise<void>;
}) {
	const changeFrameField = (
		field: "duration_frame" | "transport_offset_frame",
		value: string,
	) => {
		const frame = parseFrame(value);
		if (frame === null || (field === "duration_frame" && frame < 1)) return;
		setDraft({ ...draft, [field]: frame });
	};
	return (
		<div className="timecode-settings-fields">
			<TextField
				label="Name"
				value={draft.name}
				onChange={(event) =>
					setDraft({ ...draft, name: event.currentTarget.value })
				}
			/>
			<div className="timecode-duration-fields">
				<TextField
					label="Duration"
					value={formatFrame(duration)}
					pattern="[0-9]+:[0-5][0-9]:[0-5][0-9]:[0-9]+"
					onChange={(event) =>
						changeFrameField("duration_frame", event.currentTarget.value)
					}
				/>
				<TextField
					label="Transport offset"
					value={formatFrame(draft.transport_offset_frame)}
					pattern="[0-9]+:[0-5][0-9]:[0-5][0-9]:[0-9]+"
					onChange={(event) =>
						changeFrameField(
							"transport_offset_frame",
							event.currentTarget.value,
						)
					}
				/>
			</div>
			<CheckboxField
				className="timecode-checkbox"
				label="Auto-start"
				stateLabel="Start with external Timecode"
				checked={draft.auto_start}
				onChange={(event) =>
					setDraft({ ...draft, auto_start: event.currentTarget.checked })
				}
			/>
			<div className="timecode-audio-import">
				<span>Audio file</span>
				<RootConfinedFilePickerButton
					label="Choose audio file"
					allowedExtensions={["wav", "mp3"]}
					disabled={busy || audioImporting}
					onFiles={async ([file]) => file && importAudio(file)}
				/>
				<small>
					{audioImporting
						? "Importing and normalizing…"
						: draft.audio
							? `Managed audio ${draft.audio.asset_id}`
							: "WAV or MP3; MP3 is normalized to managed WAV."}
				</small>
			</div>
			<div className="timecode-csv-panel">
				<SelectField
					label="Import mode"
					value={csvMode}
					onChange={setCsvMode}
					options={[
						{ value: "append", label: "Append" },
						{ value: "replace", label: "Replace" },
					]}
				/>
				<RootConfinedFilePickerButton
					label="Choose marker CSV"
					allowedExtensions={["csv"]}
					disabled={busy}
					onFiles={async ([file]) => file && importCsv(file)}
				/>
				{csvError && <p role="alert">{csvError}</p>}
			</div>
		</div>
	);
}

function TimecodeHeaderToolbar({
	act,
	busy,
	isNew,
	timelineRef,
	draft,
	cueLists,
}: {
	act(action: TimecodeTransportAction): Promise<void>;
	busy: boolean;
	isNew: boolean;
	timelineRef: RefObject<TimecodeTimelineEditorHandle | null>;
	draft: TimecodeDefinition;
	cueLists: readonly unknown[];
}) {
	const [addAnchor, setAddAnchor] = useState<DOMRect | null>(null);
	const disabled = busy || isNew;
	const runAdd = (action: () => void) => {
		setAddAnchor(null);
		action();
	};
	return (
		<>
			<div className="timecode-header-toolbar">
				<fieldset
					className="timecode-header-group"
					aria-label="Timecode transport"
				>
					<Button onClick={() => void act({ type: "go" })} disabled={disabled}>
						Go
					</Button>
					<Button
						onClick={() => void act({ type: "pause" })}
						disabled={disabled}
					>
						Pause
					</Button>
					<Button
						onClick={() => void act({ type: "stop" })}
						disabled={disabled}
					>
						Stop
					</Button>
					<Button
						onClick={() => void act({ type: "rewind" })}
						disabled={disabled}
					>
						Rewind
					</Button>
				</fieldset>
				<div className="timecode-header-group">
					<Button
						aria-haspopup="menu"
						aria-expanded={Boolean(addAnchor)}
						onClick={(event) =>
							setAddAnchor((current) =>
								current ? null : event.currentTarget.getBoundingClientRect(),
							)
						}
					>
						Add
					</Button>
				</div>
			</div>
			{addAnchor &&
				createPortal(
					<div
						className="timecode-add-menu-layer"
						onPointerDown={(event) =>
							event.target === event.currentTarget && setAddAnchor(null)
						}
					>
						<div
							className="timecode-add-menu"
							role="menu"
							aria-label="Add"
							style={{ top: addAnchor.bottom + 3, left: addAnchor.left }}
						>
							<Button
								role="menuitem"
								onClick={() => runAdd(() => timelineRef.current?.addMarker())}
							>
								Add Marker
							</Button>
							<Button
								role="menuitem"
								disabled={draft.lanes.some(
									(lane) => lane.content.kind === "audio_volume",
								)}
								onClick={() =>
									runAdd(() => timelineRef.current?.addAudioLane())
								}
							>
								Add Audio Lane
							</Button>
							<Button
								role="menuitem"
								onClick={() =>
									runAdd(() => timelineRef.current?.addSpeedLane())
								}
							>
								Add Speed Lane
							</Button>
							<Button
								role="menuitem"
								disabled={!cueLists.length}
								onClick={() =>
									runAdd(() => timelineRef.current?.addCueListLane())
								}
							>
								Add Cuelist Lane
							</Button>
						</div>
					</div>,
					document.body,
				)}
		</>
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

export function parseFrame(value: string): number | null {
	const match = /^(\d+):([0-5]\d):([0-5]\d):(\d+)$/.exec(value.trim());
	if (!match) return null;
	const [, hours, minutes, seconds, frames] = match;
	const framePart = Number(frames);
	if (framePart >= FPS) return null;
	return (
		(Number(hours) * 60 * 60 + Number(minutes) * 60 + Number(seconds)) * FPS +
		framePart
	);
}
