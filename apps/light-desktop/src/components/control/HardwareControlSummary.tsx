import { HardwareControlSummaryView } from "@tosklight/ui/command";
import { ModalNumberEditor } from "@tosklight/ui/input";
import { useState } from "react";
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
import { formatSpeedGroupBpm } from "./speedGroupFormatting";
import { useSpeedGroupInteraction } from "./useSpeedGroupInteraction";

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
	const speedGroupInteraction = useSpeedGroupInteraction();
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
	return (
		<HardwareControlSummaryView
			values={[
				{
					id: "programmer-fade",
					label: "Prog Fade",
					display: `${prog.toFixed(1)}s`,
				},
				{
					id: "cue-fade",
					label: "Cue Fade",
					display: `${cue.toFixed(1)}s`,
				},
				{
					id: "page",
					label: "Page",
					display: String(page ?? "—"),
					disabled: !pageReady,
					ariaLabel: pageReady ? `Page ${page}` : "Playback page loading",
				},
			]}
			speedGroups={(["A", "B", "C", "D", "E"] as const).map((group, index) => ({
				id: group,
				bpm: bpms[index],
				display: formatSpeedGroupBpm(bpms[index]),
			}))}
			onValue={(id) => {
				if (id === "programmer-fade") openTime("prog", prog);
				else if (id === "cue-fade") openTime("cue", cue);
				else openPagesOrRename();
			}}
			onSpeedPointerDown={(group, event) =>
				speedGroupInteraction.beginHold(
					group,
					event.shiftKey || state.shiftArmed,
				)
			}
			onSpeedPointerEnd={speedGroupInteraction.endHold}
			onSpeedActivate={(group, event) =>
				speedGroupInteraction.activate(group, event.shiftKey)
			}
			onSpeedSettings={speedGroupInteraction.openSettings}
			overlays={
				<>
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
					<PlaybackPageMenu
						open={pagesOpen}
						onClose={() => setPagesOpen(false)}
					/>
					<PlaybackPageRenameDialog
						page={renamePage}
						onClose={() => setRenamePage(null)}
					/>
					{speedGroupInteraction.settings}
				</>
			}
		/>
	);
}
