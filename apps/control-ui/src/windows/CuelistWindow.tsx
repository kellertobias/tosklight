import { useState } from "react";
import { useServer } from "../api/ServerContext";
import { useApp } from "../state/AppContext";
import { CuelistDetail } from "./cuelistWindow/CuelistDetail";
import { CuelistPool } from "./cuelistWindow/CuelistPool";
import { CuelistSettings } from "./cuelistWindow/CuelistSettings";
import { useCuelistPool } from "./cuelistWindow/useCuelistSelection";
import type { WindowProps } from "./windowTypes";
import { useShowObjectView } from "../features/showObjects/ShowObjectsView";

export function CuelistWindow({
	active = true,
	builtIn = false,
	compact,
	cueListTab,
	showCueSidebar = true,
	cueListSource = "fixed",
	fixedCueListNumber,
}: WindowProps) {
	const server = useServer();
	const { state, dispatch } = useApp();
	const pool = useCuelistPool();
	const [localTab, setLocalTab] = useState<"pool" | "cues">(
		cueListTab ?? "pool",
	);
	const [localSelectedCuelist, setLocalSelectedCuelist] = useState(1);
	const [settingsCuelist, setSettingsCuelist] = useState<number | null>(null);
	const [settingsOpen, setSettingsOpen] = useState(false);
	const [message, setMessage] = useState("");
	const tab = builtIn ? state.cuelistBuiltInView : localTab;
	useShowObjectView("group", active && tab !== "pool");
	const firstAvailableCuelist = pool[0]?.number ?? 1;
	const paneSelectedCuelist =
		cueListSource === "follow-selection"
			? (server.playbacks?.selected_playback ?? null)
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
	const settingsCueListId =
		settingsDefinition?.target.type === "cue_list"
			? settingsDefinition.target.cue_list_id
			: null;
	const settingsCueObject = settingsCueListId
		? server.cueObjects?.find((candidate) => candidate.id === settingsCueListId)
		: undefined;
	const settings = settingsOpen && settingsCueObject && (
		<CuelistSettings
			object={settingsCueObject}
			speedGroupsBpm={
				server.configuration?.speed_groups_bpm ?? [120, 90, 60, 30, 15]
			}
			close={() => setSettingsOpen(false)}
			save={server.saveCueList}
		/>
	);
	if (tab === "pool")
		return (
			<CuelistPool
				compact={compact}
				builtIn={builtIn}
				selectedCuelist={selectedCuelist}
				message={message}
				onMessage={setMessage}
				onOpenCuelist={openCuelist}
				onSelectLocalCuelist={setLocalSelectedCuelist}
				onOpenSettings={openSettings}
				settings={settings}
			/>
		);
	return (
		<CuelistDetail
			compact={compact}
			cueListTab={cueListTab}
			cueListSource={cueListSource}
			showCueSidebar={showCueSidebar}
			selectedCuelist={selectedCuelist}
			settingsOpen={settingsOpen}
			settings={settings}
			onOpenPool={openPool}
			onOpenSettings={() => openSettings(selectedCuelist)}
		/>
	);
}
