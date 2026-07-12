import type { ComponentType } from "react";
import type { BuiltInWindow } from "../types";
import type { WindowProps } from "./windowTypes";
import { StageWindow } from "./StageWindow";
import { GroupsWindow } from "./GroupsWindow";
import { FixtureSheetWindow } from "./FixtureSheetWindow";
import { PresetsWindow } from "./PresetsWindow";
import { PlaybackWindow } from "./PlaybackWindow";
import { DynamicsWindow } from "./DynamicsWindow";
import { ChannelsWindow } from "./ChannelsWindow";
import { DmxWindow } from "./DmxWindow";
import { SetupWindow } from "./SetupWindow";

export const windowRegistry: Record<BuiltInWindow, ComponentType<WindowProps>> = {
  stage: StageWindow, groups: GroupsWindow, fixtures: FixtureSheetWindow, presets: PresetsWindow,
  playback: PlaybackWindow, dynamics: DynamicsWindow, channels: ChannelsWindow, dmx: DmxWindow, setup: SetupWindow,
};
