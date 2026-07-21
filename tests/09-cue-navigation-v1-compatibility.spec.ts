import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import { fixtureIdsByNumber, loadCanonicalCopy, putObject } from "./support/catalog";

/**
 * CUE navigation is owned by the typed v2 command-line HTTP contract; `CUE-014` states operator
 * intent through it. This spec deliberately retains the *other* surface: the v1 textual WebSocket
 * envelope that external integrations still use. It proves the compatibility caller reaches the
 * same typed Playback action and keeps its v1 response and notification behavior, so the v1 adapter
 * cannot silently diverge from the typed owner before it is removed.
 */
test.describe("docs/engineering/refactoring-test-boundaries.md", () => {
  test("CUE-015 @api › retained v1 WebSocket CUE navigation converges on the typed Playback action", async ({
    api,
    bench,
  }) => {
    await loadCanonicalCopy(api, bench, "cue-navigation-v1-compatibility", "compact-rig");
    await installTwinPlaybacks(api);
    await api.request("POST", "/api/v1/cuelists/2/select", {});

    // Go To through the v1 textual envelope moves the desk-selected Playback.
    const goTo = await api.command<unknown>("programmer.execute", { value: "CUE 3" });
    expect(goTo).toMatchObject({ protocol_version: 1, ok: true });
    expect(await runtime(api, 2)).toMatchObject({ current_cue_number: 3, master: 1, enabled: true });

    // Load through the same envelope marks the next Cue without moving the current one.
    await api.command("programmer.execute", { value: "CUE CUE 2" });
    expect(await runtime(api, 2)).toMatchObject({
      current_cue_number: 3,
      effective_next_cue_number: 2,
      effective_next_is_loaded: true,
    });

    // Explicit pool and explicit-page addressing behave identically on the compatibility surface.
    await api.command("programmer.execute", { value: "CUE SET 1 CUE 2" });
    expect(await runtime(api, 1)).toMatchObject({ current_cue_number: 2, enabled: true });
    await api.command("programmer.execute", { value: "CUE CUE SET 1 . 2 CUE 1" });
    expect(await runtime(api, 2)).toMatchObject({ effective_next_cue_number: 1, effective_next_is_loaded: true });

    // A rejected action reports through the v1 envelope and moves no runtime.
    const before = await playbackState(api);
    await expect(api.command("programmer.execute", { value: "CUE 99" })).rejects.toThrow(
      /programmer\.execute failed/i,
    );
    await expect(api.command("programmer.execute", { value: "CUE SET 99 CUE 1" })).rejects.toThrow(
      /playback 99 does not exist/i,
    );
    expect(await playbackState(api)).toMatchObject({
      selected_playback: before.selected_playback,
      active: before.active,
    });
  });
});

async function installTwinPlaybacks(api: ApiDriver): Promise<string> {
  const fixture = (await fixtureIdsByNumber(api))[1];
  const cueListId = crypto.randomUUID();
  await putObject(api, "cue_list", cueListId, {
    id: cueListId,
    name: "Compatibility Cuelist",
    priority: 0,
    mode: "sequence",
    looped: false,
    wrap_mode: "off",
    restart_mode: "first_cue",
    cues: [1, 2, 3].map((number) => ({
      id: crypto.randomUUID(),
      number,
      name: `Cue ${number}`,
      fade_millis: 0,
      delay_millis: 0,
      trigger: { type: "manual" },
      phasers: [],
      group_changes: [],
      changes: [
        {
          fixture_id: fixture,
          attribute: "intensity",
          value: { kind: "normalized", value: number / 10 },
          automatic_restore: false,
        },
      ],
    })),
  });
  for (const [number, name] of [
    [1, "Twin A"],
    [2, "Twin B"],
  ] as const) {
    await putObject(api, "playback", String(number), {
      number,
      name,
      target: { type: "cue_list", cue_list_id: cueListId },
      buttons: ["go_minus", "go", "flash"],
      fader: "master",
      go_activates: true,
      auto_off: false,
      xfade_millis: 0,
      color: "#20c997",
      flash_release: "release_all",
      protect_from_swap: false,
    });
  }
  await putObject(api, "playback_page", "1", { number: 1, name: "Main", slots: { "1": 1, "2": 2 } });
  return cueListId;
}

async function playbackState(api: ApiDriver): Promise<any> {
  return api.request("GET", "/api/v1/playbacks");
}

async function runtime(api: ApiDriver, playback: number): Promise<any> {
  return (await playbackState(api)).active.find((item: any) => item.playback_number === playback);
}
