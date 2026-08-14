import { WindowHeader } from "@tosklight/ui/window-kit";
import { ModalFrame } from "@tosklight/ui/modals";
import { useState } from "react";
import { useCommandLineSurface } from "../../components/control/commandLine/useCommandLineSurface";
import type { WindowProps } from "../windowTypes";
import { CuePropertyModal } from "./CueProperties";
import {
	type CueEditableProperty,
	CueTable,
	type CueTableEmptyState,
} from "./CueTable";
import { useCueTimingProgress } from "./cueTimingProgress";
import { useCueEditor } from "./useCueEditor";
import { useSelectedCuelist } from "./useCuelistSelection";
import { useCueThumbnails } from "./useCueThumbnails";

function emptyState(
	cueListAvailable: boolean,
	cueListTab: WindowProps["cueListTab"],
	cueListSource: WindowProps["cueListSource"],
	selectedCuelist: number | null,
	selectedPlaybackExists: boolean,
	viewOnly: boolean,
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
		description: viewOnly
			? "The configured Cuelist is missing or unavailable."
			: "Choose an available Cuelist in this pane's settings.",
		icon: "◎",
	};
}

interface CuelistDetailProps {
	active: boolean;
	compact?: boolean;
	cueListTab: WindowProps["cueListTab"];
	cueListSource: WindowProps["cueListSource"];
	showCueSidebar: boolean;
	compactRows: boolean;
	cueInformationBlock: "off" | "current" | "next";
	selectedCuelist: number | null;
	settingsOpen: boolean;
	settings: React.ReactNode;
	onOpenPool: () => void;
	onOpenSettings: () => void;
	thumbnails?: Record<number, string>;
	fixedCueListId?: string;
	viewOnly?: boolean;
}

export function CuelistDetail(props: CuelistDetailProps) {
	const [propertyEditor, setPropertyEditor] = useState<{
		index: number;
		property: CueEditableProperty;
	} | null>(null);
	const [previewCue, setPreviewCue] = useState<number | null>(null);
	const selection = useSelectedCuelist(
		props.selectedCuelist,
		props.active,
		props.fixedCueListId,
	);
	const cues = selection.cueList?.cues ?? [];
	const editor = useCueEditor({
		cues,
		selectedCueObject: selection.selectedCueObject,
		activeCueIndex: selection.active?.cue_index,
		followActiveCue:
			props.cueListTab === "cues" && props.cueListSource === "follow-selection",
	});
	const generatedThumbnails = useCueThumbnails(cues, props.active);
	const thumbnails = props.thumbnails ?? generatedThumbnails;
	const timingProgressByRow = useCueTimingProgress(cues, selection.active);
	const command = useCommandLineSurface({
		enabled: props.active && !props.viewOnly,
		observeCommand: true,
	});
	const informationCue =
		props.cueInformationBlock === "current"
			? cues[selection.active?.cue_index ?? -1]
			: props.cueInformationBlock === "next"
				? cues.find(
						(cue) =>
							cue.number === selection.active?.effective_next_cue_number,
					)
				: undefined;
	const informationLabel =
		props.cueInformationBlock === "current" ? "Current Cue" : "Next Cue";
	return (
		<div className="cuelist-window">
			{!props.compact && (
				<WindowHeader
					title={`Cuelist View · Cuelist ${props.selectedCuelist}${selection.cueList?.name ? ` · ${selection.cueList.name}` : ""}`}
					info={{
						primary: selection.active ? "Running" : "Ready",
						secondary: `Revision ${selection.selectedCueObject?.revision ?? 0}${selection.cueList ? ` · ${selection.cueList.mode} · priority ${selection.cueList.priority}` : ""}`,
					}}
					groups={[
						{ id: "cuelist-navigation", actions: [
							{
								id: "pool",
								label: "← Cuelist Pool",
								onPress: props.onOpenPool,
							},
							{
								id: "settings",
								label: "Cuelist Settings",
								onPress: props.onOpenSettings,
							},
						] },
					]}
				/>
			)}
			<div
				className={`sequence-layout ${props.cueInformationBlock !== "off" ? "with-cue-information" : ""}`.trim()}
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
						Boolean(props.viewOnly),
					)}
					onSelectCue={editor.setSelectedCue}
					onEditCueProperty={(index, property) => {
						editor.setSelectedCue(index);
						setPropertyEditor({ index, property });
					}}
					onOpenCuePreview={setPreviewCue}
					interactive={!props.viewOnly}
					compactRows={props.compactRows}
					timingProgressByRow={timingProgressByRow}
					playbackNumber={props.selectedCuelist}
					command={command}
				/>
				{props.cueInformationBlock !== "off" && (
					<section
						className="cue-information-block"
						aria-label={`${informationLabel} Information`}
					>
						<strong>
							{informationLabel}
							{informationCue
								? ` · Cue ${informationCue.number}${informationCue.name ? ` · ${informationCue.name}` : ""}`
								: " · None"}
						</strong>
						<p>
							{informationCue?.information ||
								`No ${informationLabel.toLowerCase()} information.`}
						</p>
					</section>
				)}
				{propertyEditor && cues[propertyEditor.index] && (
					<CuePropertyModal
						cue={cues[propertyEditor.index]}
						cues={cues}
						property={propertyEditor.property}
						editError={editor.cueEditError}
						onCancel={() => setPropertyEditor(null)}
						onSave={async (cue) => {
							editor.setCueDraft(cue);
							return editor.saveCue(cue);
						}}
					/>
				)}
				{previewCue !== null && cues[previewCue] && thumbnails[previewCue] && (
					<ModalFrame
						id={`cue-preview-${cues[previewCue].id ?? cues[previewCue].number}`}
						ariaLabel={`Cue ${cues[previewCue].number} preview image`}
						title={`Cue ${cues[previewCue].number}${cues[previewCue].name ? ` · ${cues[previewCue].name}` : ""}`}
						closeLabel="Close Cue preview"
						dialogClassName="cuelist-preview-modal"
						onClose={() => setPreviewCue(null)}
					>
						<div className="cuelist-preview-modal-body">
							<img
								src={thumbnails[previewCue]}
								alt={`Cue ${cues[previewCue].number} preview`}
							/>
						</div>
					</ModalFrame>
				)}
			</div>
			{props.settings}
		</div>
	);
}
