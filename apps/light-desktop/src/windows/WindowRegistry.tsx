import type { ComponentType } from "react";
import type { BuiltInWindow } from "../types";
import { ChannelsWindow } from "./ChannelsWindow";
import { CuelistWindow } from "./CuelistWindow";
import { DmxWindow } from "./DmxWindow";
import { DynamicsWindow } from "./DynamicsWindow";
import { FileManagerWindow } from "./FileManagerWindow";
import { FixtureSheetWindow } from "./FixtureSheetWindow";
import { GroupsWindow } from "./GroupsWindow";
import { HelpWindow } from "./HelpWindow";
import { PatchWindow } from "./PatchWindow";
import { PresetsWindow } from "./PresetsWindow";
import { SchedulerWindow } from "./SchedulerWindow";
import { SetupWindow } from "./SetupWindow";
import { StageWindow } from "./StageWindow";
import { TextEditorWindow } from "./TextEditorWindow";
import { VirtualPlaybacksWindow } from "./VirtualPlaybacksWindow";
import type { WindowProps } from "./windowTypes";

export type RegisteredWindow = Exclude<BuiltInWindow, "layout">;

export function isRegisteredWindow(
	kind: BuiltInWindow,
): kind is RegisteredWindow {
	return kind !== "layout";
}

export const windowRegistry: Record<
	RegisteredWindow,
	ComponentType<WindowProps>
> = {
	stage: StageWindow,
	groups: GroupsWindow,
	fixtures: FixtureSheetWindow,
	presets: PresetsWindow,
	cuelists: CuelistWindow,
	cuelist_pool: (props) => <CuelistWindow {...props} cueListTab="pool" />,
	cues: (props) => <CuelistWindow {...props} cueListTab="cues" />,
	qlists: CuelistWindow,
	qlist_pool: (props) => <CuelistWindow {...props} cueListTab="pool" />,
	qs: (props) => <CuelistWindow {...props} cueListTab="cues" />,
	playback: CuelistWindow,
	playback_pool: (props) => <CuelistWindow {...props} cueListTab="pool" />,
	cue_list: (props) => <CuelistWindow {...props} cueListTab="cues" />,
	virtual_playbacks: VirtualPlaybacksWindow,
	file_manager: FileManagerWindow,
	text_editor: TextEditorWindow,
	dynamics: DynamicsWindow,
	scheduler: SchedulerWindow,
	channels: ChannelsWindow,
	dmx: DmxWindow,
	patch: PatchWindow,
	setup: SetupWindow,
	help: HelpWindow,
};
