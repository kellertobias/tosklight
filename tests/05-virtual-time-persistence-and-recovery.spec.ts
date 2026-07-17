import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import { promisify } from "node:util";
import type { Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import type { DeskDriver } from "../apps/control-ui/e2e/bench/desk";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import type { LightBench } from "../apps/control-ui/e2e/bench/lightBench";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import type { OscHardware } from "../apps/control-ui/e2e/bench/protocols";
import {
  activeShowId,
  fixtureIdsByNumber,
  normalized,
  object,
  programmer,
  putObject,
  loadCanonicalCopy,
} from "./support/catalog";

const sqlite = promisify(execFile);
const FIXED_NOW = "2020-01-01T00:00:00Z";
const SHOW_004_CASES = ["fixture-number", "group-defaults", "playback-defaults", "route-defaults", "virtual-dimmer-metadata", "cue-defaults"] as const;
type Show004Case = typeof SHOW_004_CASES[number];
type HardwareState = { hardware?: OscHardware; hardwareClientId?: string };

test.describe("docs/testing/05-virtual-time-persistence-and-recovery.md", () => {
  pairedScenario<HardwareState>({
    id: "TIME-001",
    title: "zero ticks emit current state without advancing behavior time",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `time-001-${surface}`);
      expect((await api.request<{ now: string }>("POST", "/api/v1/test/clock/reset", undefined, false)).now).toBe(FIXED_NOW);
      // Reset deliberately clears the test-bench programmer registry. Reconnect the durable user
      // so this surface starts with the same production session/programmer relationship as the UI.
      await api.login();
      return {};
    },
    api: async ({ api }) => {
      await setProgrammerFade(api, 0, 3_000);
      await api.command("programmer.execute", { value: "FIXTURE 1 AT 50" });
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await connectHardware(api, bench, state, "time-001-ui");
      await desk.open(bench.baseUrl);
      await setProgrammerFadeThroughUi(api, page, 0);
      await openFixtures(page);
      await fixtureRow(page, 1).click();
      await setDimmerByTouch(page, 50);
    },
    assert: async ({ api, bench }, state) => {
      try {
        await assertZeroTicks(api, bench);
      } finally {
        await disconnectHardware(api, state);
      }
    },
  });

  pairedScenario<{ fixtureId: string } & HardwareState>({
    id: "TIME-002",
    title: "all programmer-fade boundaries are exact",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `time-002-${surface}`);
      return { fixtureId: (await fixtureIdsByNumber(api))[1] };
    },
    api: async ({ api, bench }) => {
      await setProgrammerFade(api, 3_000);
      await api.command("programmer.execute", { value: "FIXTURE 1 AT 0" });
      await bench.tick(3_000);
      await api.command("programmer.execute", { value: "FIXTURE 1 AT 100" });
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await connectHardware(api, bench, state, "time-002-ui");
      await desk.open(bench.baseUrl);
      await setProgrammerFadeThroughUi(api, page, 3);
      await openFixtures(page);
      await fixtureRow(page, 1).click();
      await setDimmerByTouch(page, 0);
      await bench.tick(3_000);
      await fixtureRow(page, 1).click();
      await setDimmerByTouch(page, 100);
      await expectEncoderTarget(page, 100);
    },
    assert: async ({ api, bench }, state) => {
      try {
        await assertFadeBoundaries(api, bench, state.fixtureId);
      } finally {
        await disconnectHardware(api, state);
      }
    },
  });

  test("TIME-002 @ui › touch-set fixture timing is stored and replayed as resolved light and DMX", async ({ api, bench, desk, page }) => {
    const hardware: HardwareState = {};
    try {
      await loadCanonicalCopy(api, bench, "time-002-fixture-cue");
      const fixtureId = (await fixtureIdsByNumber(api))[1];
      await connectHardware(api, bench, hardware, "time-002-fixture-cue");
      await desk.open(bench.baseUrl);
      await setProgrammerFadeThroughUi(api, page, 3);
      await openFixtures(page);
      await fixtureRow(page, 1).click();
      await setDimmerByTouch(page, 0);
      await bench.tick(3_000);
      await fixtureRow(page, 1).click();
      await setDimmerByTouch(page, 100);

      await expectEncoderTarget(page, 100);
      await bench.tick(0);
      await expectFixtureSheetDimmer(page, 1, 0);
      await desk.recordStep(
        "PROGRAMMER TARGET 100% · RESOLVED 0%",
        "The touch encoder has jumped immediately to 100%, while Fixture Sheet and actual DMX remain at the start of the three-second Programmer Fade.",
      );
      await expect.poll(async () => {
        const value = (await programmer(api)).values.find(
          (candidate: any) => candidate.fixture_id === fixtureId && candidate.attribute === "intensity",
        );
        return value?.fade_millis;
      }).toBe(3_000);

      const cue = await recordFirstCuelistThroughUi(api, page);
      const change = cue.changes.find(
        (candidate: any) => candidate.fixture_id === fixtureId && candidate.attribute === "intensity",
      );
      expect(change).toMatchObject({
        value: { kind: "normalized", value: 1 },
        fade_millis: 3_000,
      });

      await api.command("programmer.clear", {});
      await expect.poll(async () => (await programmer(api)).values).toEqual([]);
      await openFixtures(page);
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      await assertCueReplayBoundaries(api, bench, page, desk, [{ fixtureId, number: 1, slot: 1 }]);
    } finally {
      await disconnectHardware(api, hardware);
    }
  });

  test("TIME-002 @ui › touch-set Group timing is stored and replayed for every member", async ({ api, bench, desk, page }) => {
    const hardware: HardwareState = {};
    try {
      await loadCanonicalCopy(api, bench, "time-002-group-cue");
      const fixtureIds = await fixtureIdsByNumber(api);
      await connectHardware(api, bench, hardware, "time-002-group-cue");
      await desk.open(bench.baseUrl);
      await setProgrammerFadeThroughUi(api, page, 3);
      await page.getByRole("button", { name: "Groups", exact: true }).click();
      await expect(page.locator(".group-pool-window")).toBeVisible();
      await groupCard(page, 3).click();
      await setDimmerByTouch(page, 0);
      await bench.tick(3_000);
      await groupCard(page, 3).click();
      await setDimmerByTouch(page, 100);

      await expectEncoderTarget(page, 100);
      await bench.tick(0);
      await openFixtures(page);
      for (const number of [1, 2, 3, 4]) await expectFixtureSheetDimmer(page, number, 0);
      await desk.recordStep(
        "GROUP TARGET 100% · MEMBERS RESOLVED 0%",
        "The Group touch encoder has jumped immediately to 100%, while every member and actual DMX remain at the start of the three-second Programmer Fade.",
      );
      await expect.poll(async () =>
        (await programmer(api)).group_values["3"]?.intensity?.fade_millis,
      ).toBe(3_000);

      const cue = await recordFirstCuelistThroughUi(api, page);
      const change = cue.group_changes.find(
        (candidate: any) => candidate.group_id === "3" && candidate.attribute === "intensity",
      );
      expect(change).toMatchObject({
        value: { kind: "normalized", value: 1 },
        fade_millis: 3_000,
      });

      await api.command("programmer.clear", {});
      await expect.poll(async () => (await programmer(api)).group_values).toEqual({});
      await openFixtures(page);
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      await assertCueReplayBoundaries(
        api,
        bench,
        page,
        desk,
        [1, 2, 3, 4].map((number) => ({ fixtureId: fixtureIds[number], number, slot: number })),
      );
    } finally {
      await disconnectHardware(api, hardware);
    }
  });

  test("TIME-003 @wire › chaser and phaser phase use virtual timestamps across incremental, speed, pause, and week jumps", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "time-003");
    const fixtures = await fixtureIdsByNumber(api);
    const showId = await activeShowId(api);
    const chaserId = await installTimeCuelists(api, fixtures[1], fixtures[2]);
    await setSpeedGroups(api, [120, 90, 60, 30, 15]);

    await restartPlaybackRun(api, bench, showId, [1]);
    await bench.tick(1_000);
    const direct = await playbackRuntime(api, 1);
    await restartPlaybackRun(api, bench, showId, [1]);
    for (let index = 0; index < 4; index += 1) await bench.tick(250);
    const incremental = await playbackRuntime(api, 1);
    expect(incremental.current_cue_number).toBe(direct.current_cue_number);
    expect(incremental.activated_at).toBe(direct.activated_at);

    await restartPlaybackRun(api, bench, showId, [1]);
    await bench.tick(250);
    await setSpeedGroups(api, [60, 90, 60, 30, 15]);
    await bench.tick(499);
    expect((await playbackRuntime(api, 1)).current_cue_number).toBe(1);
    await bench.tick(1);
    expect((await playbackRuntime(api, 1)).current_cue_number).toBe(2);

    await setSpeedGroups(api, [120, 90, 60, 30, 15]);
    await restartPlaybackRun(api, bench, showId, [1]);
    await bench.tick(250);
    await api.request("POST", `/api/v1/playbacks/${chaserId}/pause`, {});
    const paused = await playbackRuntime(api, 1);
    await bench.tick(3_000);
    expect(await playbackRuntime(api, 1)).toMatchObject({ current_cue_number: paused.current_cue_number, paused: true });
    await api.request("POST", `/api/v1/playbacks/${chaserId}/go`, {});
    await bench.tick(249);
    expect((await playbackRuntime(api, 1)).current_cue_number).toBe(1);
    await bench.tick(1);
    expect((await playbackRuntime(api, 1)).current_cue_number).toBe(2);

    await restartPlaybackRun(api, bench, showId, [1]);
    const week = await bench.tick(604_800_000);
    expect(week.now).toBe("2020-01-08T00:00:00Z");
    expect((await playbackRuntime(api, 1)).current_cue_number).toBe(1);

    await restartPlaybackRun(api, bench, showId, [2]);
    await bench.tick(250);
    const phaserDirect = await visualizationLevel(api, fixtures[2], "intensity");
    await restartPlaybackRun(api, bench, showId, [2]);
    for (let index = 0; index < 10; index += 1) await bench.tick(25);
    expect(await visualizationLevel(api, fixtures[2], "intensity")).toBeCloseTo(phaserDirect, 6);
    expect(phaserDirect).toBeCloseTo(0.5, 6);

    await api.request("POST", "/api/v1/cuelists/3/button", { button: 3, pressed: true });
    await bench.tick(1_000);
    expect(await visualizationLevel(api, fixtures[2], "intensity")).toBeCloseTo(phaserDirect, 6);
    await api.request("POST", "/api/v1/cuelists/3/button", { button: 3, pressed: true });
    await bench.tick(250);
    expect(await visualizationLevel(api, fixtures[2], "intensity")).toBeCloseTo(1, 6);

    await restartPlaybackRun(api, bench, showId, [2]);
    await bench.tick(604_800_000);
    expect(await visualizationLevel(api, fixtures[2], "intensity")).toBeCloseTo(0, 6);
  });

  pairedScenario<{ copyId: string; fixtureIds: Record<number, string>; revisionName: string }>({
    id: "SHOW-001",
    title: "operator programming and a named revision produce the durable restart state",
    arrange: async ({ api, bench }, surface) => {
      const copy = await loadCanonicalCopy(api, bench, `show-001-${surface}`);
      await setProgrammerFade(api, 0, 3_000);
      return {
        copyId: copy.id,
        fixtureIds: await fixtureIdsByNumber(api),
        revisionName: "SHOW-001 before restart",
      };
    },
    api: async ({ api }, state) => {
      await api.command("selection.set", { fixtures: [state.fixtureIds[5], state.fixtureIds[6]] });
      await api.command("programmer.execute", { value: "RECORD + GROUP 3" });
      await api.command("programmer.execute", { value: "GROUP 3 AT 40" });
      await api.command("programmer.execute", { value: "RECORD SET 1" });
      await api.command("programmer.execute", { value: "SET 1 AT 1.1" });
      await api.command("programmer.clear", {});
      await api.command("programmer.clear", {});
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      await api.command("programmer.execute", { value: "FIXTURE 12 AT 65" });
      await api.request("POST", `/api/v1/shows/${state.copyId}/revisions`, { name: state.revisionName });
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      await programShow001ThroughUi(api, page, state.revisionName);
    },
    assert: async ({ api, bench }, state) => assertShow001State(api, bench, state),
  });

  test("SHOW-001 @restart › supplemental process check preserves named show state, durable programmer, active playback, PID, and first frame", async ({ api, bench }) => {
    const copy = await loadCanonicalCopy(api, bench, "show-001");
    await setProgrammerFade(api, 0, 0);
    const fixtures = await fixtureIdsByNumber(api);
    const group = await object<any>(api, "group", "3");
    await putObject(api, "group", "3", { ...group.body, fixtures: [...group.body.fixtures, fixtures[5], fixtures[6]] }, group.revision);
    const expectedGroup = [1, 2, 3, 4, 5, 6].map((number) => fixtures[number]);
    expect((await object<any>(api, "group", "3")).body.fixtures).toEqual(expectedGroup);
    const cueListId = await installGroupCue(api, "3", 0.4);
    expect((await object<any>(api, "group", "3")).body.fixtures).toEqual(expectedGroup);
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await api.command("programmer.execute", { value: "FIXTURE 12 AT 65" });
    await bench.tick(0);
    await api.command("master.set", { grand_master: 0.5, blackout: false });
    const durableBefore = (await programmer(api));
    const revision = await api.request<any>("POST", `/api/v1/shows/${copy.id}/revisions`, { name: "SHOW-001 before restart" });
    expect(revision.name).toBe("SHOW-001 before restart");
    const expectedFirstFrame = await bench.tick(0);
    expect(expectedFirstFrame.universes.find((universe) => universe.universe === 1)?.slots.slice(0, 6)).toEqual(Array(6).fill(51));
    expect(slot(expectedFirstFrame, 12)).toBe(83);
    const entry = await showEntry(api, copy.id);
    expect(entry.path.startsWith(bench.dataDir)).toBe(true);
    const oldPid = bench.serverPid();
    expect(oldPid).toBeTruthy();
    await bench.stopServerGracefully(api.session!.token);
    const showHash = await fileHash(entry.path);
    const newPid = await bench.startServer();
    expect(newPid).not.toBe(oldPid);
    await api.login("Operator");

    const bootstrap = await api.request<any>("GET", "/api/v1/bootstrap", undefined, false);
    expect(bootstrap.active_show.id).toBe(copy.id);
    expect((await object<any>(api, "group", "3")).body.fixtures).toEqual(expectedGroup);
    const restoredGroupChange = (await object<any>(api, "cue_list", cueListId)).body.cues[0].group_changes[0];
    expect(restoredGroupChange).toMatchObject({ group_id: "3", attribute: "intensity", value: { kind: "normalized" } });
    expect(restoredGroupChange.value.value).toBeCloseTo(0.4, 6);
    expect((await object<any>(api, "playback", "1")).body.target.cue_list_id).toBe(cueListId);
    expect(await playbackRuntime(api, 1)).toMatchObject({ current_cue_number: 1, enabled: true });
    const restored = await programmer(api);
    expect(restored.user_id).toBe(durableBefore.user_id);
    expect(restored.values.find((value: any) => value.fixture_id === fixtures[12] && value.attribute === "intensity")?.value).toMatchObject({ value: 0.65 });
    expect(await fileHash(entry.path)).toBe(showHash);
    expect(await api.request<any>("GET", "/api/v1/visualization")).toMatchObject({ grand_master: 0.5, blackout: false });
    const firstFrame = await bench.tick(0);
    expect(firstFrame.universes).toEqual(expectedFirstFrame.universes);
    expect(firstFrame.universes.find((universe) => universe.universe === 1)?.slots.slice(0, 6)).toEqual(Array(6).fill(51));
    expect(slot(firstFrame, 12)).toBe(83);
  });

  for (const fault of ["before-atomic-replacement", "during-temporary-write", "after-replacement-before-cleanup"] as const) {
    test(`SHOW-002 @restart › supplemental ${fault} fixture recovers as one complete old or new SQLite revision`, async ({ api, bench }) => {
      const copy = await loadCanonicalCopy(api, bench, `show-002-${fault}`);
      let group = await object<any>(api, "group", "3");
      await putObject(api, "group", "3", { ...group.body, name: "Atomic baseline" }, group.revision);
      await api.request("POST", `/api/v1/shows/${copy.id}/revisions`, { name: "SHOW-002 baseline" });
      const entry = await showEntry(api, copy.id);
      await bench.stopServerGracefully(api.session!.token);
      const oldBytes = await fs.readFile(entry.path);
      const oldHash = hash(oldBytes);

      await bench.startServer();
      await api.login();
      group = await object<any>(api, "group", "3");
      await putObject(api, "group", "3", { ...group.body, name: "Atomic replacement" }, group.revision);
      await bench.stopServerGracefully(api.session!.token);
      const newBytes = await fs.readFile(entry.path);
      const newHash = hash(newBytes);
      expect(newHash).not.toBe(oldHash);
      await fs.writeFile(entry.path, oldBytes);

      const temporary = `${entry.path}.storage-fault.tmp`;
      const backup = `${entry.path}.storage-fault.backup`;
      await fs.writeFile(backup, oldBytes);
      if (fault === "during-temporary-write") await fs.writeFile(temporary, newBytes.subarray(0, Math.floor(newBytes.length / 2)));
      if (fault === "after-replacement-before-cleanup") await fs.writeFile(entry.path, newBytes);

      await bench.startServer();
      await api.login();
      const recoveredHash = await fileHash(entry.path);
      const expectedNew = fault === "after-replacement-before-cleanup";
      expect(recoveredHash).toBe(expectedNew ? newHash : oldHash);
      expect((await object<any>(api, "group", "3")).body.name).toBe(expectedNew ? "Atomic replacement" : "Atomic baseline");
      expect([oldHash, newHash]).toContain(recoveredHash);
      expect((await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).active_show_error).toBeNull();
      if (fault === "during-temporary-write") expect((await fs.stat(temporary)).size).toBeLessThan(newBytes.length);
    });
  }

  pairedScenario<{ damagedPath: string; damagedHash: string; damagedShowId: string; recoveryShowId: string; recoveryShowName: string }>({
    id: "SHOW-003",
    title: "a malformed active show stays intact while the operator opens a valid recovery show",
    arrange: async ({ api, bench }, surface) => arrangeMalformedRecovery(api, bench, surface),
    api: async ({ api }, state) => {
      await api.request("POST", `/api/v1/shows/${state.recoveryShowId}/open`, { transition: "safe_blackout" });
    },
    ui: async ({ bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      const recovery = page.getByRole("alertdialog", { name: "Show recovery required" });
      await expect(recovery).toBeVisible();
      await expect(recovery).toContainText("has not been changed or deleted");
      await recovery.getByRole("button", { name: `Load Latest Autosave for ${state.recoveryShowName}` }).click();
    },
    assert: async ({ api, bench }, state) => {
      await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).active_show?.id).toBe(state.recoveryShowId);
      const bootstrap = await api.request<any>("GET", "/api/v1/bootstrap", undefined, false);
      expect(bootstrap.active_show_error).toBeNull();
      expect(await fileHash(state.damagedPath)).toBe(state.damagedHash);
      const frame = await bench.tick(0);
      const universe = frame.universes.find((candidate) => candidate.universe === 1);
      expect(universe).toBeDefined();
      expect(universe!.slots.every((value) => value === 0)).toBe(true);
    },
  });

  for (const corruption of ["malformed", "schema-invalid", "referentially-invalid"] as const) {
    test(`SHOW-003 @restart › supplemental ${corruption} active show starts ready in recovery and preserves corrupt evidence`, async ({ api, bench }) => {
      const copy = await loadCanonicalCopy(api, bench, `show-003-${corruption}`);
      if (corruption === "referentially-invalid") {
        await putObject(api, "playback", "999", {
          number: 999, name: "Recovery reference", target: { type: "group", group_id: "1" }, buttons: ["select", "select_dereferenced", "flash"], button_count: 3,
          fader: "master", has_fader: true, go_activates: true, auto_off: true, xfade_millis: 0, color: "#20c997", flash_release: "release_all", protect_from_swap: false,
        });
      }
      const entry = await showEntry(api, copy.id);
      await bench.stopServerGracefully(api.session!.token);
      const validBytes = await fs.readFile(entry.path);
      if (corruption === "malformed") {
        await fs.writeFile(entry.path, Buffer.from("not a ToskLight SQLite show\n"));
      } else if (corruption === "schema-invalid") {
        await runSql(entry.path, "UPDATE objects SET body_json=json_set(body_json, '$.master', 'not-a-number') WHERE kind='group' AND id='1'");
      } else {
        await runSql(entry.path, "UPDATE objects SET body_json=json_set(body_json, '$.target', json('{\"type\":\"group\",\"group_id\":\"missing-group\"}')) WHERE kind='playback' AND id='999'");
      }
      const corruptHash = await fileHash(entry.path);
      await bench.startServer();
      await api.login();

      const readinessResponse = await fetch(`${bench.baseUrl}/api/v1/readiness`);
      expect(readinessResponse.ok).toBe(true);
      const readiness = await readinessResponse.json() as any;
      expect(readiness).toMatchObject({ status: "ready", recovery_mode: true });
      expect(readiness.active_show_error).toBeTruthy();
      const bootstrap = await api.request<any>("GET", "/api/v1/bootstrap", undefined, false);
      expect(bootstrap.active_show.id).toBe(copy.id);
      expect(bootstrap.active_show_error).toBeTruthy();
      const safe = await bench.tick(0);
      expect(safe.universes.every((universe) => universe.slots.every((value) => value === 0))).toBe(true);
      expect(await fileHash(entry.path)).toBe(corruptHash);

      const recovered = await api.request<{ id: string }>("POST", "/api/v1/shows", {
        name: `show-003-recovered-${corruption}-${crypto.randomUUID()}`,
        data_base64: validBytes.toString("base64"),
        overwrite: false,
      });
      await api.request("POST", `/api/v1/shows/${recovered.id}/open`, { transition: "safe_blackout" });
      expect((await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).active_show_error).toBeNull();
      expect((await bench.tick(0)).universes.find((universe) => universe.universe === 1)).toBeDefined();
      expect(await fileHash(entry.path)).toBe(corruptHash);
    });
  }

  for (const migration of SHOW_004_CASES) {
    test(`SHOW-004 @restart › supplemental ${migration} legacy fields normalize once and stay byte/revision stable`, async ({ api, bench }) => {
      const prepared = await prepareMigrationCase(api, bench, migration);
      await bench.stopServerGracefully(api.session!.token);
      await stageLegacyMigration(prepared.entry.path, migration, prepared.cueListId);
      const legacyHash = await fileHash(prepared.entry.path);

      await bench.startServer();
      await api.login();
      const migrated = await migrationSnapshot(api, migration, prepared.cueListId);
      assertMigrationSnapshot(migration, migrated);
      await bench.stopServerGracefully(api.session!.token);
      const migratedHash = await fileHash(prepared.entry.path);
      expect(migratedHash).not.toBe(legacyHash);

      await bench.startServer();
      await api.login();
      const reopened = await migrationSnapshot(api, migration, prepared.cueListId);
      expect(reopened).toEqual(migrated);
      await bench.stopServerGracefully(api.session!.token);
      expect(await fileHash(prepared.entry.path)).toBe(migratedHash);
      await bench.startServer();
      await api.login();
    });
  }

  test("FIXTURE-001 @restart › supplemental fresh startup seeds reserved Generic profiles once with stable IDs", async ({ api, bench }) => {
    const initial = reservedGenericProfileSnapshot(await fixtureProfiles(api));
    expect(initial.length).toBeGreaterThan(0);
    expect(new Set(initial.map((profile) => profile.id)).size).toBe(initial.length);
    expect(initial.every((profile) => profile.manufacturer === "Generic" && profile.revision === 1)).toBe(true);

    await bench.stopServerGracefully(api.session!.token);
    await bench.startServer();
    await api.login();

    expect(reservedGenericProfileSnapshot(await fixtureProfiles(api))).toEqual(initial);
    const database = `${bench.dataDir}/fixtures.sqlite`;
    expect(Number(await readSql(database, "SELECT COUNT(DISTINCT id) FROM fixture_profiles WHERE reserved_source='builtin:generic-catalog'"))).toBe(initial.length);
    expect(Number(await readSql(database, "SELECT COUNT(*) FROM fixture_profile_migration_failures WHERE legacy_id IN (SELECT legacy_id FROM fixture_profile_legacy_map WHERE profile_id IN (SELECT id FROM fixture_profiles WHERE reserved_source='builtin:generic-catalog'))"))).toBe(0);
  });

  test("FIXTURE-001 @restart › supplemental compatible schema-v1 modes migrate on real startup and retain exact sources idempotently", async ({ api, bench }) => {
    const definitions = await fixtureDefinitions(api);
    const dimmerModes = definitions
      .filter((definition) => definition.manufacturer === "Generic" && definition.model === "Dimmer")
      .slice(0, 2);
    expect(dimmerModes).toHaveLength(2);
    const family = `Legacy startup ${crypto.randomUUID()}`;
    const rows: LegacyFixtureRow[] = dimmerModes.map((definition, index) => ({
      definition: {
        ...definition,
        id: crypto.randomUUID(),
        revision: 1,
        schema_version: 1,
        manufacturer: "E2E Legacy",
        name: family,
        model: family,
        mode: index === 0 ? "Coarse" : "Fine",
        profile_id: null,
        mode_id: null,
        profile_snapshot: null,
      },
      source: Buffer.from(`retained-compatible-gdtf-${index}`),
    }));
    const expectedProfileId = rows[0].definition.id;
    const database = `${bench.dataDir}/fixtures.sqlite`;

    await bench.stopServerGracefully(api.session!.token);
    await insertLegacyFixtureRows(database, rows);
    await bench.startServer();
    await api.login();

    expect(await api.request<any>("GET", "/api/v1/readiness", undefined, false)).toMatchObject({ status: "ready", recovery_mode: false });
    const migrated = (await fixtureProfiles(api)).find((profile) => profile.manufacturer === "E2E Legacy" && profile.name === family);
    expect(migrated).toMatchObject({ id: expectedProfileId, revision: 1, schema_version: 2, reserved_source: null });
    expect(migrated.modes.map((mode: any) => mode.name)).toEqual(["Coarse", "Fine"]);

    await bench.stopServerGracefully(api.session!.token);
    for (const row of rows) {
      expect(await legacyFixtureRow(database, row.definition.id)).toEqual({
        json: JSON.stringify(row.definition),
        sourceHex: row.source.toString("hex").toUpperCase(),
      });
    }
    expect(Number(await readSql(database, `SELECT COUNT(*) FROM fixture_profile_legacy_map WHERE profile_id=${sqlString(expectedProfileId)} AND profile_revision=1`))).toBe(2);
    const firstSnapshot = await fixtureProfileMigrationSnapshot(database, expectedProfileId);

    await bench.startServer();
    await api.login();
    const reopened = (await fixtureProfiles(api)).find((profile) => profile.id === expectedProfileId);
    expect(reopened).toEqual(migrated);
    await bench.stopServerGracefully(api.session!.token);
    expect(await fixtureProfileMigrationSnapshot(database, expectedProfileId)).toBe(firstSnapshot);
    await bench.startServer();
    await api.login();
  });

  test("FIXTURE-001 @restart › supplemental malformed and conflicting schema-v1 rows keep startup ready with retained evidence and stable warnings", async ({ api, bench }) => {
    const [base] = (await fixtureDefinitions(api)).filter((definition) => definition.manufacturer === "Generic" && definition.model === "Dimmer");
    expect(base).toBeDefined();
    const family = `Conflicting startup ${crypto.randomUUID()}`;
    const conflictingRows: LegacyFixtureRow[] = [0, 1].map((index) => ({
      definition: {
        ...base,
        id: crypto.randomUUID(),
        revision: 1,
        schema_version: 1,
        manufacturer: "E2E Recovery",
        name: family,
        model: family,
        mode: index === 0 ? "Narrow" : "Wide",
        physical: { ...base.physical, width_millimetres: index === 0 ? 250 : 500 },
        profile_id: null,
        mode_id: null,
        profile_snapshot: null,
      },
      source: Buffer.from(`retained-conflict-gdtf-${index}`),
    }));
    const malformedId = crypto.randomUUID();
    const malformedJson = "{";
    const malformedSource = Buffer.from("retained-malformed-gdtf");
    const database = `${bench.dataDir}/fixtures.sqlite`;

    await bench.stopServerGracefully(api.session!.token);
    await insertLegacyFixtureRows(database, conflictingRows);
    await runSql(database, `INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json,source_gdtf) VALUES(${sqlString(malformedId)},1,'Broken','Broken','Broken',${sqlString(malformedJson)},X'${malformedSource.toString("hex")}')`);
    await bench.startServer();
    await api.login();

    expect(await api.request<any>("GET", "/api/v1/readiness", undefined, false)).toMatchObject({ status: "ready", recovery_mode: false });
    const warnings = await fixtureProfileWarnings(api);
    expect(warnings.some((warning) => warning.includes(malformedId) && warning.includes("could not be migrated") && warning.includes("original definition and GDTF source were retained"))).toBe(true);
    expect(warnings.some((warning) => warning.includes("E2E Recovery") && warning.includes(family) && warning.includes("conflicting fixture-level metadata") && warning.includes("retained as separate profiles"))).toBe(true);
    const recoveryProfiles = (await fixtureProfiles(api)).filter((profile) => profile.manufacturer === "E2E Recovery" && profile.name === family);
    expect(recoveryProfiles).toHaveLength(2);
    expect(bench.recentLog()).toContain("fixture library migration requires operator attention");
    expect(bench.recentLog()).toContain(malformedId);

    await bench.stopServerGracefully(api.session!.token);
    expect(await legacyFixtureRow(database, malformedId)).toEqual({
      json: malformedJson,
      sourceHex: malformedSource.toString("hex").toUpperCase(),
    });
    for (const row of conflictingRows) {
      expect(await legacyFixtureRow(database, row.definition.id)).toEqual({
        json: JSON.stringify(row.definition),
        sourceHex: row.source.toString("hex").toUpperCase(),
      });
    }
    const failure = await readSql(database, `SELECT hex(error) FROM fixture_profile_migration_failures WHERE legacy_id=${sqlString(malformedId)} AND legacy_revision=1`);
    expect(failure).not.toBe("");
    const warningSnapshot = await fixtureWarningSnapshot(database, family, malformedId);

    await bench.startServer();
    await api.login();
    expect(await fixtureProfileWarnings(api)).toEqual(warnings);
    expect((await fixtureProfiles(api)).filter((profile) => profile.manufacturer === "E2E Recovery" && profile.name === family)).toEqual(recoveryProfiles);
    await bench.stopServerGracefully(api.session!.token);
    expect(await fixtureWarningSnapshot(database, family, malformedId)).toBe(warningSnapshot);
    expect(await readSql(database, `SELECT hex(error) FROM fixture_profile_migration_failures WHERE legacy_id=${sqlString(malformedId)} AND legacy_revision=1`)).toBe(failure);
    await bench.startServer();
    await api.login();
  });

  pairedScenario<{
    sourceId: string;
    sourceName: string;
    savedRevision: number;
    copyId?: string;
    copyProvenance?: Record<string, unknown>;
    expectedSourceName?: string;
    expectedCopyName?: string;
    expectedCopyRevisions?: string[];
  }>({
    id: "SHOW-005",
    title: "named revisions load as durable, visibly independent copies",
    arrange: async ({ api, bench }, surface) => {
      const source = await loadCanonicalCopy(api, bench, `show-005-${surface}`);
      const sourceEntry = await showEntry(api, source.id);
      const named = await showObject(api, source.id, "group", "4");
      await api.request("PUT", `/api/v1/shows/${source.id}/objects/group/4`, {
        ...named.body,
        name: "Named revision state",
      }, true, named.revision);
      const saved = await api.request<{ revision: number }>("POST", `/api/v1/shows/${source.id}/revisions`, { name: "Approved focus" });
      const latest = await showObject(api, source.id, "group", "4");
      await api.request("PUT", `/api/v1/shows/${source.id}/objects/group/4`, {
        ...latest.body,
        name: "Newer autosave state",
      }, true, latest.revision);
      return { sourceId: source.id, sourceName: sourceEntry.name, savedRevision: saved.revision };
    },
    api: async ({ api, bench }, state) => {
      const copy = await api.request<any>("POST", `/api/v1/shows/${state.sourceId}/revisions/${state.savedRevision}/open`, { transition: "hold_current" });
      expect(copy.id).not.toBe(state.sourceId);
      expect(copy.name).toMatch(new RegExp(`^${escapeRegex(state.sourceName)}-rev-${state.savedRevision}-\\d{4}-\\d{2}-\\d{2}`));
      expect(copy.revision_copy).toMatchObject({ show_id: state.sourceId, show_name: state.sourceName, revision: state.savedRevision, revision_name: "Approved focus" });

      const collision = await api.request<any>("POST", `/api/v1/shows/${state.sourceId}/revisions/${state.savedRevision}/open`, { transition: "hold_current" });
      expect(collision.id).not.toBe(copy.id);
      expect(collision.name).not.toBe(copy.name);
      await api.request("POST", `/api/v1/shows/${copy.id}/open`, { transition: "hold_current" });

      const copyGroup = await showObject(api, copy.id, "group", "4");
      await api.request("PUT", `/api/v1/shows/${copy.id}/objects/group/4`, {
        ...copyGroup.body,
        name: "Copy-only edit",
      }, true, copyGroup.revision);
      await api.request("POST", `/api/v1/shows/${copy.id}/revisions`, { name: "Copy checkpoint" });
      await bench.stopServerGracefully(api.session!.token);
      await bench.startServer();
      await api.login();

      state.copyId = copy.id;
      state.copyProvenance = copy.revision_copy;
      state.expectedSourceName = "Newer autosave state";
      state.expectedCopyName = "Copy-only edit";
      state.expectedCopyRevisions = ["Copy checkpoint"];
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      await page.getByRole("button", { name: /Open show menu/ }).click();
      await page.getByRole("button", { name: "Load", exact: true }).click();
      const sourceCard = page.locator(".revision-show-library article").filter({ has: page.getByText(state.sourceName, { exact: true }) });
      const revisionAction = sourceCard.locator(".named-revision-list button").filter({ hasText: "Approved focus" });
      await expect(revisionAction).toContainText("Load Revision as Copy");
      await revisionAction.click();

      await expect(page.locator(".dock-identity b")).toContainText("Revision Copy");
      await expect(page.getByRole("dialog", { name: "Load show", exact: true })).toBeHidden();
      const copy = (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).active_show;
      expect(copy.id).not.toBe(state.sourceId);
      const showMenu = page.getByRole("dialog", { name: "Show", exact: true });
      await expect(showMenu).toContainText(`Revision ${state.savedRevision} · Approved focus`);
      await expect(showMenu).toContainText(`autosaved to this copy, not to ${state.sourceName}`);

      await showMenu.getByRole("button", { name: "Save", exact: true }).click();
      const manualSave = page.getByRole("dialog", { name: "Save revision copy" });
      await expect(manualSave.getByRole("button", { name: "Keep as Separate Show" })).toBeVisible();
      await expect(manualSave.getByRole("button", { name: "Overwrite Original Show" })).toBeVisible();
      await manualSave.getByRole("button", { name: "Overwrite Original Show" }).click();
      const confirmation = page.getByRole("alertdialog", { name: new RegExp(`Confirm overwrite ${escapeRegex(state.sourceName)}`) });
      await expect(confirmation).toContainText("identity and named revisions are preserved");
      await confirmation.getByRole("button", { name: "Cancel" }).click();
      expect((await showObject(api, state.sourceId, "group", "4")).body.name).toBe("Newer autosave state");

      await showMenu.getByRole("button", { name: "Save", exact: true }).click();
      await page.getByRole("dialog", { name: "Save revision copy" }).getByRole("button", { name: "Overwrite Original Show" }).click();
      await page.getByRole("alertdialog").getByRole("button", { name: new RegExp(`Replace ${escapeRegex(state.sourceName)} Latest Autosave`) }).click();
      await expect.poll(async () => (await showObject(api, state.sourceId, "group", "4")).body.name).toBe("Named revision state");

      state.copyId = copy.id;
      state.copyProvenance = copy.revision_copy;
      state.expectedSourceName = "Named revision state";
      state.expectedCopyName = "Named revision state";
      state.expectedCopyRevisions = [];
    },
    assert: async ({ api }, state) => {
      expect(state.copyId).toBeTruthy();
      expect(state.copyProvenance).toMatchObject({ show_id: state.sourceId, show_name: state.sourceName, revision: state.savedRevision, revision_name: "Approved focus" });
      expect((await showObject(api, state.sourceId, "group", "4")).body.name).toBe(state.expectedSourceName);
      expect((await showObject(api, state.copyId!, "group", "4")).body.name).toBe(state.expectedCopyName);
      expect((await api.request<any[]>("GET", `/api/v1/shows/${state.sourceId}/revisions`)).map((entry) => entry.name)).toEqual(["Approved focus"]);
      expect((await api.request<any[]>("GET", `/api/v1/shows/${state.copyId}/revisions`)).map((entry) => entry.name)).toEqual(state.expectedCopyRevisions);
      expect((await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).active_show.id).toBe(state.copyId);
      expect((await api.request<any[]>("GET", "/api/v1/shows", undefined, false)).some((entry) => entry.id === state.copyId)).toBe(true);
    },
  });
});

async function assertZeroTicks(api: ApiDriver, bench: LightBench): Promise<void> {
  const before = behaviorTimestamps(await programmer(api));
  const osc = await bench.osc();
  const deskAlias = api.session!.desk.osc_alias;
  const pageFeedback = `/light/${deskAlias}/feedback/page`;
  const clientId = `time-001-${crypto.randomUUID()}`;
  try {
    await osc.subscribe(clientId, deskAlias);
    // Subscription feedback is a full asynchronous burst. Drain it before marking the two
    // explicitly clocked cycles so UDP delivery already in flight cannot be misattributed.
    await new Promise<void>((resolve) => setTimeout(resolve, 75));
    const bursts: Array<{ now: string; artnetSequence: number; sacnSequence: number }> = [];
    for (let call = 0; call < 2; call += 1) {
      const artnetMark = bench.artnet.mark();
      const sacnMark = bench.sacn.mark();
      const oscMark = osc.mark();
      const frame = await bench.tick(0);
      expect(frame).toMatchObject({ now: FIXED_NOW, packets_sent: 2 });
      const artnet = await bench.artnet.nextAfter(artnetMark, "artnet", 1);
      const sacn = await bench.sacn.nextAfter(sacnMark, "sacn", 101);
      await osc.expectAfter(oscMark, pageFeedback);
      await new Promise<void>((resolve) => setTimeout(resolve, 20));
      expect(bench.artnet.packets.slice(artnetMark).filter((packet) => packet.protocol === "artnet" && packet.universe === 1)).toHaveLength(1);
      expect(bench.sacn.packets.slice(sacnMark).filter((packet) => packet.protocol === "sacn" && packet.universe === 101)).toHaveLength(1);
      expect(osc.messages.slice(oscMark).filter((message) => message.address === pageFeedback)).toHaveLength(1);
      expect(Array.from(artnet.slots.slice(0, 12))).toEqual([128, ...Array(11).fill(0)]);
      expect(Array.from(sacn.slots.slice(0, 12))).toEqual([128, ...Array(11).fill(0)]);
      bursts.push({ now: frame.now, artnetSequence: artnet.sequence, sacnSequence: sacn.sequence });
    }
    expect(bursts.map((burst) => burst.now)).toEqual([FIXED_NOW, FIXED_NOW]);
    expectSequenceIncrement(bursts[0].artnetSequence, bursts[1].artnetSequence);
    expectSequenceIncrement(bursts[0].sacnSequence, bursts[1].sacnSequence);
    expect(behaviorTimestamps(await programmer(api))).toEqual(before);
  } finally {
    await osc.send("/light/unsubscribe", [clientId]).catch(() => undefined);
    await osc.close();
  }
}

async function connectHardware(api: ApiDriver, bench: LightBench, state: HardwareState, prefix: string): Promise<void> {
  state.hardware = await bench.osc();
  state.hardwareClientId = `${prefix}-${crypto.randomUUID()}`;
  await state.hardware.subscribe(state.hardwareClientId, api.session!.desk.osc_alias);
  await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(true);
}

async function disconnectHardware(api: ApiDriver, state: HardwareState): Promise<void> {
  if (!state.hardware || !state.hardwareClientId) return;
  await state.hardware.send("/light/unsubscribe", [state.hardwareClientId]);
  await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(false);
  await state.hardware.close();
}

async function assertFadeBoundaries(api: ApiDriver, bench: LightBench, fixtureId: string): Promise<void> {
  const increments = [0, 1, 1_498, 1, 1, 1_498, 1, 1];
  const checkpoints = [0, 1, 1_499, 1_500, 1_501, 2_999, 3_000, 3_001];
  const levels: number[] = [];
  for (let index = 0; index < increments.length; index += 1) {
    const artnetMark = bench.artnet.mark();
    const sacnMark = bench.sacn.mark();
    const frame = await bench.tick(increments[index]);
    const level = await visualizationLevel(api, fixtureId, "intensity");
    const expectedLevel = Math.min(checkpoints[index] / 3_000, 1);
    const expectedByte = Math.round(expectedLevel * 255);
    levels.push(level);
    expect(level).toBeCloseTo(expectedLevel, 6);
    expect(slot(frame, 1)).toBe(expectedByte);
    const artnet = await bench.artnet.nextAfter(artnetMark, "artnet", 1);
    const sacn = await bench.sacn.nextAfter(sacnMark, "sacn", 101);
    expect(artnet.slots[0]).toBe(expectedByte);
    expect(sacn.slots[0]).toBe(expectedByte);
  }
  expect(levels).toEqual([...levels].sort((left, right) => left - right));
  expect(levels[3]).toBeCloseTo(0.5, 8);
  expect(Math.round(levels[3] * 255)).toBe(128);
  expect(levels[6]).toBe(1);
  expect(levels[7]).toBe(1);
}

async function expectEncoderTarget(page: Page, percent: number): Promise<void> {
  const encoder = page.locator(".vertical-touch-fader-stack").filter({
    hasText: "Enc 1 · Dimmer",
  });
  await expect(encoder.locator(".vertical-touch-fader > strong")).toHaveText(`${percent}%`);
}

async function expectFixtureSheetDimmer(page: Page, fixtureNumber: number, percent: number): Promise<void> {
  await expect(fixtureRow(page, fixtureNumber).getByRole("cell").nth(2)).toContainText(`${percent}%`);
}

async function recordFirstCuelistThroughUi(api: ApiDriver, page: Page): Promise<any> {
  await openBuiltIn(page, "Cuelists");
  await page.locator(".global-store-button").click();
  await expect(page.locator(".global-store-button")).toHaveText("REC ARMED");
  await page.locator(".cuelist-card").first().click();
  await expect.poll(async () => {
    const playbacks = await api.request<any>("GET", "/api/v1/playbacks");
    return playbacks.pool.some((definition: any) => definition.number === 1);
  }).toBe(true);
  const playbacks = await api.request<any>("GET", "/api/v1/playbacks");
  const definition = playbacks.pool.find((candidate: any) => candidate.number === 1);
  const cueList = playbacks.cue_lists.find(
    (candidate: any) => candidate.id === definition?.target?.cue_list_id,
  );
  expect(cueList?.cues).toHaveLength(1);
  return cueList.cues[0];
}

async function assertCueReplayBoundaries(
  api: ApiDriver,
  bench: LightBench,
  page: Page,
  desk: DeskDriver,
  targets: Array<{ fixtureId: string; number: number; slot: number }>,
): Promise<void> {
  const increments = [0, 1, 1_498, 1, 1, 1_498, 1, 1];
  const checkpoints = [0, 1, 1_499, 1_500, 1_501, 2_999, 3_000, 3_001];
  const observed = new Map<string, number[]>();
  for (let index = 0; index < increments.length; index += 1) {
    const artnetMark = bench.artnet.mark();
    const sacnMark = bench.sacn.mark();
    const frame = await bench.tick(increments[index]);
    const expectedLevel = Math.min(checkpoints[index] / 3_000, 1);
    const expectedByte = Math.round(expectedLevel * 255);
    const expectedPercent = Math.round(expectedLevel * 100);
    for (const target of targets) {
      const level = await visualizationLevel(api, target.fixtureId, "intensity");
      observed.set(target.fixtureId, [...(observed.get(target.fixtureId) ?? []), level]);
      expect(level).toBeCloseTo(expectedLevel, 6);
      expect(slot(frame, target.slot)).toBe(expectedByte);
      await expectFixtureSheetDimmer(page, target.number, expectedPercent);
    }
    const artnet = await bench.artnet.nextAfter(artnetMark, "artnet", 1);
    const sacn = await bench.sacn.nextAfter(sacnMark, "sacn", 101);
    for (const target of targets) {
      expect(artnet.slots[target.slot - 1]).toBe(expectedByte);
      expect(sacn.slots[target.slot - 1]).toBe(expectedByte);
    }
    await desk.recordStep(
      `CUE FADE · ${checkpoints[index]} ms · ${expectedPercent}%`,
      `Fixture Sheet, resolved engine value, logical DMX, Art-Net, and sACN agree at ${expectedPercent}% (${expectedByte}/255).`,
    );
  }
  for (const levels of observed.values()) {
    expect(levels).toEqual([...levels].sort((left, right) => left - right));
    expect(levels[3]).toBeCloseTo(0.5, 8);
    expect(levels[6]).toBe(1);
    expect(levels[7]).toBe(1);
  }
}

async function setProgrammerFadeThroughUi(api: ApiDriver, page: Page, seconds: number): Promise<void> {
  await page.locator(".hardware-control-summary").getByRole("button").filter({ hasText: "Prog Fade" }).click();
  const dialog = page.locator(".direct-value-modal").filter({ hasText: "Prog. Fade" });
  await expect(dialog).toBeVisible();
  await page.keyboard.type(String(seconds));
  await page.keyboard.press("Enter");
  await expect(dialog).toBeHidden();
  await expect.poll(async () => {
    const response = await api.request<any>("GET", "/api/v1/configuration");
    return (response.configuration ?? response).programmer_fade_millis;
  }).toBe(seconds * 1_000);
}

async function programShow001ThroughUi(api: ApiDriver, page: Page, revisionName: string): Promise<void> {
  await openFixtures(page);
  await fixtureRow(page, 5).click();
  await fixtureRow(page, 6).click();
  await openGroups(page);
  await page.locator(".global-store-button").click();
  await groupCard(page, 3).click();
  const mode = page.locator(".record-mode-dialog");
  await expect(mode).toBeVisible();
  await mode.getByRole("button", { name: "Merge", exact: true }).click();
  await expect(mode).toBeHidden();
  const fixtures = await fixtureIdsByNumber(api);
  await expect.poll(async () => (await object<any>(api, "group", "3")).body.fixtures)
    .toEqual([1, 2, 3, 4, 5, 6].map((number) => fixtures[number]));

  await groupCard(page, 3).click();
  await setDimmerByTouch(page, 40);
  await expect.poll(async () => normalized((await programmer(api)).group_values["3"]?.intensity?.value)).toBe(0.4);
  await openCuelistPool(page);
  await page.locator(".global-store-button").click();
  await expect(page.locator(".global-store-button")).toHaveText("REC ARMED");
  const cuelist = page.locator(".cuelist-card").first();
  await cuelist.click();
  await expect.poll(async () => (await api.request<any>("GET", "/api/v1/playbacks")).pool.some((playback: any) => playback.number === 1)).toBe(true);

  await page.getByRole("button", { name: "SET", exact: true }).click();
  await cuelist.click();
  await page.locator(".mode-toggle").click();
  await page.getByRole("button", { name: "Assign Cuelist 1 to page 1 playback 1" }).click();
  await expect.poll(async () => {
    const playbacks = await api.request<any>("GET", "/api/v1/playbacks");
    return playbacks.pages.find((candidate: any) => candidate.number === 1)?.slots?.["1"];
  }).toBe(1);
  await page.locator(".mode-toggle").click();
  await page.getByRole("button", { name: "CLR", exact: true }).click();
  await page.getByRole("button", { name: "CLR", exact: true }).click();
  await cuelist.click();
  await expect(page.locator(".cue-table")).toBeVisible();
  await page.locator(".mode-toggle").click();
  await page.locator('.playback-fader-bank article[data-page="1"][data-playback-slot="1"]').getByRole("button", { name: "GO +", exact: true }).click();
  await page.locator(".mode-toggle").click();

  await openFixtures(page);
  await fixtureRow(page, 12).click();
  await setDimmerByTouch(page, 65);
  await expect.poll(async () => {
    const fixtureId = (await fixtureIdsByNumber(api))[12];
    return normalized((await programmer(api)).values.find((value: any) => value.fixture_id === fixtureId && value.attribute === "intensity")?.value);
  }).toBe(0.65);
  await saveNamedRevisionThroughUi(page, revisionName);
}

async function assertShow001State(
  api: ApiDriver,
  bench: LightBench,
  state: { copyId: string; fixtureIds: Record<number, string>; revisionName: string },
): Promise<void> {
  expect((await object<any>(api, "group", "3")).body.fixtures).toEqual([1, 2, 3, 4, 5, 6].map((number) => state.fixtureIds[number]));
  const playbacks = await api.request<any>("GET", "/api/v1/playbacks");
  const definition = playbacks.pool.find((playback: any) => playback.number === 1);
  expect(definition?.target.type).toBe("cue_list");
  const cueList = playbacks.cue_lists.find((candidate: any) => candidate.id === definition.target.cue_list_id);
  expect(cueList?.cues).toHaveLength(1);
  const groupChange = cueList.cues[0].group_changes[0];
  expect(groupChange).toMatchObject({
    group_id: "3",
    attribute: "intensity",
    value: { kind: "normalized" },
  });
  expect(groupChange.value.value).toBeCloseTo(0.4, 6);
  expect(playbacks.active.find((runtime: any) => runtime.playback_number === 1)).toMatchObject({ current_cue_number: 1, enabled: true });
  const durable = await programmer(api);
  expect(durable.values.find((value: any) => value.fixture_id === state.fixtureIds[12] && value.attribute === "intensity")?.value).toMatchObject({ value: 0.65 });
  const revisions = await api.request<any[]>("GET", `/api/v1/shows/${state.copyId}/revisions`);
  expect(revisions.some((revision) => revision.name === state.revisionName)).toBe(true);
  const frame = await bench.tick(3_000);
  expect(frame.universes.find((universe) => universe.universe === 1)?.slots.slice(0, 6)).toEqual(Array(6).fill(102));
  expect(slot(frame, 12)).toBe(166);
}

async function arrangeMalformedRecovery(api: ApiDriver, bench: LightBench, surface: string) {
  const damaged = await loadCanonicalCopy(api, bench, `show-003-${surface}`);
  const entry = await showEntry(api, damaged.id);
  const response = await fetch(`${api.baseUrl}/api/v1/shows/${damaged.id}/download`, {
    headers: { authorization: `Bearer ${api.session?.token}` },
  });
  expect(response.ok).toBe(true);
  const recoveryShowName = `show-003-valid-${surface}-${crypto.randomUUID()}`;
  const recovery = await api.request<{ id: string }>("POST", "/api/v1/shows", {
    name: recoveryShowName,
    data_base64: Buffer.from(await response.arrayBuffer()).toString("base64"),
    overwrite: false,
  });
  await bench.stopServerGracefully(api.session!.token);
  await fs.writeFile(entry.path, Buffer.from("not a ToskLight SQLite show\n"));
  const damagedHash = await fileHash(entry.path);
  await bench.startServer();
  await api.login("Operator");
  const readiness = await api.request<any>("GET", "/api/v1/readiness", undefined, false);
  expect(readiness).toMatchObject({ status: "ready", recovery_mode: true });
  return {
    damagedPath: entry.path,
    damagedHash,
    damagedShowId: damaged.id,
    recoveryShowId: recovery.id,
    recoveryShowName,
  };
}

async function prepareMigrationCase(api: ApiDriver, bench: LightBench, migration: Show004Case) {
  const copy = await loadCanonicalCopy(api, bench, `show-004-${migration}`);
  let cueListId: string | undefined;
  if (migration === "playback-defaults" || migration === "cue-defaults") {
    const fixtures = await fixtureIdsByNumber(api);
    cueListId = await installSequence(api, fixtures[1]);
    if (migration === "playback-defaults") await putObject(api, "playback", "1", playback(1, cueListId, "Legacy playback"));
  }
  return { entry: await showEntry(api, copy.id), cueListId };
}

async function stageLegacyMigration(file: string, migration: Show004Case, cueListId?: string): Promise<void> {
  if (migration === "fixture-number") {
    await runSql(file, "UPDATE objects SET body_json=json_remove(body_json, '$.fixture_number') WHERE kind='patched_fixture'");
  } else if (migration === "group-defaults") {
    await runSql(file, "UPDATE objects SET body_json=json_remove(body_json, '$.color', '$.icon', '$.derived_from', '$.frozen_from', '$.programming', '$.master', '$.playback_fader') WHERE kind='group' AND id='3'");
  } else if (migration === "playback-defaults") {
    await runSql(file, "UPDATE objects SET body_json=json_remove(body_json, '$.buttons', '$.button_count', '$.fader', '$.has_fader', '$.go_activates', '$.auto_off', '$.xfade_millis', '$.color', '$.flash_release', '$.protect_from_swap', '$.presentation_icon', '$.presentation_image') WHERE kind='playback' AND id='1'");
  } else if (migration === "route-defaults") {
    await runSql(file, "UPDATE objects SET body_json=json_remove(body_json, '$.destination') WHERE kind='route'");
  } else if (migration === "virtual-dimmer-metadata") {
    await runSql(file, "UPDATE objects SET body_json=json_remove(body_json, '$.definition.heads[0].parameters[0].metadata', '$.definition.heads[0].parameters[0].capabilities') WHERE kind='patched_fixture' AND json_extract(body_json, '$.fixture_number')=21");
  } else {
    if (!cueListId) throw new Error("cue-defaults migration needs a Cuelist");
    await runSql(file, `UPDATE objects SET body_json=json_remove(body_json, '$.cues[0].id', '$.cues[1].id', '$.intensity_priority_mode', '$.wrap_mode', '$.restart_mode', '$.force_cue_timing', '$.disable_cue_timing', '$.chaser_xfade_millis', '$.speed_multiplier') WHERE kind='cue_list' AND id='${cueListId}'`);
  }
}

async function migrationSnapshot(api: ApiDriver, migration: Show004Case, cueListId?: string): Promise<any> {
  if (migration === "fixture-number") {
    const fixtures = await api.request<any[]>("GET", `/api/v1/shows/${await activeShowId(api)}/objects/patched_fixture`, undefined, false);
    return fixtures.map((fixture) => ({ id: fixture.id, revision: fixture.revision, name: fixture.body.name, fixture_number: fixture.body.fixture_number })).sort((left, right) => left.fixture_number - right.fixture_number);
  }
  if (migration === "group-defaults") return object<any>(api, "group", "3");
  if (migration === "playback-defaults") return object<any>(api, "playback", "1");
  if (migration === "route-defaults") return object<any>(api, "route", "artnet");
  if (migration === "virtual-dimmer-metadata") {
    const fixtures = await api.request<any[]>("GET", `/api/v1/shows/${await activeShowId(api)}/objects/patched_fixture`, undefined, false);
    return fixtures.find((fixture) => fixture.body.fixture_number === 21);
  }
  if (!cueListId) throw new Error("cue-defaults migration needs a Cuelist");
  return object<any>(api, "cue_list", cueListId);
}

function assertMigrationSnapshot(migration: Show004Case, snapshot: any): void {
  if (migration === "fixture-number") {
    expect(snapshot.map((fixture: any) => fixture.fixture_number)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 21, 22, 23, 24]);
  } else if (migration === "group-defaults") {
    expect(snapshot.body).toMatchObject({ color: null, icon: null, derived_from: null, frozen_from: null, programming: {}, master: 1, playback_fader: null });
  } else if (migration === "playback-defaults") {
    expect(snapshot.body).toMatchObject({ buttons: ["go_minus", "go", "flash"], button_count: 3, fader: "master", has_fader: true, go_activates: true, auto_off: true, xfade_millis: 0, color: "#20c997", flash_release: "release_all", protect_from_swap: false });
  } else if (migration === "route-defaults") {
    expect(snapshot.body).toMatchObject({ protocol: "art_net", logical_universe: 1, destination_universe: 1, destination: null, enabled: true });
  } else if (migration === "virtual-dimmer-metadata") {
    const intensity = snapshot.body.definition.heads[0].parameters.find((parameter: any) => parameter.attribute === "intensity");
    expect(intensity).toMatchObject({ virtual_dimmer: true, capabilities: [], metadata: { physical_min: 0, physical_max: 1, unit: null, invert: false, wrap: false, curve: "linear" } });
  } else {
    expect(snapshot.body).toMatchObject({ intensity_priority_mode: "htp", restart_mode: "first_cue", force_cue_timing: false, disable_cue_timing: false, chaser_xfade_millis: 0, speed_multiplier: 1 });
    expect(snapshot.body.cues.map((cue: any) => cue.id)).toHaveLength(2);
    expect(snapshot.body.cues.every((cue: any) => /^[0-9a-f-]{36}$/.test(cue.id))).toBe(true);
  }
}

async function openBuiltIn(page: Page, name: string): Promise<void> {
  const entry = page.locator(".dock-entry").filter({ hasText: name }).first();
  if (!await entry.isVisible()) await page.getByRole("button", { name: "BUILT-INS", exact: true }).click();
  await expect(entry).toBeVisible();
  await entry.click();
}

async function openFixtures(page: Page): Promise<void> {
  await openBuiltIn(page, "Fixtures");
  await expect(page.locator(".fixture-window")).toBeVisible();
}

function fixtureRow(page: Page, number: number) {
  return page.locator(".fixture-window .ui-data-table-row:not(.header)")
    .filter({ has: page.getByRole("cell", { name: String(number), exact: true }) })
    .first();
}

async function openShiftedWindow(page: Page, key: string, windowSelector: string): Promise<void> {
  if (await page.locator(windowSelector).isVisible()) return;
  const shift = page.getByRole("button", { name: "SHIFT", exact: true });
  if (!await shift.isVisible()) await page.locator(".mode-toggle").click();
  await shift.click();
  await page.getByRole("button", { name: key, exact: true }).click();
  await expect(page.locator(windowSelector)).toBeVisible();
}

async function openGroups(page: Page): Promise<void> {
  await openShiftedWindow(page, "1", ".group-pool-window");
}

async function openCuelistPool(page: Page): Promise<void> {
  await openShiftedWindow(page, "4", ".cuelist-pool-window");
}

function groupCard(page: Page, number: number) {
  return page.locator(".group-pool-window .group-card").nth(number - 1);
}

async function setDimmerByTouch(page: Page, value: number): Promise<void> {
  const encoder = page.locator(".vertical-touch-fader-stack").filter({ hasText: "Enc 1 · Dimmer" });
  await encoder.getByRole("button", { name: "Set value" }).click();
  const dialog = page.getByRole("dialog", { name: "Enc 1 · Dimmer value" });
  await expect(dialog).toBeVisible();
  await page.keyboard.type(String(value));
  await page.keyboard.press("Enter");
  await expect(dialog).toBeHidden();
}

async function saveNamedRevisionThroughUi(page: Page, name: string): Promise<void> {
  await page.getByRole("button", { name: /Open show menu/ }).click();
  await page.getByRole("button", { name: "Save Named Revision", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Save named revision" });
  await dialog.getByLabel("Revision name").fill(name);
  await dialog.getByRole("button", { name: /^Save Revision/ }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator(".show-details")).toContainText(name);
  await page.getByRole("button", { name: "Close Show", exact: true }).click();
}

async function setProgrammerFade(
  api: ApiDriver,
  millis: number,
  sequenceMasterFadeMillis?: number,
): Promise<void> {
  const response = await api.request<any>("GET", "/api/v1/configuration");
  const configuration = response.configuration ?? response;
  await api.request("PUT", "/api/v1/configuration", {
    ...configuration,
    programmer_fade_millis: millis,
    ...(sequenceMasterFadeMillis == null
      ? {}
      : { sequence_master_fade_millis: sequenceMasterFadeMillis }),
  });
}

async function setSpeedGroups(api: ApiDriver, speedGroups: number[]): Promise<void> {
  const response = await api.request<any>("GET", "/api/v1/configuration");
  const configuration = response.configuration ?? response;
  await api.request("PUT", "/api/v1/configuration", { ...configuration, speed_groups_bpm: speedGroups, sequence_master_fade_millis: 0 });
}

function behaviorTimestamps(state: any): unknown {
  return {
    last_activity: state.last_activity,
    values: state.values.map((value: any) => value.changed_at),
    groups: Object.fromEntries(Object.entries(state.group_values ?? {}).map(([group, attributes]: [string, any]) => [group, Object.fromEntries(Object.entries(attributes).map(([attribute, value]: [string, any]) => [attribute, value.changed_at]))])),
  };
}

function expectSequenceIncrement(before: number, after: number): void {
  expect(after).toBe(before >= 255 ? 1 : before + 1);
}

function slot(frame: { universes: Array<{ universe: number; slots: number[] }> }, address: number): number | undefined {
  return frame.universes.find((universe) => universe.universe === 1)?.slots[address - 1];
}

async function visualizationLevel(api: ApiDriver, fixtureId: string, attribute: string): Promise<number> {
  const visualization = await api.request<any>("GET", "/api/v1/visualization");
  return normalized(visualization.values.find((item: any) => item.fixture_id === fixtureId && item.attribute === attribute)?.value) ?? 0;
}

async function installTimeCuelists(api: ApiDriver, chaserFixture: string, phaserFixture: string): Promise<string> {
  const chaserId = crypto.randomUUID();
  await putObject(api, "cue_list", chaserId, {
    id: chaserId, name: "Virtual Chaser", priority: 0, mode: "chaser", looped: true, chaser_step_millis: 1_000,
    speed_group: "A", intensity_priority_mode: "htp", wrap_mode: "tracking", restart_mode: "first_cue",
    force_cue_timing: false, disable_cue_timing: false, chaser_xfade_millis: 0, speed_multiplier: 1,
    cues: [0.25, 0.5, 0.75, 1].map((level, index) => cue(index + 1, chaserFixture, level)),
  });
  const phaserId = crypto.randomUUID();
  const phaserCue = cue(1, phaserFixture, 0);
  phaserCue.phasers = [{
    fixture_ids: [phaserFixture], group_ids: [], attribute: "intensity",
    phaser: { mode: "absolute", steps: [{ position: 0, value: 0, curve_to_next: "linear" }, { position: 0.5, value: 1, curve_to_next: "linear" }], cycles_per_minute: 60, phase_start_degrees: 0, phase_end_degrees: 0, width: 1 },
  }];
  await putObject(api, "cue_list", phaserId, {
    id: phaserId, name: "Virtual Phaser", priority: 1, mode: "sequence", looped: false, chaser_step_millis: 1_000,
    speed_group: null, intensity_priority_mode: "htp", wrap_mode: "off", restart_mode: "first_cue",
    force_cue_timing: false, disable_cue_timing: true, chaser_xfade_millis: 0, speed_multiplier: 1, cues: [phaserCue],
  });
  await putObject(api, "playback", "1", playback(1, chaserId, "Virtual Chaser"));
  await putObject(api, "playback", "2", playback(2, phaserId, "Virtual Phaser"));
  await putObject(api, "playback", "3", {
    number: 3, name: "Dynamics Control", target: { type: "grand_master" }, buttons: ["blackout", "flash", "pause_dynamics"], button_count: 3,
    fader: "master", has_fader: true, go_activates: true, auto_off: true, xfade_millis: 0, color: "#20c997", flash_release: "release_all", protect_from_swap: false,
  });
  return chaserId;
}

async function restartPlaybackRun(api: ApiDriver, bench: LightBench, showId: string, numbers: number[]): Promise<void> {
  for (const number of [1, 2]) await api.request("POST", `/api/v1/cuelists/${number}/off`, {}).catch(() => undefined);
  await api.request("POST", `/api/v1/shows/${showId}/open`, { transition: "hold_current" });
  await api.request("POST", "/api/v1/test/clock/reset", undefined, false);
  for (const number of numbers) await api.request("POST", `/api/v1/cuelists/${number}/go`, {});
}

async function playbackRuntime(api: ApiDriver, number: number): Promise<any> {
  const state = await api.request<any>("GET", "/api/v1/playbacks");
  const runtime = state.active.find((item: any) => item.playback_number === number);
  expect(runtime).toBeDefined();
  return runtime;
}

async function installGroupCue(api: ApiDriver, groupId: string, level: number): Promise<string> {
  const id = crypto.randomUUID();
  const first = cue(1, (await fixtureIdsByNumber(api))[1], 0);
  first.changes = [];
  first.group_changes = [{ group_id: groupId, attribute: "intensity", value: { kind: "normalized", value: level } }];
  await putObject(api, "cue_list", id, sequence(id, "SHOW-001 Cuelist", [first]));
  await putObject(api, "playback", "1", playback(1, id, "SHOW-001 Playback"));
  return id;
}

async function installSequence(api: ApiDriver, fixtureId: string): Promise<string> {
  const id = crypto.randomUUID();
  await putObject(api, "cue_list", id, sequence(id, "Legacy migration", [cue(1, fixtureId, 0.25), cue(2, fixtureId, 0.75)]));
  return id;
}

function sequence(id: string, name: string, cues: any[]): any {
  return { id, name, priority: 0, mode: "sequence", looped: false, chaser_step_millis: 1_000, speed_group: null, intensity_priority_mode: "htp", wrap_mode: "off", restart_mode: "first_cue", force_cue_timing: false, disable_cue_timing: false, chaser_xfade_millis: 0, speed_multiplier: 1, cues };
}

function cue(number: number, fixtureId: string, level: number): any {
  return { id: crypto.randomUUID(), number, name: `Cue ${number}`, changes: [{ fixture_id: fixtureId, attribute: "intensity", value: { kind: "normalized", value: level }, automatic_restore: false }], group_changes: [], fade_millis: 0, delay_millis: 0, trigger: { type: "manual" }, phasers: [] };
}

function playback(number: number, cueListId: string, name: string): any {
  return { number, name, target: { type: "cue_list", cue_list_id: cueListId }, buttons: ["go_minus", "go", "flash"], button_count: 3, fader: "master", has_fader: true, go_activates: true, auto_off: false, xfade_millis: 0, color: "#20c997", flash_release: "release_all", protect_from_swap: false };
}

async function showEntry(api: ApiDriver, id: string): Promise<any> {
  const entries = await api.request<any[]>("GET", "/api/v1/shows");
  const entry = entries.find((candidate) => candidate.id === id);
  expect(entry).toBeDefined();
  return entry;
}

async function showObject(api: ApiDriver, showId: string, kind: string, id: string): Promise<any> {
  const entries = await api.request<any[]>("GET", `/api/v1/shows/${showId}/objects/${kind}`, undefined, false);
  const entry = entries.find((candidate) => candidate.id === id);
  expect(entry).toBeDefined();
  return entry;
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

type LegacyFixtureRow = { definition: Record<string, any>; source: Buffer };

async function fixtureDefinitions(api: ApiDriver): Promise<any[]> {
  return api.request<any[]>("GET", "/api/v1/fixture-library", undefined, false);
}

async function fixtureProfiles(api: ApiDriver): Promise<any[]> {
  return api.request<any[]>("GET", "/api/v1/fixture-profiles", undefined, false);
}

async function fixtureProfileWarnings(api: ApiDriver): Promise<string[]> {
  return api.request<string[]>("GET", "/api/v1/fixture-profiles/warnings", undefined, false);
}

function reservedGenericProfileSnapshot(profiles: any[]): any[] {
  return profiles
    .filter((profile) => profile.reserved_source === "builtin:generic-catalog")
    .map((profile) => ({
      id: profile.id,
      revision: profile.revision,
      manufacturer: profile.manufacturer,
      name: profile.name,
      modes: profile.modes.map((mode: any) => ({ id: mode.id, name: mode.name })),
    }))
    .sort((left, right) => left.id.localeCompare(right.id));
}

async function insertLegacyFixtureRows(database: string, rows: LegacyFixtureRow[]): Promise<void> {
  await runSql(database, rows.map((row) => {
    const definition = row.definition;
    return `INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json,source_gdtf) VALUES(${sqlString(definition.id)},${Number(definition.revision)},${sqlString(definition.manufacturer)},${sqlString(definition.model)},${sqlString(definition.mode)},${sqlString(JSON.stringify(definition))},X'${row.source.toString("hex")}')`;
  }).join(";"));
}

async function legacyFixtureRow(database: string, id: string): Promise<{ json: string; sourceHex: string }> {
  const encoded = await readSql(database, `SELECT hex(definition_json)||'|'||COALESCE(hex(source_gdtf),'') FROM fixture_definitions WHERE id=${sqlString(id)} AND revision=1`);
  const [jsonHex, sourceHex] = encoded.split("|");
  return { json: Buffer.from(jsonHex, "hex").toString("utf8"), sourceHex };
}

async function fixtureProfileMigrationSnapshot(database: string, profileId: string): Promise<string> {
  return readSql(database, `SELECT hex(profile_json)||':'||(SELECT COUNT(*) FROM fixture_profile_legacy_map WHERE profile_id=p.id AND profile_revision=p.revision)||':'||(SELECT COUNT(*) FROM fixture_profile_legacy_sources WHERE profile_id=p.id AND profile_revision=p.revision) FROM fixture_profiles p WHERE p.id=${sqlString(profileId)} AND p.revision=1`);
}

async function fixtureWarningSnapshot(database: string, family: string, malformedId: string): Promise<string> {
  return readSql(database, `SELECT group_concat(hex(message),'|') FROM (SELECT message FROM fixture_library_warnings WHERE message LIKE ${sqlString(`%${family}%`)} OR message LIKE ${sqlString(`%${malformedId}%`)} ORDER BY message)`);
}

function sqlString(value: string): string {
  return `'${value.replaceAll("'", "''")}'`;
}

async function runSql(file: string, sql: string): Promise<void> {
  await sqlite("sqlite3", [file, sql]);
}

async function readSql(file: string, sql: string): Promise<string> {
  const { stdout } = await sqlite("sqlite3", ["-noheader", file, sql]);
  return stdout.trim();
}

async function fileHash(file: string): Promise<string> {
  return hash(await fs.readFile(file));
}

function hash(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}
