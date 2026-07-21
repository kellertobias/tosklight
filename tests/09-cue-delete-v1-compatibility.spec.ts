import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { loadCanonicalCopy, object } from "./support/catalog";
import {
  groupCue,
  installPlaybackSequence,
} from "./cueSemanticContracts/support";

/**
 * Public Cue-deletion scenarios use the exact revisioned v2 action helper. This one test retains
 * the deliberately separate integration surface used by existing external clients: a v1 textual
 * `programmer.execute` WebSocket envelope. Successful deletion proves that envelope reaches the
 * typed Programming action; Rust boundary tests own replay and legacy-notification cardinality.
 */
test.describe("docs/engineering/refactoring-test-boundaries.md", () => {
  test("CUE-016 @api › retained v1 WebSocket Cue deletion reaches the typed Programming action", async ({
    api,
    bench,
  }) => {
    await loadCanonicalCopy(
      api,
      bench,
      "cue-delete-v1-compatibility",
      "compact-rig",
    );
    const installed = await installPlaybackSequence(api, 1, [
      groupCue(1, []),
      groupCue(2, []),
    ]);
    const before = await object<any>(api, "cue_list", installed.id);

    const response = await api.command("programmer.execute", {
      value: "DELETE SET 1 CUE 2",
    });

    expect(response).toMatchObject({ protocol_version: 1, ok: true });
    const after = await object<any>(api, "cue_list", installed.id);
    expect(after.revision).toBe(before.revision + 1);
    expect(after.body.cues.map((cue: any) => [cue.id, cue.number])).toEqual([
      [before.body.cues[0].id, 1],
    ]);
  });
});
