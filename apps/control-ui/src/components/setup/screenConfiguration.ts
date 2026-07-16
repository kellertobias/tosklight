import type { ScreenConfiguration } from "../../api/types";
import type { DeskModel } from "../../types";

interface ScreenLayout {
  desks: DeskModel[];
  activeDeskId: string;
}

export function screenForAddAction(
  screens: ScreenConfiguration[],
  layout: ScreenLayout,
  createId: () => string = () => crypto.randomUUID(),
): ScreenConfiguration {
  const configuredScreen = screens.find((screen) => !screen.desired_open);
  if (configuredScreen) return { ...configuredScreen, desired_open: true };

  return createScreenConfiguration(screens, layout, true, createId);
}

export function createScreenConfiguration(
  screens: ScreenConfiguration[],
  layout: ScreenLayout,
  desiredOpen = false,
  createId: () => string = () => crypto.randomUUID(),
): ScreenConfiguration {
  return {
    id: createId(),
    name: `Screen ${screens.length + 1}`,
    layout,
    show_dock: true,
    show_playbacks: true,
    playback_count: 40,
    playback_rows: 4,
    first_playback_slot: 41 + screens.length * 40,
    page_mode: "follow_main",
    show_page_controls: true,
    desired_open: desiredOpen,
    display_id: null,
    bounds: null,
    fullscreen: false,
  };
}
