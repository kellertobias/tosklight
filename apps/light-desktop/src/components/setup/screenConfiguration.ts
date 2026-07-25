import type {
  ControlDesk,
  PlaybackSurfaceLayout,
  ScreenConfiguration,
} from "../../api/types";
import type { DeskModel } from "../../types";

interface ScreenLayout {
  desks: DeskModel[];
  activeDeskId: string;
}

export function defaultDeskPlaybackLayout(desk: ControlDesk): PlaybackSurfaceLayout {
  return desk.playback_layout ?? {
    playbacks_per_row: desk.columns,
    rows: Array.from({ length: desk.rows }, (_, index) => ({
      first_playback_slot: 1 + index * desk.columns,
      has_fader: true,
      button_count: desk.buttons,
    })),
  };
}

export function screenPlaybackLayout(screen: ScreenConfiguration): PlaybackSurfaceLayout {
  if (screen.playback_layout) return screen.playback_layout;
  const perRow = Math.ceil(screen.playback_count / screen.playback_rows);
  return {
    playbacks_per_row: perRow,
    rows: Array.from({ length: screen.playback_rows }, (_, index) => ({
      first_playback_slot: screen.first_playback_slot + index * perRow,
      has_fader: true,
      button_count: 3,
    })),
  };
}

export function playbackLayoutLegacyFields(layout: PlaybackSurfaceLayout) {
  return {
    columns: layout.playbacks_per_row,
    rows: layout.rows.length,
    buttons: Math.max(0, ...layout.rows.map((row) => row.button_count)),
    playback_count: layout.playbacks_per_row * layout.rows.length,
    playback_rows: layout.rows.length,
    first_playback_slot: layout.rows[0]?.first_playback_slot ?? 1,
  };
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
    playback_layout: {
      playbacks_per_row: 10,
      rows: Array.from({ length: 4 }, (_, index) => ({
        first_playback_slot: 41 + screens.length * 40 + index * 10,
        has_fader: true,
        button_count: 3,
      })),
    },
  };
}
