import { Button, ErrorAlert, SwitchField, TextInput } from "@tosklight/ui";
import { type ReactNode, useMemo, useState } from "react";
import type { TimecodeAudioAnalysisSnapshot } from "../../api/client/timecodes";
import type { TimecodeDefinition } from "../../api/types/timecode";
import { SongAnalysisLaneControlProvider, type TimecodeBeatGrid, TimecodeBeatGridProvider } from "./TimecodeBeatGrid";
import { useTimecodeAudioAnalysis } from "./useTimecodeAudioAnalysis";
import "./timecodeBeatGrid.css";

interface Analysis {
	snapshot: TimecodeAudioAnalysisSnapshot | null;
	error: string | null;
	available: boolean;
	start(): void;
	cancel(): void;
}

interface Suggestion {
	frame: number;
	label: string;
	key: string;
}

const formatTime = (frame: number, fps: number) => {
	const seconds = frame / fps;
	return `${Math.floor(seconds / 60)}:${(seconds % 60).toFixed(1).padStart(4, "0")}`;
};

/**
 * Song analysis for a Timecode: run it, see its progress, and turn suggested sections into
 * markers. Suggestions stay suggestions — drawn dashed, never stored — until accepted; an
 * accepted or discarded suggestion is not offered again, and a re-run keeps accepted markers.
 */
export function useTimecodeSongAnalysisPanel({
	analysis,
	definition,
	fps,
	onCommit,
}: {
	analysis: Analysis;
	definition: TimecodeDefinition;
	fps: number;
	onCommit(definition: TimecodeDefinition): void;
}): { panel: ReactNode; beatGrid: TimecodeBeatGrid | undefined } {
	const [names, setNames] = useState<Record<string, string>>({});
	const [discarded, setDiscarded] = useState<ReadonlySet<string>>(new Set());
	const [showBeats, setShowBeats] = useState(true);
	const [snap, setSnap] = useState(true);
	const result = analysis.snapshot?.state === "ready" ? analysis.snapshot.analysis : null;
	const suggestions = useMemo<Suggestion[]>(() => {
		if (!result) return [];
		// A marker within a beat of a suggestion already covers it.
		const tolerance = result.bpm > 0 ? Math.round((60 / result.bpm) * fps / 2) : fps;
		return result.sections
			.map((section) => ({ frame: section.frame, label: section.label, key: `${section.frame}` }))
			.filter(
				(section) =>
					!discarded.has(section.key) &&
					!definition.markers.some((marker) => Math.abs(marker.frame - section.frame) <= tolerance),
			);
	}, [definition.markers, discarded, fps, result]);
	const nameOf = (suggestion: Suggestion) => names[suggestion.key] ?? suggestion.label;
	const accept = (chosen: readonly Suggestion[]) => {
		if (!chosen.length) return;
		onCommit({
			...definition,
			markers: [
				...definition.markers,
				...chosen.map((suggestion) => ({
					id: crypto.randomUUID(),
					frame: suggestion.frame,
					name: nameOf(suggestion).trim() || suggestion.label,
					color: null,
				})),
			].sort((left, right) => left.frame - right.frame),
		});
	};
	const beatGrid = result
		? {
				beatFrames: result.beat_frames,
				downbeatFrames: result.downbeat_frames,
				suggestions: suggestions.map((suggestion) => ({ frame: suggestion.frame, label: nameOf(suggestion) })),
				snap: snap && result.beat_frames.length > 0,
				showBeats,
			}
		: undefined;
	if (!analysis.available) return { panel: null, beatGrid: undefined };
	const state = analysis.snapshot?.state ?? "idle";
	const panel = (
		<section className="timecode-song-analysis" aria-label="Song analysis">
			<header>
				<strong>Song analysis</strong>
				{state === "running" ? (
					<>
						<progress
							aria-label="Analysing audio"
							max={1}
							value={analysis.snapshot?.progress ?? 0}
						/>
						<span role="status">Analysing audio… {Math.round((analysis.snapshot?.progress ?? 0) * 100)}%</span>
						<Button onClick={analysis.cancel}>Cancel</Button>
					</>
				) : result ? (
					<>
						<span role="status">
							{result.bpm > 0 ? `${result.bpm.toFixed(1)} BPM` : "No steady beat"} ·{" "}
							{suggestions.length} suggested section{suggestions.length === 1 ? "" : "s"}
						</span>
						<SwitchField bare label="Beat grid" checked={showBeats} onChange={(event) => setShowBeats(event.target.checked)} />
						<SwitchField
							bare
							label="Snap to beats"
							checked={snap}
							disabled={!result.beat_frames.length}
							onChange={(event) => setSnap(event.target.checked)}
						/>
						<Button disabled={!suggestions.length} onClick={() => accept(suggestions)}>
							Accept All
						</Button>
					</>
				) : (
					<>
						<span role="status">
							{state === "cancelled"
								? "Analysis cancelled."
								: state === "failed"
									? "Analysis failed."
									: "Find the beat grid and song sections of the linked audio."}
						</span>
						<Button variant="primary" onClick={analysis.start}>
							{state === "idle" ? "Analyse Audio" : "Analyse Again"}
						</Button>
					</>
				)}
			</header>
			{(analysis.error || analysis.snapshot?.error) && (
				<ErrorAlert as="p" role="alert">
					{analysis.error ?? analysis.snapshot?.error}
				</ErrorAlert>
			)}
			{result && suggestions.length > 0 && (
				<ul className="timecode-section-suggestions" aria-label="Suggested sections">
					{suggestions.map((suggestion) => (
						<li key={suggestion.key}>
							<span className="timecode-section-time">{formatTime(suggestion.frame, fps)}</span>
							<TextInput
								aria-label={`Name for section at ${formatTime(suggestion.frame, fps)}`}
								value={nameOf(suggestion)}
								onChange={(event) => setNames((current) => ({ ...current, [suggestion.key]: event.target.value }))}
							/>
							<Button aria-label={`Accept ${nameOf(suggestion)} at ${formatTime(suggestion.frame, fps)}`} onClick={() => accept([suggestion])}>
								Accept
							</Button>
							<Button
								aria-label={`Discard ${nameOf(suggestion)} at ${formatTime(suggestion.frame, fps)}`}
								onClick={() => setDiscarded((current) => new Set(current).add(suggestion.key))}
							>
								Discard
							</Button>
						</li>
					))}
				</ul>
			)}
		</section>
	);
	return { panel, beatGrid };
}

/** The song analysis strip above the timeline, with its beat grid supplied to the timeline inside. */
export function TimecodeSongAnalysis({
	showId,
	api,
	draft,
	record,
	setDraft,
	children,
}: {
	showId: string | null;
	api: Parameters<typeof useTimecodeAudioAnalysis>[0]["api"];
	draft: TimecodeDefinition;
	record: { definition: TimecodeDefinition } | null | undefined;
	setDraft(definition: TimecodeDefinition): void;
	children: ReactNode;
}) {
	const analysis = useTimecodeAudioAnalysis({ showId, timecodeId: draft.id, savedAudio: record?.definition.audio, api });
	const { panel, beatGrid } = useTimecodeSongAnalysisPanel({ analysis, definition: draft, fps: TIMECODE_ANALYSIS_FPS, onCommit: setDraft });
	// The strip opens while analysing and when asked for; otherwise the timeline keeps its room.
	const [open, setOpen] = useState(false);
	const state = analysis.snapshot?.state ?? "idle";
	const analysed = state === "ready";
	const laneControl = analysis.available
		? {
				label: state === "running" ? "Analysing…" : analysed ? (open ? "Hide sections" : "Sections") : "Analyse",
				ariaLabel: analysed ? (open ? "Hide song analysis" : "Show song analysis") : "Analyse song",
				onPress: () => {
					if (!analysed && state !== "running") analysis.start();
					setOpen(!(analysed && open));
				},
			}
		: undefined;
	return (
		<>
			{(open || state === "running") && panel}
			<SongAnalysisLaneControlProvider value={laneControl}>
				<TimecodeBeatGridProvider value={beatGrid}>{children}</TimecodeBeatGridProvider>
			</SongAnalysisLaneControlProvider>
		</>
	);
}

/** Timecode frames per second, the unit the analysis reports in. */
const TIMECODE_ANALYSIS_FPS = 44;
