import { HardwareControlSummaryView } from "@tosklight/ui/command";
import { ModalNumberEditor } from "@tosklight/ui/input";
import { type ReactNode, useEffect, useRef, useState } from "react";
import { useConfigurationActions } from "../../features/configuration/ConfigurationActionsProvider";
import {
	useProgrammerFadeMillis,
	useReleaseFadeMillis,
	useSequenceMasterFadeMillis,
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
import { useSpeedGroupRuntimeView } from "../../features/speedGroupRuntime/SpeedGroupRuntimeView";
import { useApp } from "../../state/AppContext";
import { KeyboardPageActions } from "./commandLine/playbackShortcutKeys";
import { HighlightErrorAlert } from "./HighlightControls";
import {
	PlaybackPageMenu,
	PlaybackPageRenameDialog,
} from "./PlaybackPageDialogs";
import { formatSpeedGroupBpm } from "./speedGroupFormatting";
import { useSpeedGroupInteraction } from "./useSpeedGroupInteraction";

type HardwareTimeKind = "prog" | "cue" | "release";

function controlTimingPatch(kind: HardwareTimeKind | null, input: string) {
	const seconds = Math.max(
		0,
		Math.min(kind === "prog" ? 20 : 60, Number(input)),
	);
	if (!Number.isFinite(seconds)) return null;
	const millis = Math.round(seconds * 1000);
	return kind === "prog"
		? { programmer_fade_millis: millis }
		: kind === "cue"
			? { sequence_master_fade_millis: millis }
			: { release_fade_millis: millis };
}

function HardwareTimeInputModal({
	kind,
	onChange,
	onClose,
	onSubmit,
	value,
}: {
	kind: HardwareTimeKind;
	onChange: (value: string) => void;
	onClose: () => void;
	onSubmit: () => void;
	value: string;
}) {
	return (
		<ModalNumberEditor
			ariaLabel={`${kind === "prog" ? "Programmer" : kind === "cue" ? "Cue" : "Release"} fade value`}
			title={
				kind === "prog" ? "Prog. Fade" : kind === "cue" ? "Cue Fade" : "Release"
			}
			value={value}
			onChange={onChange}
			onSubmit={onSubmit}
			onClose={onClose}
		/>
	);
}

function useHardwarePageWindowEvents(
	hardwarePages: KeyboardPageActions,
	pageReady: boolean,
	page: number | null,
	pages: readonly ShowObject<"playback_page">[],
	onMenu: () => void,
) {
	useEffect(() => {
		const step = (event: Event) => {
			if (!pageReady) return;
			hardwarePages.step(
				{ activePage: page, pages: pages.map((item) => item.body) },
				(event as CustomEvent<number>).detail > 0 ? 1 : -1,
			);
		};
		const menu = () => {
			if (pageReady) onMenu();
		};
		window.addEventListener("light:playback-page-step", step);
		window.addEventListener("light:playback-page-menu", menu);
		return () => {
			window.removeEventListener("light:playback-page-step", step);
			window.removeEventListener("light:playback-page-menu", menu);
		};
	}, [hardwarePages, onMenu, page, pageReady, pages]);
}

function HardwareControlOverlays({
	highlightError,
	onDismissHighlight,
	timeInput,
	inputValue,
	onInputValue,
	onSubmitTime,
	onCloseTime,
	pagesOpen,
	onClosePages,
	renamePage,
	onCloseRename,
	speedGroupSettings,
}: {
	highlightError: string | null;
	onDismissHighlight: () => void;
	timeInput: HardwareTimeKind | null;
	inputValue: string;
	onInputValue: (value: string) => void;
	onSubmitTime: () => void;
	onCloseTime: () => void;
	pagesOpen: boolean;
	onClosePages: () => void;
	renamePage: ShowObject<"playback_page"> | null;
	onCloseRename: () => void;
	speedGroupSettings: ReactNode;
}) {
	return (
		<>
			<HighlightErrorAlert
				message={highlightError}
				onDismiss={onDismissHighlight}
			/>
			{timeInput && (
				<HardwareTimeInputModal
					kind={timeInput}
					value={inputValue}
					onChange={onInputValue}
					onSubmit={onSubmitTime}
					onClose={onCloseTime}
				/>
			)}
			<PlaybackPageMenu open={pagesOpen} onClose={onClosePages} />
			<PlaybackPageRenameDialog page={renamePage} onClose={onCloseRename} />
			{speedGroupSettings}
		</>
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
	const [timeInput, setTimeInput] = useState<HardwareTimeKind | null>(null);
	const [inputValue, setInputValue] = useState("");
	const speedGroupInteraction = useSpeedGroupInteraction();
	const playbackDesk = usePlaybackDeskView();
	const runtimeActions = usePlaybackRuntimeActions();
	const runtimeStatus = usePlaybackRuntimeStatus();
	const topology = usePlaybackPagesView();
	const topologyActions = usePlaybackTopologyActions();
	const speedGroups = useSpeedGroupRuntimeView();
	const prog = (useProgrammerFadeMillis() ?? 3000) / 1000;
	const cue = (useSequenceMasterFadeMillis() ?? 3000) / 1000;
	const release = (useReleaseFadeMillis() ?? 3000) / 1000;
	const runtimeReady = runtimeStatus.status === "ready";
	const page = runtimeReady ? (playbackDesk?.active_page ?? null) : null;
	const openTime = (kind: HardwareTimeKind, value: number) => {
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
	const hardwarePages = useRef(new KeyboardPageActions()).current;
	useEffect(() => {
		hardwarePages.syncAuthority(
			pageReady ? (topologyActions?.createPage ?? null) : null,
			pageReady ? (runtimeActions?.setActivePage ?? null) : null,
		);
		return () => hardwarePages.invalidate();
	}, [
		hardwarePages,
		pageReady,
		topologyActions?.createPage,
		runtimeActions?.setActivePage,
	]);
	useHardwarePageWindowEvents(
		hardwarePages,
		pageReady,
		page,
		topology.pages,
		() => setPagesOpen(true),
	);
	const openPagesOrRename = () => {
		if (!pageReady) return;
		if (state.playbackSetArmed && activePage) {
			dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
			setRenamePage(activePage);
		} else setPagesOpen(true);
	};
	const openPageRename = () => {
		if (!pageReady || !activePage) return;
		dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
		setRenamePage(activePage);
	};
	const submitTime = () => {
		const patch = controlTimingPatch(timeInput, inputValue);
		if (patch) void configurationActions?.setControlTiming(patch);
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
					id: "release-fade",
					label: "Release",
					display: `${release.toFixed(1)}s`,
				},
				{
					id: "page",
					label: "Page",
					display: String(page ?? "—"),
					disabled: !pageReady,
					ariaLabel: pageReady ? `Page ${page}` : "Playback page loading",
					settings: true,
				},
			]}
			speedGroups={(["A", "B", "C", "D", "E"] as const).map((group, index) => {
				const bpm = speedGroups.ready
					? speedGroups.projection?.groups[index]?.manualBpm
					: undefined;
				return {
					id: group,
					bpm,
					display: bpm === undefined ? "—" : formatSpeedGroupBpm(bpm),
				};
			})}
			onValue={(id) => {
				if (id === "programmer-fade") openTime("prog", prog);
				else if (id === "cue-fade") openTime("cue", cue);
				else if (id === "release-fade") openTime("release", release);
				else openPagesOrRename();
			}}
			onValueSettings={(id) => {
				if (id === "page") openPageRename();
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
				<HardwareControlOverlays
					highlightError={highlightError}
					onDismissHighlight={() => highlightActions?.dismissHighlightError()}
					timeInput={timeInput}
					inputValue={inputValue}
					onInputValue={setInputValue}
					onSubmitTime={submitTime}
					onCloseTime={() => setTimeInput(null)}
					pagesOpen={pagesOpen}
					onClosePages={() => setPagesOpen(false)}
					renamePage={renamePage}
					onCloseRename={() => setRenamePage(null)}
					speedGroupSettings={speedGroupInteraction.settings}
				/>
			}
		/>
	);
}
