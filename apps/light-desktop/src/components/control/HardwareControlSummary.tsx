import { Button } from "@tosklight/ui";
import { ModalNumberEditor } from "@tosklight/ui/input";
import { type CSSProperties, useRef, useState } from "react";
import { useConfigurationActions } from "../../features/configuration/ConfigurationActionsProvider";
import {
	useProgrammerFadeMillis,
	useSequenceMasterFadeMillis,
	useSpeedGroupsBpm,
} from "../../features/configuration/ConfigurationState";
import {
	useHighlightActions,
	useHighlightErrorMessage,
} from "../../features/highlight/HighlightState";
import {
	usePlaybackDeskView,
	usePlaybackRuntimeActions,
	usePlaybackRuntimeStatus,
} from "../../features/playbackRuntime/PlaybackRuntimeView";
import { usePlaybackTopologyActions } from "../../features/playbackTopology/PlaybackTopologyProvider";
import { usePlaybackPagesView } from "../../features/playbackTopology/PlaybackTopologyView";
import type { ShowObject } from "../../features/showObjects/contracts";
import { useApp } from "../../state/AppContext";
import { HighlightErrorAlert } from "./HighlightControls";
import {
	PlaybackPageMenu,
	PlaybackPageRenameDialog,
} from "./PlaybackPageDialogs";

function HardwareTimeInputModal({
	kind,
	onChange,
	onClose,
	onSubmit,
	value,
}: {
	kind: "prog" | "cue";
	onChange: (value: string) => void;
	onClose: () => void;
	onSubmit: () => void;
	value: string;
}) {
	return (
		<ModalNumberEditor
			ariaLabel={`${kind === "prog" ? "Programmer" : "Cue"} fade value`}
			title={kind === "prog" ? "Prog. Fade" : "Cue Fade"}
			value={value}
			onChange={onChange}
			onSubmit={onSubmit}
			onClose={onClose}
		/>
	);
}

export function HardwareControlSummary() {
	const highlightError = useHighlightErrorMessage();
	const highlightActions = useHighlightActions();
	const configurationActions = useConfigurationActions();
	const { state, dispatch } = useApp();
	const [pagesOpen, setPagesOpen] = useState(false);
	const [renamePage, setRenamePage] =
		useState<ShowObject<"playback_page"> | null>(null);
	const [timeInput, setTimeInput] = useState<"prog" | "cue" | null>(null);
	const [inputValue, setInputValue] = useState("");
	const taps = useRef<Record<string, number[]>>({});
	const playbackDesk = usePlaybackDeskView();
	const runtimeActions = usePlaybackRuntimeActions();
	const runtimeStatus = usePlaybackRuntimeStatus();
	const topology = usePlaybackPagesView();
	const topologyActions = usePlaybackTopologyActions();
	const bpms = useSpeedGroupsBpm() ?? [120, 90, 60, 30, 15];
	const prog = (useProgrammerFadeMillis() ?? 3000) / 1000;
	const cue = (useSequenceMasterFadeMillis() ?? 3000) / 1000;
	const runtimeReady = runtimeStatus.status === "ready";
	const page = runtimeReady ? (playbackDesk?.active_page ?? null) : null;
	const openTime = (kind: "prog" | "cue", value: number) => {
		const next = String(Number(value.toFixed(1)));
		setTimeInput(kind);
		setInputValue(next);
	};
	const activePage =
		topology.pages.find((item) => item.body.number === page) ?? null;
	const pageReady =
		topology.ready &&
		activePage !== null &&
		runtimeActions !== null &&
		topologyActions !== null;
	const openPagesOrRename = () => {
		if (!pageReady) return;
		if (state.playbackSetArmed && activePage) {
			dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
			setRenamePage(activePage);
		} else setPagesOpen(true);
	};
	const submitTime = () => {
		const value = Math.max(
			0,
			Math.min(timeInput === "prog" ? 20 : 60, Number(inputValue)),
		);
		if (Number.isFinite(value))
			void configurationActions?.setControlTiming(
				timeInput === "prog"
					? { programmer_fade_millis: Math.round(value * 1000) }
					: { sequence_master_fade_millis: Math.round(value * 1000) },
			);
		setTimeInput(null);
	};
	const tap = (group: string, index: number) => {
		const now = performance.now();
		const recent = [...(taps.current[group] ?? []), now]
			.filter((time) => now - time < 3000)
			.slice(-6);
		taps.current[group] = recent;
		if (recent.length < 2) return;
		const intervals = recent
			.slice(1)
			.map((time, offset) => time - recent[offset]);
		const values = [...bpms] as [number, number, number, number, number];
		values[index] = Math.round(
			60000 /
				(intervals.reduce((sum, value) => sum + value, 0) / intervals.length),
		);
		void configurationActions?.setControlTiming({ speed_groups_bpm: values });
	};
	return (
		<div className="hardware-control-summary">
			<div className="hardware-values">
				<Button onClick={() => openTime("prog", prog)}>
					<small>Prog Fade</small>
					<b>{prog.toFixed(1)}s</b>
				</Button>
				<Button onClick={() => openTime("cue", cue)}>
					<small>Cue Fade</small>
					<b>{cue.toFixed(1)}s</b>
				</Button>
				<Button
					aria-label={pageReady ? `Page ${page}` : "Playback page loading"}
					disabled={!pageReady}
					onClick={openPagesOrRename}
				>
					<small>Page</small>
					<b>{page ?? "—"}</b>
				</Button>
			</div>
			<div className="hardware-speed-groups">
				{(["A", "B", "C", "D", "E"] as const).map((group, index) => (
					<Button
						style={{ "--bpm": bpms[index] } as CSSProperties}
						key={group}
						onClick={() => tap(group, index)}
					>
						<b>{group}</b>
						<span>{bpms[index]} BPM</span>
					</Button>
				))}
			</div>
			<HighlightErrorAlert
				message={highlightError}
				onDismiss={() => highlightActions?.dismissHighlightError()}
			/>
			{timeInput && (
				<HardwareTimeInputModal
					kind={timeInput}
					value={inputValue}
					onChange={setInputValue}
					onSubmit={submitTime}
					onClose={() => setTimeInput(null)}
				/>
			)}
			<PlaybackPageMenu open={pagesOpen} onClose={() => setPagesOpen(false)} />
			<PlaybackPageRenameDialog
				page={renamePage}
				onClose={() => setRenamePage(null)}
			/>
		</div>
	);
}
