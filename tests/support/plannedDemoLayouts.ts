import type { ApiDriver } from "../bench/core/api";

export function plannedDemoLayout() {
  return {
    activeDeskId: "busking",
    desks: [
      {
        id: "busking",
        name: "Busking",
        panes: [
          pane("busking-groups", "groups", "Groups", 1, 1, 6, 9),
          pane("busking-color", "presets", "Color Presets", 7, 1, 6, 9, { presetFamily: "Color" }),
          pane("busking-position", "presets", "Position Presets", 1, 10, 6, 9, { presetFamily: "Position" }),
          pane("busking-beam", "presets", "Beam Presets", 7, 10, 6, 9, { presetFamily: "Beam" }),
          pane("busking-playbacks", "virtual_playbacks", "Virtual Playbacks", 13, 1, 12, 18, {
            virtualPlaybackRows: 6,
            virtualPlaybackColumns: 5,
            virtualPlaybackPageMode: "pinned",
            virtualPlaybackPinnedPage: 1,
          }),
        ],
      },
      {
        id: "programming",
        name: "Programming",
        panes: [
          pane("programming-fixtures", "fixtures", "Fixture Sheet", 1, 1, 12, 18),
          pane("programming-stage", "stage", "Stage", 13, 1, 12, 12, {
            stageView: "3d",
            followPreload: true,
            showBeamGuides: true,
            stageRenderQuality: "lines_and_beams",
          }),
          pane("programming-dmx", "dmx", "DMX Output", 13, 13, 12, 6),
        ],
      },
      {
        id: "theater",
        name: "Theater",
        panes: [
          pane("theater-cuelist", "cue_list", "Cuelist", 1, 1, 12, 18, {
            showCueSidebar: true,
            cueListSource: "follow-selection",
          }),
          pane("theater-text", "text_editor", "Theater Script", 13, 1, 12, 18, {
            textEditorMode: "split",
            textEditorReadOnly: false,
          }),
        ],
      },
    ],
  };
}

export async function installPlannedDemoLayout(api: ApiDriver, showId: string) {
  const userId = api.session?.user.id;
  if (!userId) throw new Error("Plan 76 desktop generation requires an authenticated user");
  const body = plannedDemoLayout();
  const existing = (await api.showObjects<any>(showId, "user_layout"))
    .find((layout) => layout.id === userId);
  await api.seedShowObject(showId, "user_layout", userId, body, existing?.revision ?? 0);
  return body;
}

function pane(
  id: string,
  kind: string,
  title: string,
  x: number,
  y: number,
  width: number,
  height: number,
  options: Record<string, unknown> = {},
) {
  return { id, kind, title, x, y, width, height, ...options };
}
