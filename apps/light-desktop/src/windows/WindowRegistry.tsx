import type { ComponentType } from "react";
import type { BuiltInWindow } from "../types";
import type { WindowProps } from "./windowTypes";
import { StageWindow } from "./StageWindow";
import { GroupsWindow } from "./GroupsWindow";
import { FixtureSheetWindow } from "./FixtureSheetWindow";
import { PresetsWindow } from "./PresetsWindow";
import { CuelistWindow } from "./CuelistWindow";
import { DynamicsWindow } from "./DynamicsWindow";
import { ChannelsWindow } from "./ChannelsWindow";
import { DmxWindow } from "./DmxWindow";
import { SetupWindow } from "./SetupWindow";
import { PatchWindow } from "./PatchWindow";
import { HelpWindow } from "./HelpWindow";
import { DevelopmentWindow } from "./DevelopmentWindow";
import { VirtualPlaybacksWindow } from "./VirtualPlaybacksWindow";
import { FileManagerWindow } from "./FileManagerWindow";
import { TextEditorWindow } from "./TextEditorWindow";

export const windowRegistry: Record<BuiltInWindow, ComponentType<WindowProps>> = {
  stage: StageWindow, groups: GroupsWindow, fixtures: FixtureSheetWindow, presets: PresetsWindow,
  cuelists: CuelistWindow, cuelist_pool: (props) => <CuelistWindow {...props} cueListTab="pool"/>, cues: (props) => <CuelistWindow {...props} cueListTab="cues"/>,
  qlists: CuelistWindow, qlist_pool: (props) => <CuelistWindow {...props} cueListTab="pool"/>, qs: (props) => <CuelistWindow {...props} cueListTab="cues"/>, playback: CuelistWindow, playback_pool: (props) => <CuelistWindow {...props} cueListTab="pool"/>, cue_list: (props) => <CuelistWindow {...props} cueListTab="cues"/>, virtual_playbacks: VirtualPlaybacksWindow, file_manager: FileManagerWindow, text_editor: TextEditorWindow, dynamics: DynamicsWindow, channels: ChannelsWindow, dmx: DmxWindow, patch: PatchWindow, setup: SetupWindow, help: HelpWindow, development: DevelopmentWindow,
};
