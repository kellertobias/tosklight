import {
	Button,
	CheckboxField,
	SelectField,
	TextField,
	type TitleAction,
} from "@tosklight/ui";
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
import { createLightApi } from "../api/client/api";
import {
	type TimecodeDefinition,
	type TimecodeObjectRecord,
	TimecodesApiClient,
	type TimecodeTransportAction,
	type TimecodeTransportSnapshot,
} from "../api/client/timecodes";
import type { CueList } from "../api/types";
import { useCommandLineSurface } from "../components/control/commandLine/useCommandLineSurface";
import { RootConfinedFilePickerButton } from "../components/files/RootConfinedFilePickerButton";
import {
	useReleaseFadeMillis,
	useSequenceMasterFadeMillis,
} from "../features/configuration/ConfigurationState";
import {
	consumeObjectEditorRequest,
	currentObjectEditorRequest,
	subscribeObjectEditorRequest,
} from "../features/controlSurfaceInteraction/objectEditorRequest";
import { useActiveShowId } from "../features/deskSnapshot/DeskSnapshotState";
import { usePatchedFixturesView } from "../features/patch/PatchState";
import {
	type SaveCueListTopology,
	useCueListTopologyWriter,
} from "../features/playbackTopology/useCueListTopologyWriter";
import { useCueTimingWriter } from "../features/timecode/useCueTimingWriter";
import {
	useCueLists,
	usePlaybackDefinitions,
} from "../features/showObjects/ShowObjectsState";
import type { ShowObject } from "../features/showObjects/contracts";
import { useShowObjectView } from "../features/showObjects/ShowObjectsView";
import {
	parseMarkerCsv,
	reconcileAutomaticAudioLane,
} from "../features/timecode/editorModel";
import { useTimecodeActions } from "../features/timecode/TimecodeActionsContext";
import { useTimecodeAutosave } from "../features/timecode/useTimecodeAutosave";
import {
	type TimecodeAudioPlayerOption,
	type TimecodeCueListOption,
	TimecodeTimelineEditor,
	type TimecodeTimelineEditorHandle,
} from "../features/timecode/TimecodeTimelineEditor";
import {
	timecodeTransportActions,
	useTimecodeEditorTransport,
} from "../features/timecode/TimecodeEditorTransport";
import { useTimecodeWaveform } from "../features/timecode/useTimecodeWaveform";
import { TimecodeEditorFeedback } from "../features/timecode/TimecodeEditorFeedback";
import { TimecodeFrameField } from "../features/timecode/TimecodeFrameField";
import { useTimecodeEditorHistory } from "../features/timecode/useTimecodeEditorHistory";
import type { WindowProps } from "./windowTypes";
import "./TimecodeRuntimeWindow.css";

const FPS = 44;
const TIMECODE_POOL_SIZE = 100;
const AUDIO_PLAYER_PROFILE_ID = "358171f4-c3a7-4d75-b256-b9cf30afd4ab";

/// The Cuelists the timeline offers, each carrying the playback number it is addressed by.
function useTimelineCueLists(
	cueLists: readonly ShowObject<"cue_list">[],
	active: boolean,
): TimecodeCueListOption[] {
	const numbers = useCueListPlaybackNumbers(active);
	return useMemo(
		() =>
			cueLists.map((cueList) => ({
				id: cueList.body.id,
				name: cueList.body.name,
				number: numbers.get(cueList.body.id),
				cues: cueList.body.cues,
				objectId: cueList.id,
				revision: cueList.revision,
				body: cueList.body,
			})),
		[cueLists, numbers],
	);
}

/// The playback number each Cuelist is addressed by, when it has one.
///
/// A Cuelist carries no number of its own; the number belongs to the playback that targets it, so
/// a Cuelist in several playbacks is named by the lowest.
function useCueListPlaybackNumbers(active: boolean): Map<string, number> {
	const playbacks = usePlaybackDefinitions(active);
	return useMemo(() => {
		const numbers = new Map<string, number>();
		for (const playback of playbacks) {
			const target = playback.body.target;
			if (target.type !== "cue_list") continue;
			const existing = numbers.get(target.cue_list_id);
			if (existing === undefined || playback.body.number < existing)
				numbers.set(target.cue_list_id, playback.body.number);
		}
		return numbers;
	}, [playbacks]);
}

export function TimecodeRuntimeWindow({
	active = true,
	compact = false,
}: WindowProps) {
	const showId = useActiveShowId();
	const command = useCommandLineSurface({
		enabled: active,
		observeCommand: true,
	});
	const cueLists = useCueLists(active);
	const saveCueList = useCueListTopologyWriter();
	const sequenceFadeMillis = useSequenceMasterFadeMillis() ?? 3_000;
	const releaseFadeMillis = useReleaseFadeMillis() ?? 3_000;
	const patchedFixtures = usePatchedFixturesView(active);
	const audioPlayers = useMemo<TimecodeAudioPlayerOption[]>(
		() =>
			patchedFixtures
				.filter(
					(fixture) =>
						fixture.definition.profile_id === AUDIO_PLAYER_PROFILE_ID,
				)
				.map((fixture) => ({
					fixtureId: fixture.fixture_id,
					name: fixture.fixture_number
						? `Audio Player ${fixture.fixture_number}`
						: fixture.name?.trim() || `Audio Player ${fixture.fixture_id}`,
				})),
		[patchedFixtures],
	);
	useShowObjectView("cue_list", active);
	const timelineCueLists = useTimelineCueLists(cueLists, active);
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
	const start = useCallback(
		async (item: TimecodeObjectRecord) => {
			if (!showId) return;
			try {
				await api.transportAction(showId, item.definition.id, { type: "go" });
				setError(null);
			} catch (reason) {
				setError(String(reason));
			}
		},
		[api, showId],
	);
	const stop = useCallback(
		async (item: TimecodeObjectRecord) => {
			if (!showId || !/^OFF$/iu.test(command.read().text.trim())) return;
			try {
				const snapshot = await api.transportAction(showId, item.definition.id, {
					type: "stop",
				});
				setRuntime((current) => mergeTimecodeSnapshots(current, [snapshot]));
				setError(null);
				await command.reset();
			} catch (reason) {
				setError(`Timecode Off failed: ${String(reason)}`);
			}
		},
		[api, command, showId],
	);

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

	useEffect(() => {
		if (!active) return;
		const openRequested = (request: {
			kind: "macro" | "timecode";
			objectId: string;
		}) => {
			if (request.kind !== "timecode") return;
			const timecode = objects.find(
				(candidate) => candidate.definition.id === request.objectId,
			);
			if (!timecode) return;
			setEditing(timecode);
			consumeObjectEditorRequest(request);
		};
		const current = currentObjectEditorRequest();
		if (current) openRequested(current);
		return subscribeObjectEditorRequest(openRequested);
	}, [active, objects]);

	if (editing) {
		return (
			<TimecodeEditor
				showId={showId}
				item={editing}
				api={api}
				snapshot={runtime.get(editing.definition.id)}
				cueLists={timelineCueLists}
				saveCueList={saveCueList}
				timingDefaults={{ sequenceFadeMillis, releaseFadeMillis }}
				audioPlayers={audioPlayers}
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
	const offPending = /^OFF$/iu.test(command.text.trim());
	return (
		<section className="timecode-window">
			{!compact && (
				<WindowHeader
					title="Timecode"
					info={{ primary: `${objects.length} Timecodes` }}
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
									item && offPending
										? `Turn off Timecode ${number} ${item.definition.name}`
										: item
											? `Timecode ${number} ${item.definition.name}`
											: `Empty Timecode ${number}`
								}
								model={{
									number,
									primary: item?.definition.name ?? "Empty",
									secondary:
										item && offPending
											? "Tap to stop Timecode"
											: snapshot
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
									workflow: item && offPending ? "Off" : undefined,
								}}
								className={
									item && offPending ? "command-target-off" : undefined
								}
								onClick={() => {
									const commandText = command.read().text.trim();
									if (item && /^OFF$/iu.test(commandText)) void stop(item);
									else if (item && /^ASSIGN$/i.test(commandText))
										void command.replace(`ASSIGN TIMECODE ${number}`);
									else if (item && /^SET$/i.test(commandText)) setEditing(item);
									else if (item) void start(item);
									else setEditing(newTimecode(number));
								}}
								onContextMenu={(event) => {
									event.preventDefault();
									if (item) setEditing(item);
								}}
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

/**
 * The audio a Timecode holds, named so an operator recognises it, with the way to replace it
 * beside it. The managed asset id names nothing anyone would know.
 */
/** How many frames of timeline an imported audio file occupies. */
function audioDurationFrames(
	imported: {
		sample_frames: number;
		sample_rate: number;
	},
	definition: TimecodeDefinition,
) {
	return Math.max(
		Math.ceil((imported.sample_frames * FPS) / imported.sample_rate),
		...definition.markers.map((marker) => marker.frame),
		...definition.lanes.flatMap((lane) =>
			"clips" in lane.content
				? lane.content.clips.map((clip) => clip.end_frame)
				: lane.content.keyframes.map((keyframe) => keyframe.frame),
		),
	);
}

function AudioFileField({
	audio,
	busy,
	importing,
	onChoose,
}: {
	audio: TimecodeDefinition["audio"];
	busy: boolean;
	importing: boolean;
	onChoose(file: File): Promise<void>;
}) {
	return (
		<div className="timecode-audio-import">
			<span>Audio file</span>
			<strong className="timecode-audio-file-name">
				{audio?.file_name ?? (audio ? "Managed audio" : "No audio file")}
			</strong>
			<RootConfinedFilePickerButton
				label={audio ? "Change Selected File" : "Select File"}
				allowedExtensions={["wav", "mp3"]}
				disabled={busy}
				onFiles={async ([file]) => file && onChoose(file)}
			/>
			<small>
				{importing
					? "Importing and normalizing…"
					: "WAV or MP3; MP3 is normalized to managed WAV."}
			</small>
		</div>
	);
}

export function TimecodeEditor({
	showId,
	item,
	api,
	snapshot,
	cueLists,
	audioPlayers,
	saveCueList,
	timingDefaults = { sequenceFadeMillis: 3_000, releaseFadeMillis: 3_000 },
	onClose,
}: {
	showId: string | null;
	item: TimecodeObjectRecord | NewTimecode;
	api: TimecodesApiClient;
	snapshot?: TimecodeTransportSnapshot;
	cueLists: TimecodeCueListOption[];
	audioPlayers: TimecodeAudioPlayerOption[];
	saveCueList?: SaveCueListTopology;
	timingDefaults?: {
		sequenceFadeMillis: number;
		releaseFadeMillis: number;
	};
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
	const {
		transport,
		editorFrame,
		scrub,
		acceptTransportResponse,
		beginTransportAction,
	} = useTimecodeEditorTransport(snapshot);
	const latestDraft = useRef(draft);
	latestDraft.current = draft;
	const [error, setError] = useState<string | null>(null);
	const [actionBusy, setActionBusy] = useState(false);
	const [audioImporting, setAudioImporting] = useState(false);
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
	const [settingsOpen, setSettingsOpen] = useState(false);
	const [markersLocked, setMarkersLocked] = useState(false);
	const [csvMode, setCsvMode] = useState<"append" | "replace">("append");
	const [csvError, setCsvError] = useState<string | null>(null);
	const timelineRef = useRef<TimecodeTimelineEditorHandle>(null);
	const cueTiming = useCueTimingWriter(cueLists, saveCueList ?? null);
	const effectiveCueLists = cueTiming.cueLists;
	useEffect(() => {
		const reconciled = reconcileAutomaticAudioLane(draft);
		if (reconciled !== draft) setDraft(reconciled);
	}, [draft, setDraft]);
	const {
		record,
		saving,
		saveError,
		retry: retrySave,
		flush,
	} = useTimecodeAutosave({ showId, item, draft, api });
	const isNew = !record;
	const {
		waveformPeaks,
		waveformError,
		waveformLoading,
		seedWaveform,
		retryWaveform,
	} = useTimecodeWaveform({
		showId,
		timecodeId: draft.id,
		audio: draft.audio,
		savedAudio: record?.definition.audio,
		api,
	});
	const duration = draft.duration_frame ?? 0;
	const frame = Math.min(editorFrame, duration);
	const busy = saving || actionBusy || audioImporting || cueTiming.saving;
	useEffect(() => {
		if (cueTiming.error) setError(cueTiming.error);
	}, [cueTiming.error]);
	const act = async (action: TimecodeTransportAction) => {
		if (!showId || !record) return;
		const interactionRevision = beginTransportAction();
		setActionBusy(true);
		setError(null);
		try {
			if (action.type !== "stop") await flush();
			const result = await api.transportAction(showId, draft.id, action);
			acceptTransportResponse(
				result,
				action.type === "seek",
				interactionRevision,
			);
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
			setDraft(
				reconcileAutomaticAudioLane({
					...latestDraft.current,
					duration_frame: audioDurationFrames(imported, latestDraft.current),
					// `file_name` is what the operator chose, so the lane and the settings can name it.
					audio: {
						asset_id: imported.asset_id,
						asset_revision: imported.asset_revision,
						file_name: file.name,
					},
				}),
			);
			seedWaveform(
				{
					asset_id: imported.asset_id,
					asset_revision: imported.asset_revision,
				},
				peaks,
			);
		} catch (reason) {
			setError(reason instanceof Error ? reason.message : String(reason));
		} finally {
			setAudioImporting(false);
		}
	};
	const close = async () => {
		try {
			await flush();
			await onClose();
		} catch (reason) {
			setError(`Could not close before autosave completed: ${String(reason)}`);
		}
	};
	const importCsv = async (file: File) => {
		try {
			const csvSource = await file.text();
			const imported = parseMarkerCsv(csvSource, FPS, Math.max(1, duration));
			setDraft({
				...latestDraft.current,
				markers:
					csvMode === "append"
						? [...latestDraft.current.markers, ...imported]
						: imported,
			});
			setCsvError(null);
		} catch (reason) {
			setCsvError(reason instanceof Error ? reason.message : String(reason));
		}
	};
	const transportActions = timecodeTransportActions({
		disabled: isNew || busy || Boolean(saveError),
		stopDisabled: isNew || actionBusy,
		state: transport?.state,
		onAction: act,
	});

	const addAction = useTimecodeAddAction({
		timelineRef,
		draft,
		cueLists: effectiveCueLists,
		audioPlayers,
		onError: setError,
	});
	return (
		<section className="timecode-window timecode-editor" aria-busy={busy}>
			<WindowHeader
				title={`Timecode ${draft.number}`}
				info={{
					primary: transport?.state ?? "Stopped",
					secondary: audioImporting
						? "Importing audio…"
						: saving
							? record
								? "Saving…"
								: "Creating…"
							: saveError
								? "Not saved"
								: waveformLoading
									? "Loading waveform…"
									: record
										? "Saved"
										: "Not saved",
				}}
				groups={[
					{ id: "timecode-add", actions: [addAction] },
					{
						id: "timecode-history",
						actions: [
							{
								id: "back",
								label: "Back",
								onPress: () => void close(),
								disabled: busy,
							},
							{
								id: "undo",
								label: "Undo",
								onPress: undo,
								disabled: !canUndo || busy,
							},
							{
								id: "redo",
								label: "Redo",
								onPress: redo,
								disabled: !canRedo || busy,
							},
						],
					},
					{
						id: "timecode-position",
						actions: [
							{
								id: "position",
								label: formatFrame(transport?.frame ?? 0),
								ariaLabel: "Timecode position",
								className: "timecode-position-action",
								onPress: () => void act({ type: "seek", frame }),
								disabled: isNew || busy,
							},
						],
					},
					{ id: "timecode-transport", actions: transportActions },
				]}
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
										markersLocked,
										setMarkersLocked,
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
			<TimecodeEditorFeedback
				savingError={saveError}
				waveformError={waveformError}
				busy={busy}
				onRetrySave={retrySave}
				onRetryWaveform={retryWaveform}
			/>
			<TimecodeTimelineEditor
				ref={timelineRef}
				definition={draft}
				frame={frame}
				fps={FPS}
				cueLists={effectiveCueLists}
				audioPlayers={audioPlayers}
				waveformPeaks={waveformPeaks}
				markersLocked={markersLocked}
				clipStatuses={transport?.cue_list_clips}
				onScrub={scrub}
				onCommit={setDraft}
				onPreview={previewDraft}
				onBeginGesture={beginGesture}
				onEndGesture={endGesture}
				timingDefaults={timingDefaults}
				onSaveCueList={cueTiming.save}
				onCueTimingError={setError}
			/>
		</section>
	);
}

/// The settings an operator edits, grouped so one pane does not ask for everything at once:
/// what the Timecode is, how it follows an external clock, and what its markers do.
const TIMECODE_SETTINGS_TABS = ["Generic", "Sync", "Markers"] as const;

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
	markersLocked,
	setMarkersLocked,
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
	markersLocked: boolean;
	setMarkersLocked(value: boolean): void;
}) {
	const [activeTab, setActiveTab] =
		useState<(typeof TIMECODE_SETTINGS_TABS)[number]>("Generic");

	return (
		<div className="timecode-settings-fields">
			<div
				className="timecode-settings-tabs"
				role="tablist"
				aria-label="Timecode settings"
			>
				{TIMECODE_SETTINGS_TABS.map((tab) => (
					<Button
						key={tab}
						role="tab"
						aria-selected={activeTab === tab}
						active={activeTab === tab}
						onClick={() => setActiveTab(tab)}
					>
						{tab}
					</Button>
				))}
			</div>
			{activeTab === "Generic" && (
				<>
					<TextField
						label="Name"
						value={draft.name}
						onChange={(event) =>
							setDraft({ ...draft, name: event.currentTarget.value })
						}
					/>
					<div className="timecode-duration-fields">
						<TimecodeFrameField
							label="Duration"
							value={duration}
							fps={FPS}
							minimum={1}
							onChange={(duration_frame) =>
								setDraft({ ...draft, duration_frame })
							}
						/>
					</div>
					<AudioFileField
						audio={draft.audio}
						busy={busy || audioImporting}
						importing={audioImporting}
						onChoose={importAudio}
					/>
				</>
			)}
			{activeTab === "Sync" && (
				<>
					<div className="timecode-duration-fields">
						<TimecodeFrameField
							label="Transport offset"
							value={draft.transport_offset_frame}
							fps={FPS}
							onChange={(transport_offset_frame) =>
								setDraft({ ...draft, transport_offset_frame })
							}
						/>
					</div>
					<CheckboxField
						className="timecode-checkbox"
						label="Arm"
						stateLabel="Start with external Timecode"
						checked={draft.auto_start}
						onChange={(event) =>
							setDraft({ ...draft, auto_start: event.currentTarget.checked })
						}
					/>
				</>
			)}
			{activeTab === "Markers" && (
				<>
					<CheckboxField
						className="timecode-checkbox"
						label="Lock markers"
						stateLabel="Prevent marker movement"
						checked={markersLocked}
						onChange={(event) => setMarkersLocked(event.currentTarget.checked)}
					/>
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
				</>
			)}
		</div>
	);
}

function useTimecodeAddAction({
	timelineRef,
	draft,
	cueLists,
	audioPlayers,
	onError,
}: {
	timelineRef: RefObject<TimecodeTimelineEditorHandle | null>;
	draft: TimecodeDefinition;
	cueLists: readonly unknown[];
	audioPlayers: readonly TimecodeAudioPlayerOption[];
	onError(message: string | null): void;
}) {
	const addLockedRef = useRef(false);
	const releaseTimerRef = useRef<number | null>(null);
	useEffect(
		() => () => {
			if (releaseTimerRef.current !== null)
				window.clearTimeout(releaseTimerRef.current);
		},
		[],
	);
	const runAdd = useCallback(
		(label: string, action: () => void) => {
			if (addLockedRef.current) return;
			addLockedRef.current = true;
			onError(null);
			try {
				action();
			} catch (reason) {
				onError(
					`Could not ${label}: ${
						reason instanceof Error ? reason.message : String(reason)
					}`,
				);
			} finally {
				releaseTimerRef.current = window.setTimeout(() => {
					addLockedRef.current = false;
					releaseTimerRef.current = null;
				}, 250);
			}
		},
		[onError],
	);
	return {
		id: "add",
		kind: "dropdown",
		className: "timecode-add-title-action",
		ariaLabel: "Add",
		label: (
			<>
				<span aria-hidden="true">＋</span> Add
			</>
		),
		dropdown: {
			kind: "items",
			ariaLabel: "Add",
			items: [
				{
					kind: "action",
					id: "marker",
					label: "Add Marker",
					onPress: () =>
						runAdd("add marker", () => timelineRef.current?.addMarker()),
				},
				...audioPlayers.map((player) => ({
					kind: "action" as const,
					id: `audio-player-${player.fixtureId}`,
					label: `Add ${player.name} Lane`,
					disabled: draft.lanes.some(
						(lane) =>
							lane.content.kind === "audio_player" &&
							lane.content.fixture_id === player.fixtureId,
					),
					onPress: () =>
						runAdd(`add ${player.name} lane`, () =>
							timelineRef.current?.addAudioPlayerLane(player.fixtureId),
						),
				})),
				{
					kind: "action",
					id: "speed",
					label: "Add Speed Lane",
					onPress: () =>
						runAdd("choose a speed group", () =>
							timelineRef.current?.chooseSpeedLane(),
						),
				},
				{
					kind: "action",
					id: "cuelist",
					label: "Add Cuelist Lane",
					disabled: !cueLists.length,
					onPress: () =>
						runAdd("open the cuelist chooser", () =>
							timelineRef.current?.chooseCueListLane(),
						),
				},
			],
		},
	} satisfies TitleAction;
}

/// Waveform buckets read from a file being imported.
///
/// Markers are aligned against the waveform, so it has to carry the same detail as the one the
/// desk serves for an already-linked file (`AUDIO_WAVEFORM_BUCKETS`); otherwise the lane loses
/// resolution until the show is reopened.
const IMPORTED_WAVEFORM_BUCKETS = 98_304;

async function decodeAudioPeaks(
	file: File,
	count = IMPORTED_WAVEFORM_BUCKETS,
): Promise<number[]> {
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
	return `${String(Math.floor(seconds / 3600)).padStart(2, "0")}:${String(Math.floor(seconds / 60) % 60).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}.${String(frame % FPS).padStart(2, "0")}`;
}

export function parseFrame(value: string): number | null {
	const match = /^(\d+):([0-5]\d):([0-5]\d)[.:](\d+)$/.exec(value.trim());
	if (!match) return null;
	const [, hours, minutes, seconds, frames] = match;
	const framePart = Number(frames);
	if (framePart >= FPS) return null;
	return (
		(Number(hours) * 60 * 60 + Number(minutes) * 60 + Number(seconds)) * FPS +
		framePart
	);
}
