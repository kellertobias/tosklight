import { useEffect, useState } from "react";
import {
	consumeObjectEditorRequest,
	currentObjectEditorRequest,
	subscribeObjectEditorRequest,
} from "../features/controlSurfaceInteraction/objectEditorRequest";
import { useSpeedGroupsBpm } from "../features/configuration/ConfigurationState";
import { usePlaybackDeskView } from "../features/playbackRuntime/PlaybackRuntimeView";
import { useCueListTopologyWriter } from "../features/playbackTopology/useCueListTopologyWriter";
import { useCueLists } from "../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../features/showObjects/ShowObjectsView";
import { useApp } from "../state/AppContext";
import { CuelistDetail } from "./cuelistWindow/CuelistDetail";
import { CuelistPool } from "./cuelistWindow/CuelistPool";
import { CuelistSettings } from "./cuelistWindow/CuelistSettings";
import { useCuelistPool } from "./cuelistWindow/useCuelistSelection";
import type { WindowProps } from "./windowTypes";

const DEFAULT_SPEED_GROUPS_BPM: [number, number, number, number, number] = [
	120, 90, 60, 30, 15,
];

export function CuelistWindow({
	active = true,
	builtIn = false,
	compact,
	cueListTab,
	showCueSidebar = true,
	cueListCompactRows = false,
	cueInformationBlock = "off",
	cueListSource = "fixed",
	fixedCueListNumber,
	fixedCueListId,
	viewOnly = false,
	paneId,
	thumbnails,
}: WindowProps & { thumbnails?: Record<number, string> }) {
	const saveCueList = useCueListTopologyWriter();
	const { state, dispatch } = useApp();
	const pool = useCuelistPool();
	const cueLists = useCueLists();
	const playbackDesk = usePlaybackDeskView(
		active && cueListSource === "follow-selection",
	);
	const [localTab, setLocalTab] = useState<"pool" | "cues">(
		cueListTab ?? "pool",
	);
	const [localSelectedCuelist, setLocalSelectedCuelist] = useState(1);
	const [settingsCuelist, setSettingsCuelist] = useState<number | null>(null);
	const [settingsOpen, setSettingsOpen] = useState(false);
	const [message, setMessage] = useState("");
	const tab = builtIn ? state.cuelistBuiltInView : localTab;
	useShowObjectView("group", active && tab !== "pool");
	useShowObjectView("cue_list", active);
	useShowObjectView("playback", active);
	useShowObjectView("playback_page", active && tab === "pool");
	const firstAvailableCuelist = pool[0]?.number ?? 1;
	const hasFixedCueListId = fixedCueListId !== undefined;
	const fixedDefinition = hasFixedCueListId
		? pool.find((definition) => definition.cueList.id === fixedCueListId)
		: undefined;
	const selectedByDesk = playbackDesk?.selected_cue_list
		? pool.find((entry) => entry.cueList.id === playbackDesk.selected_cue_list)
		: pool.find(
				(entry) => entry.assignment?.number === playbackDesk?.selected_playback,
			);
	const paneSelectedCuelist =
		cueListSource === "follow-selection"
			? (selectedByDesk?.canonicalNumber ?? null)
			: hasFixedCueListId
				? (fixedDefinition?.number ?? null)
				: (fixedCueListNumber ?? firstAvailableCuelist);
	const selectedCuelist = builtIn
		? (state.cuelistBuiltInNumber ?? firstAvailableCuelist)
		: cueListTab === "cues"
			? paneSelectedCuelist
			: localSelectedCuelist;
	const openCuelist = (number: number) => {
		if (builtIn) dispatch({ type: "OPEN_BUILTIN_CUELIST", number });
		else {
			setLocalSelectedCuelist(number);
			setLocalTab("cues");
		}
	};
	useEffect(() => {
		if (!active || !builtIn) return;
		const openRequested = (
			request: NonNullable<ReturnType<typeof currentObjectEditorRequest>>,
		) => {
			if (request.kind !== "cuelist") return;
			const entry = pool.find(
				(candidate) => candidate.cueList.id === request.objectId,
			);
			if (!entry) return;
			dispatch({ type: "OPEN_BUILTIN_CUELIST", number: entry.canonicalNumber });
			consumeObjectEditorRequest(request);
		};
		const request = currentObjectEditorRequest();
		if (request) openRequested(request);
		return subscribeObjectEditorRequest(openRequested);
	}, [active, builtIn, dispatch, pool]);
	const openPool = () => {
		if (builtIn) dispatch({ type: "SET_BUILTIN_CUELIST_VIEW", value: "pool" });
		else setLocalTab("pool");
	};
	const openSettings = (number: number | null) => {
		setSettingsCuelist(number);
		setSettingsOpen(true);
	};
	const settingsDefinition = pool.find(
		(definition) => definition.number === settingsCuelist,
	);
	const speedGroupsBpm = useSpeedGroupsBpm() ?? DEFAULT_SPEED_GROUPS_BPM;
	const settingsCueListId = settingsDefinition?.cueList.id ?? null;
	const settingsCueObject = settingsCueListId
		? cueLists.find((candidate) => candidate.body.id === settingsCueListId)
		: undefined;
	const settings = settingsOpen && settingsCueObject && (
		<CuelistSettings
			object={settingsCueObject}
			speedGroupsBpm={speedGroupsBpm}
			close={() => setSettingsOpen(false)}
			save={saveCueList}
		/>
	);
	if (tab === "pool")
		return (
			<CuelistPool
				active={active}
				compact={compact}
				builtIn={builtIn}
				selectedCuelist={selectedCuelist}
				message={message}
				onMessage={setMessage}
				onOpenCuelist={openCuelist}
				onSelectLocalCuelist={setLocalSelectedCuelist}
				onOpenSettings={openSettings}
				settings={settings}
				paneId={paneId}
			/>
		);
	return (
		<CuelistDetail
			active={active}
			compact={compact}
			cueListTab={cueListTab}
			cueListSource={cueListSource}
			showCueSidebar={showCueSidebar}
			compactRows={cueListCompactRows}
			cueInformationBlock={cueInformationBlock}
			selectedCuelist={selectedCuelist}
			settingsOpen={settingsOpen}
			settings={settings}
			onOpenPool={openPool}
			onOpenSettings={() => openSettings(selectedCuelist)}
			thumbnails={thumbnails}
			fixedCueListId={fixedCueListId}
			assignmentNumber={
				cueListSource === "follow-selection"
					? playbackDesk?.selected_cue_list
						? null
						: (playbackDesk?.selected_playback ?? undefined)
					: undefined
			}
			viewOnly={viewOnly}
		/>
	);
}
