import { WindowHeader } from "../../components/window-kit";
import type { WindowProps } from "../windowTypes";
import { CueProperties } from "./CueProperties";
import { CueTable, type CueTableEmptyState } from "./CueTable";
import { useCueEditor } from "./useCueEditor";
import { useSelectedCuelist } from "./useCuelistSelection";
import { useCueThumbnails } from "./useCueThumbnails";

function emptyState(
	cueListAvailable: boolean,
	cueListTab: WindowProps["cueListTab"],
	cueListSource: WindowProps["cueListSource"],
	selectedCuelist: number | null,
	selectedPlaybackExists: boolean,
): CueTableEmptyState {
	if (cueListAvailable)
		return {
			title: "This Cuelist has no Cues",
			description: "Record the first Cue to begin building this Cuelist.",
			icon: "▶",
		};
	if (cueListTab === "cues" && cueListSource === "follow-selection") {
		if (selectedCuelist == null)
			return {
				title: "No Cuelist selected",
				description: "Select a Cuelist playback and this pane will follow it.",
				icon: "◎",
			};
		if (selectedPlaybackExists)
			return {
				title: "Selected playback is not a Cuelist",
				description: "Select a Cuelist playback for this pane to follow.",
				icon: "◎",
			};
		return {
			title: "Selected Cuelist is unavailable",
			description:
				"The selected playback no longer exists in the playback pool.",
			icon: "◎",
		};
	}
	return {
		title: "Fixed Cuelist is unavailable",
		description: "Choose an available Cuelist in this pane's settings.",
		icon: "◎",
	};
}

interface CuelistDetailProps {
	active: boolean;
	compact?: boolean;
	cueListTab: WindowProps["cueListTab"];
	cueListSource: WindowProps["cueListSource"];
	showCueSidebar: boolean;
	selectedCuelist: number | null;
	settingsOpen: boolean;
	settings: React.ReactNode;
	onOpenPool: () => void;
	onOpenSettings: () => void;
}

export function CuelistDetail(props: CuelistDetailProps) {
	const selection = useSelectedCuelist(props.selectedCuelist, props.active);
	const cues = selection.cueList?.cues ?? [];
	const editor = useCueEditor({
		cues,
		selectedCueObject: selection.selectedCueObject,
		activeCueIndex: selection.active?.cue_index,
		followActiveCue:
			props.cueListTab === "cues" && props.cueListSource === "follow-selection",
	});
	const thumbnails = useCueThumbnails(cues);
	const showProperties =
		props.showCueSidebar && (!props.compact || props.cueListTab === "cues");
	return (
		<div className="cuelist-window">
			{!props.compact && (
				<WindowHeader
					title={`Cuelist View · Cuelist ${props.selectedCuelist}${selection.cueList?.name ? ` · ${selection.cueList.name}` : ""}`}
					info={{
						primary: selection.active ? "Running" : "Ready",
						secondary: `Revision ${selection.selectedCueObject?.revision ?? 0}${selection.cueList ? ` · ${selection.cueList.mode} · priority ${selection.cueList.priority}` : ""}`,
					}}
					actions={[
						[
							{
								id: "pool",
								label: "← Cuelist Pool",
								onClick: props.onOpenPool,
							},
							{
								id: "settings",
								label: "Cuelist Settings",
								onClick: props.onOpenSettings,
							},
						],
					]}
				/>
			)}
			<div
				className={`sequence-layout ${showProperties ? "with-cue-properties" : ""}`}
			>
				<CueTable
					cues={cues}
					active={selection.active}
					selectedCue={editor.selectedCue}
					settingsOpen={props.settingsOpen}
					thumbnails={thumbnails}
					emptyState={emptyState(
						Boolean(selection.cueList),
						props.cueListTab,
						props.cueListSource,
						props.selectedCuelist,
						Boolean(selection.selectedPlaybackDefinition),
					)}
					onSelectCue={editor.setSelectedCue}
				/>
				{showProperties && editor.cueDraft && (
					<CueProperties
						actions={{
							draft: editor.cueDraft,
							setDraft: editor.setCueDraft,
							save: editor.saveCue,
						}}
						thumbnail={thumbnails[editor.selectedCue]}
						editError={editor.cueEditError}
						active={true}
						layoutDependencies={[
							props.compact,
							editor.cueDraft.id,
							editor.cueDraft.number,
							editor.cueDraft.trigger.type,
							props.cueListTab,
						]}
					/>
				)}
			</div>
			{props.settings}
		</div>
	);
}
