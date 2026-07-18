import type { ApiDriver, Session } from "../apps/control-ui/e2e/bench/api";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import type { OscHardware } from "../apps/control-ui/e2e/bench/protocols";
import type { Locator, Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import {
  activeShowId,
  command,
  expectProgrammer,
  fixtureIdsByNumber,
  groupNumbers,
  loadCanonicalCopy,
  object,
  objects,
  pressCommand,
  putObject,
} from "./support/catalog";

interface OscSubscriptionState {
  clientId: string;
  session?: Session;
  hardware?: OscHardware;
}

interface OscIsolationState {
  first?: Session;
  second: Session;
  firstHardware?: OscHardware;
  secondHardware?: OscHardware;
  firstMark?: number;
  secondMark?: number;
}

interface SharedProgrammerState {
  first?: Session;
  second: Session;
  auditBefore: number;
}

interface PagePlaybackState {
  firstCueListId: string;
  secondCueListId: string;
  session?: Session;
}

interface RevisionConflictState {
  originalRevision: number;
  originalBody: Record<string, any>;
  successfulBody: Record<string, any>;
}

interface CrossMutationState {
  originalRevision: number;
  fixtureFive: string;
  mutationRevision?: number;
}

test.describe("docs/testing/04-osc-api-and-cross-surface.md", () => {
  pairedScenario<OscSubscriptionState>({
    id: "OSC-001",
    title: "page changes produce one complete feedback cycle without periodic mutation",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `osc-001-${surface}`);
      await installPlayback(api);
      return { clientId: `osc-001-${surface}-${crypto.randomUUID()}` };
    },
    api: async ({ api, bench }, state) => {
      state.session = api.session!;
      state.hardware = await bench.osc();
      await state.hardware.subscribe(state.clientId, state.session.desk.osc_alias);
      await state.hardware.expectAfter(0, `/light/${state.session.desk.osc_alias}/feedback/speed-group/5`);
      await setDeskPage(api, state.session, 2);
    },
    ui: async ({ bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      state.session = await browserSession(page);
      state.hardware = await bench.osc();
      await state.hardware.subscribe(state.clientId, state.session.desk.osc_alias);
      await state.hardware.expectAfter(0, `/light/${state.session.desk.osc_alias}/feedback/speed-group/5`);
      await selectPlaybackPage(page, "Page 2");
    },
    assert: async ({ api, bench }, state) => {
      const session = state.session!;
      const hardware = state.hardware!;
      const alias = session.desk.osc_alias;
      try {
        expect(await deskPage(api, session)).toBe(2);
        expect((await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(true);

        const mark = hardware.mark();
        await bench.tick(0);
        const pageFeedback = await hardware.expectAfter(mark, `/light/${alias}/feedback/page`);
        expect(pageFeedback.arguments).toEqual([2]);
        for (const address of [
          `/light/${alias}/feedback/command-line`,
          `/light/${alias}/feedback/programmer/group`,
          `/light/${alias}/feedback/page-playback/1/fader`,
          `/light/${alias}/feedback/page-playback/1/button/1`,
          `/light/${alias}/feedback/speed-group/1`,
          `/light/${alias}/feedback/speed-group/5`,
        ]) await hardware.expectAfter(mark, address);

        const quietMark = hardware.mark();
        await new Promise((resolve) => setTimeout(resolve, 75));
        expect(hardware.messages.slice(quietMark)).toHaveLength(0);
        await hardware.send("/light/unsubscribe", [state.clientId]);
        await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(false);
      } finally {
        await unsubscribeAndClose(hardware, state.clientId);
      }
    },
  });

  registerGroupOutputPair("OSC-002", 25, 64, "hardware-equivalent command reaches shared programmer and output");

  pairedScenario<OscIsolationState>({
    id: "OSC-003",
    title: "separate desk subscribers isolate partial commands and unsubscribe independently",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `osc-003-${surface}`);
      const second = await createSession(api, crypto.randomUUID());
      await withSession(api, second, () => api.command("programmer.command_line", { value: "GROUP 2 +" }));
      return { second };
    },
    api: async ({ api, bench }, state) => {
      state.first = api.session!;
      await api.command("programmer.command_line", { value: "GROUP 1 +" });
      await subscribeIsolatedHardware(bench, state);
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      state.first = await browserSession(page);
      for (const key of ["GRP", "1", "+"]) await page.getByRole("button", { name: key, exact: true }).click();
      await expect.poll(async () => normalizeCommand(await programmerCommand(api, state.first!))).toBe("GROUP 1 +");
      await subscribeIsolatedHardware(bench, state);
    },
    assert: async ({ api, bench }, state) => {
      const first = state.first!;
      const firstHardware = state.firstHardware!;
      const secondHardware = state.secondHardware!;
      const firstAlias = first.desk.osc_alias;
      const secondAlias = state.second.desk.osc_alias;
      try {
        await expect.poll(async () => normalizeCommand((await programmerForSession(api, first)).command_line)).toBe("GROUP 1 +");
        await expect.poll(async () => normalizeCommand((await programmerForSession(api, state.second)).command_line)).toBe("GROUP 2 +");
        const states = await api.request<any[]>("GET", "/api/v1/programmers", undefined, false);
        expect(states.filter((entry) => [first.session_id, state.second.session_id].includes(entry.session_id))
          .every((entry) => entry.values.length === 0 && Object.keys(entry.group_values).length === 0)).toBe(true);

        const firstFeedback = await firstHardware.expectAfter(state.firstMark!, `/light/${firstAlias}/feedback/command-line`);
        expect(normalizeCommand(String(firstFeedback.arguments[0]))).toBe("GROUP 1 +");
        await bench.tick(0);
        await secondHardware.expectAfter(state.secondMark!, `/light/${secondAlias}/feedback/page`);
        expect(secondHardware.messages.slice(state.secondMark!).filter((message) => message.address.endsWith("/feedback/command-line"))
          .every((message) => normalizeCommand(String(message.arguments[0])) !== "GROUP 1 +")).toBe(true);

        await firstHardware.send("/light/unsubscribe", ["osc-003-a"]);
        await new Promise((resolve) => setTimeout(resolve, 100));
        const disconnectedMark = firstHardware.mark();
        const secondActionMark = secondHardware.mark();
        await secondHardware.send(`/light/${secondAlias}/programmer/digit-3`, [true]);
        await bench.tick(0);
        const secondFeedbackAddress = `/light/${secondAlias}/feedback/command-line`;
        await expect.poll(() => secondHardware.messages
          .slice(secondActionMark)
          .filter((message) => message.address === secondFeedbackAddress)
          .map((message) => normalizeCommand(String(message.arguments[0])))
          .find((command) => command === "GROUP 2 + F3") ?? null).toBe("GROUP 2 + F3");
        await new Promise((resolve) => setTimeout(resolve, 50));
        expect(firstHardware.messages.slice(disconnectedMark)).toHaveLength(0);
        expect((await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(true);

        await secondHardware.send("/light/unsubscribe", ["osc-003-b"]);
        await bench.tick(0);
        await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(false);
      } finally {
        await unsubscribeAndClose(firstHardware, "osc-003-a");
        await unsubscribeAndClose(secondHardware, "osc-003-b");
      }
    },
  });

  pairedScenario<{}>({
    id: "OSC-004",
    title: "invalid input is rejected without programmer or output mutation",
    arrange: async ({ api, bench }, surface) => { await loadCanonicalCopy(api, bench, `osc-004-${surface}`); return {}; },
    api: async ({ api }) => {
      await expect(api.command("not.a.command", {})).rejects.toThrow("unknown command");
    },
    ui: async ({ bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      for (const key of ["GRP", "9", "9", "9", "AT", "5", "0", "ENT"]) {
        await page.getByRole("button", { name: key, exact: true }).click();
      }
      await expect(page.getByLabel("Command line")).toHaveClass(/error/);
    },
    assert: async ({ api, bench }) => {
      const states = await api.request<any[]>("GET", "/api/v1/programmers", undefined, false);
      expect(states.every((state) => state.values.length === 0 && Object.keys(state.group_values).length === 0)).toBe(true);
      expect((await bench.tick(0)).universes.find((entry: any) => entry.universe === 1)!.slots.slice(0, 12)).toEqual(Array(12).fill(0));
    },
  });

  pairedScenario<SharedProgrammerState>({
    id: "OSC-005",
    title: "completed values are user-shared while unfinished commands stay desk-local",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `osc-005-${surface}`);
      await ensureGroupSeven(api);
      const second = await createSession(api, crypto.randomUUID());
      await withSession(api, second, () => api.command("programmer.command_line", { value: "GROUP 1 +" }));
      return { second, auditBefore: (await audit(api)).at(-1)?.revision ?? 0 };
    },
    api: async ({ api }, state) => {
      state.first = api.session!;
      await command(api, "GROUP 7 + FIXTURE 8 AT 50");
    },
    ui: async ({ bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      state.first = await browserSession(page);
      await pressCommand(page, "GROUP 7 + 8 AT 50");
    },
    assert: async ({ api, bench }, state) => {
      const first = await programmerForSession(api, state.first!);
      const second = await programmerForSession(api, state.second);
      expect(normalizeCommand(second.command_line)).toBe("GROUP 1 +");
      expect(normalizeCommand(first.command_line)).not.toBe("GROUP 1 +");

      const fixtures = await fixtureIdsByNumber(api);
      for (const number of [1, 2, 3, 4, 8]) {
        expect(programmerFixtureValue(first, fixtures[number], "intensity")).toBeCloseTo(0.5, 4);
        expect(programmerFixtureValue(second, fixtures[number], "intensity")).toBeCloseTo(0.5, 4);
      }
      expect(second.command_line).toBe("GROUP 1 +");

      const completed = (await api.request<any[]>("GET", `/api/v1/audit?after=${state.auditBefore}`))
        .filter((event) => event.kind === "command_applied" && event.payload?.command === "programmer.execute");
      expect(completed).toHaveLength(1);
      const mark = bench.artnet.mark();
      await bench.tick(3_000);
      expect(Array.from((await bench.artnet.nextAfter(mark, "artnet", 1)).slots.slice(0, 12))).toEqual([
        128, 128, 128, 128, 0, 0, 0, 128, 0, 0, 0, 0,
      ]);
    },
  });

  pairedScenario<PagePlaybackState>({
    id: "OSC-006",
    title: "page two retargets the same current-page playback-one action",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `osc-006-${surface}`);
      return installPlayback(api);
    },
    api: async ({ api }, state) => {
      state.session = api.session!;
      await setDeskPage(api, state.session, 2);
      await api.request("POST", `/api/v1/control-desks/${state.session.desk.id}/page-playbacks/1/button`, {
        button: 1,
        pressed: true,
        surface: "hardware",
      });
    },
    ui: async ({ bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      state.session = await browserSession(page);
      await page.locator(".mode-toggle").click();
      await selectPlaybackPage(page, "Page 2");
      const bank = page.locator(".playback-fader-bank");
      await bank.getByRole("button", { name: "GO +", exact: true }).first().click();
    },
    assert: async ({ api, bench }, state) => {
      expect(await deskPage(api, state.session!)).toBe(2);
      expect(await activePlayback(api, 2)).toMatchObject({ current_cue_number: 1 });
      expect(await activePlayback(api, 1)).toBeUndefined();

      const art = bench.artnet.mark();
      const sacn = bench.sacn.mark();
      const tick = await bench.tick(0);
      expect(tick.universes.find((entry: any) => entry.universe === 1)!.slots.slice(0, 2)).toEqual([0, 191]);
      expect(Array.from((await bench.artnet.nextAfter(art, "artnet", 1)).slots.slice(0, 2))).toEqual([0, 191]);
      expect(Array.from((await bench.sacn.nextAfter(sacn, "sacn", 101)).slots.slice(0, 2))).toEqual([0, 191]);
    },
  });

  pairedScenario<RevisionConflictState>({
    id: "API-001",
    title: "authenticated membership updates reject stale revisions atomically",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `api-001-${surface}`);
      const group = await object(api, "group", "3");
      const fixture = (await fixtureIdsByNumber(api))[5];
      return {
        originalRevision: group.revision,
        originalBody: structuredClone(group.body),
        successfulBody: { ...structuredClone(group.body), fixtures: [...group.body.fixtures, fixture] },
      };
    },
    api: async ({ api }, state) => {
      await putObject(api, "group", "3", state.successfulBody, state.originalRevision);
    },
    ui: async ({ bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      await pressCommand(page, "5");
      await pressCommand(page, "RECORD + GROUP 3");
    },
    assert: async ({ api }, state) => {
      await assertRevisionConflict(api, state);
    },
  });

  pairedScenario<{ auditBefore: number }>({
    id: "API-002",
    title: "Group CRUD produces ordered audit and object events",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `api-002-${surface}`);
      return { auditBefore: (await audit(api)).at(-1)?.revision ?? 0 };
    },
    api: async ({ api }) => {
      const fixtures = await fixtureIdsByNumber(api);
      await api.command("selection.set", { fixtures: [fixtures[1], fixtures[2]] });
      await command(api, "RECORD GROUP 90");
      await api.command("selection.set", { fixtures: [fixtures[3]] });
      await command(api, "RECORD + GROUP 90");
      await command(api, "DELETE GROUP 90");
    },
    ui: async ({ bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      await pressCommand(page, "1 + 2");
      await pressCommand(page, "RECORD GROUP 90");
      await pressCommand(page, "3");
      await pressCommand(page, "RECORD + GROUP 90");
      await pressCommand(page, "DELETE GROUP 90");
    },
    assert: async ({ api }, state) => {
      expect((await objects(api, "group")).some((entry) => entry.id === "90")).toBe(false);
      const events = await api.request<any[]>("GET", `/api/v1/audit?after=${state.auditBefore}`);
      expect(events.filter((event: any) => /group|show_object|command/.test(event.kind)).length).toBeGreaterThanOrEqual(3);
    },
  });

  test("API-003 @api › revisioned command-line HTTP is atomic and replay-safe", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "api-003-command-line-http");

    const initial = await api.getCommandLine();
    expect(initial.commandLine).toMatchObject({
      text: "FIXTURE",
      target: "FIXTURE",
      pristine: true,
      revision: 0,
    });

    const groupPrefix = await api.sendCommandKey("GRP", "press", "api-003-group-prefix");
    expect(groupPrefix).toMatchObject({
      outcome: "accepted",
      action: "edited",
      command_line: { text: "GROUP", target: "FIXTURE", pristine: false, revision: 1 },
    });
    const toggled = await api.sendCommandKey("ENT", "press", "api-003-group-mode");
    expect(toggled).toMatchObject({
      outcome: "accepted",
      action: "edited",
      command_line: { text: "GROUP", target: "GROUP", pristine: true, revision: 2 },
    });

    const replaced = await api.replaceCommandLine("GROUP 1 AT 50", toggled.command_line.revision);
    expect(replaced.commandLine).toMatchObject({ text: "GROUP 1 AT 50", revision: 3 });
    await expect(api.replaceCommandLine("GROUP 2 AT 25", toggled.command_line.revision))
      .rejects.toThrow(/409.*revision conflict/i);

    const requestId = "api-003-execute-group-1";
    const historyBefore = await api.request<any[]>("GET", "/api/v1/command-history");
    const auditBefore = (await audit(api)).at(-1)?.revision ?? 0;
    const executed = await api.executeCommandLineRaw(undefined, requestId);
    expect(executed).toMatchObject({
      outcome: "accepted",
      action: "executed",
      applied: 12,
      command_line: { text: "GROUP", target: "GROUP", pristine: true },
    });
    const historyAfterExecution = await api.request<any[]>("GET", "/api/v1/command-history");
    expect(historyAfterExecution).toHaveLength(historyBefore.length + 1);
    const executionEvents = (await api.request<any[]>("GET", `/api/v1/audit?after=${auditBefore}`))
      .filter((event) => event.payload?.request_id === requestId);
    expect(executionEvents.filter((event) => event.kind === "command_applied")).toHaveLength(1);
    expect(executionEvents.filter((event) => event.kind === "programmer_changed")).toHaveLength(1);

    expect(await api.executeCommandLineRaw(undefined, requestId)).toEqual(executed);
    expect(await api.request<any[]>("GET", "/api/v1/command-history")).toEqual(historyAfterExecution);
    const replayEvents = (await api.request<any[]>("GET", `/api/v1/audit?after=${auditBefore}`))
      .filter((event) => event.payload?.request_id === requestId);
    expect(replayEvents).toEqual(executionEvents);

    const artnetMark = bench.artnet.mark();
    const sacnMark = bench.sacn.mark();
    await bench.tick(3_000);
    expect(Array.from((await bench.artnet.nextAfter(artnetMark, "artnet", 1)).slots.slice(0, 12)))
      .toEqual(Array(12).fill(128));
    expect(Array.from((await bench.sacn.nextAfter(sacnMark, "sacn", 101)).slots.slice(0, 12)))
      .toEqual(Array(12).fill(128));
  });

  registerGroupOutputPair("CROSS-001", 50, 128, "equivalent group value agrees across command surfaces");

  pairedScenario<CrossMutationState>({
    id: "CROSS-002",
    title: "browser live-reconciles the contract's external REST and command-WebSocket mutations",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `cross-002-${surface}`);
      const group = await object(api, "group", "3");
      return {
        originalRevision: group.revision,
        fixtureFive: (await fixtureIdsByNumber(api))[5],
      };
    },
    api: async ({ api }, state) => {
      state.mutationRevision = await appendFixtureFive(api);
      await api.command("programmer.group.set", {
        group_id: "3",
        attribute: "intensity",
        value: 0.5,
      });
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      await openCrossSurfaceLayout(page);

      const groupCard = page.locator(".group-pool-window .group-card").filter({ hasText: "Front Dimmers" });
      await openGroupContext(page, groupCard);
      const order = page.locator(".group-context-menu .group-order");
      await expect(order).toContainText("1. Fixture 1");
      await expect(order).toContainText("4. Fixture 4");
      await expect(order).not.toContainText("Fixture 5");

      // CROSS-002 is intentionally the external-source exception for an @ui
      // adapter: the Markdown contract makes REST and command WebSocket writes
      // the stimulus, while the browser's live reaction is the system under test.
      await test.step("Apply the contract's external REST membership mutation", async () => {
        state.mutationRevision = await appendFixtureFive(api);
      });
      await expect(groupCard).toContainText("5 fixtures");
      await expect(order).toContainText("5. Fixture 5");

      await page.locator(".group-context-menu").getByRole("button", { name: "Select live group", exact: true }).click();
      await expect(groupCard).toHaveClass(/selected/);

      await test.step("Apply the contract's external authenticated command-WebSocket value", async () => {
        await api.command("programmer.group.set", {
          group_id: "3",
          attribute: "intensity",
          value: 0.5,
        });
      });
      const dimmerEncoder = page.locator(".vertical-touch-fader-stack").filter({ hasText: "Enc 1 · Dimmer" });
      await expect(dimmerEncoder).toContainText("50%");

      const artnetMark = bench.artnet.mark();
      const sacnMark = bench.sacn.mark();
      const frame = await bench.tick(3_000);
      const expected = [128, 128, 128, 128, 128, 0, 0, 0, 0, 0, 0, 0];
      expect(frame.universes.find((entry: any) => entry.universe === 1)!.slots.slice(0, 12)).toEqual(expected);
      expect(Array.from((await bench.artnet.nextAfter(artnetMark, "artnet", 1)).slots.slice(0, 12))).toEqual(expected);
      expect(Array.from((await bench.sacn.nextAfter(sacnMark, "sacn", 101)).slots.slice(0, 12))).toEqual(expected);
      for (const number of [1, 5]) {
        const row = fixtureSheetRow(page, number);
        await expect(row.locator(".source-value").first()).toHaveClass(/source-programmer/);
        await expect(row.locator(".source-value").first()).toContainText("50%");
      }

      // Let the operator-created layout persist before proving that a reload
      // produces the same server-backed membership and resolved values.
      await page.waitForTimeout(700);
      await page.reload();
      await expect(page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
      await expect(page.locator(".group-pool-window .group-card").filter({ hasText: "Front Dimmers" })).toContainText("5 fixtures");
      for (const number of [1, 5]) {
        const row = fixtureSheetRow(page, number);
        await expect(row.locator(".source-value").first()).toHaveClass(/source-programmer/);
        await expect(row.locator(".source-value").first()).toContainText("50%");
      }
    },
    assert: async ({ api, bench }, state) => {
      expect(state.mutationRevision).toBe(state.originalRevision + 1);
      expect(await groupNumbers(api, "3")).toEqual([1, 2, 3, 4, 5]);
      expect((await object<any>(api, "group", "3")).body.fixtures.at(-1)).toBe(state.fixtureFive);
      await expectProgrammer(api, (programmer) =>
        expect(normalizedValue(programmer.group_values["3"]?.intensity)).toBeCloseTo(0.5, 4),
      );
      const artnetMark = bench.artnet.mark();
      const sacnMark = bench.sacn.mark();
      const frame = await bench.tick(3_000);
      const expected = [128, 128, 128, 128, 128, 0, 0, 0, 0, 0, 0, 0];
      expect(frame.universes.find((entry: any) => entry.universe === 1)!.slots.slice(0, 12)).toEqual(expected);
      expect(Array.from((await bench.artnet.nextAfter(artnetMark, "artnet", 1)).slots.slice(0, 12))).toEqual(expected);
      expect(Array.from((await bench.sacn.nextAfter(sacnMark, "sacn", 101)).slots.slice(0, 12))).toEqual(expected);
    },
  });

  test("OSC-001 @osc › subscription and one tick return a complete deterministic feedback cycle", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "osc-001-wire");
    const hardware = await bench.osc();
    const alias = api.session!.desk.osc_alias;
    const clientId = `osc-001-${crypto.randomUUID()}`;
    try {
      await hardware.subscribe(clientId, alias);
      const mark = hardware.mark();
      await bench.tick(0);
      for (const address of [
        `/light/${alias}/feedback/page`,
        `/light/${alias}/feedback/command-line`,
        `/light/${alias}/feedback/programmer/group`,
        `/light/${alias}/feedback/page-playback/1/fader`,
        `/light/${alias}/feedback/page-playback/1/button/1`,
        `/light/${alias}/feedback/speed-group/1`,
      ]) await hardware.expectAfter(mark, address);
      const quietMark = hardware.mark();
      await new Promise((resolve) => setTimeout(resolve, 75));
      expect(hardware.messages.slice(quietMark)).toHaveLength(0);
    } finally { await unsubscribeAndClose(hardware, clientId); }
  });

  test("OSC-002 @osc › hardware keypad command reaches feedback and both network outputs", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "osc-002-wire");
    const hardware = await bench.osc();
    const alias = api.session!.desk.osc_alias;
    const clientId = `osc-002-${crypto.randomUUID()}`;
    try {
      await hardware.subscribe(clientId, alias);
      for (const action of ["grp", "digit-1", "at", "digit-2", "digit-5", "enter"]) await hardware.send(`/light/${alias}/programmer/${action}`, [true]);
      await expectProgrammer(api, (state) => expect(state.group_values["1"]?.intensity).toBeDefined());
      const art = bench.artnet.mark(); const sacn = bench.sacn.mark();
      await bench.tick(3_000);
      expect(Array.from((await bench.artnet.nextAfter(art, "artnet", 1)).slots.slice(0, 12))).toEqual(Array(12).fill(64));
      expect(Array.from((await bench.sacn.nextAfter(sacn, "sacn", 101)).slots.slice(0, 12))).toEqual(Array(12).fill(64));
    } finally { await unsubscribeAndClose(hardware, clientId); }
  });

  test("OSC-003 @osc › subscribers on separate desk aliases stay isolated and unsubscribe is reference-counted", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "osc-003-wire");
    const second = await createSession(api, crypto.randomUUID());
    const a = await bench.osc(); const b = await bench.osc();
    try {
      await a.subscribe("osc-003-a", api.session!.desk.osc_alias);
      await b.subscribe("osc-003-b", second.desk.osc_alias);
      const aMark = a.mark(); const bMark = b.mark();
      await a.send(`/light/${api.session!.desk.osc_alias}/programmer/digit-1`, [true]);
      await a.expectAfter(aMark, `/light/${api.session!.desk.osc_alias}/feedback/command-line`);
      expect(b.messages.slice(bMark).some((message) => message.address.includes(api.session!.desk.osc_alias))).toBe(false);
      await a.send("/light/unsubscribe", ["osc-003-a"]);
      await new Promise((resolve) => setTimeout(resolve, 100));
      const disconnected = a.mark();
      await bench.tick(0);
      await new Promise((resolve) => setTimeout(resolve, 50));
      expect(a.messages.slice(disconnected)).toHaveLength(0);
      expect((await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(true);
      await b.send("/light/unsubscribe", ["osc-003-b"]);
      expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(false);
    } finally {
      await unsubscribeAndClose(a, "osc-003-a");
      await unsubscribeAndClose(b, "osc-003-b");
    }
  });

  test("OSC-004 @osc › malformed and unsubscribed input leaves authoritative state unchanged", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "osc-004-wire");
    const hardware = await bench.osc();
    try {
      await hardware.send("/light/subscribe", ["bad", "missing-desk", "wrong-port"]);
      await hardware.send("/light/main/programmer/unknown", [true]);
      await hardware.send("/light/main/programmer/digit-1", [true]);
      await bench.tick(0);
      const states = await api.request<any[]>("GET", "/api/v1/programmers", undefined, false);
      expect(states.every((state) => !state.command_line && state.values.length === 0)).toBe(true);
    } finally { await hardware.close(); }
  });

  test("OSC-005 @osc › two browser desks and their hardware share values but not interaction state", async ({ api, bench, desk, page, browser }) => {
    test.setTimeout(60_000);
    await loadCanonicalCopy(api, bench, "osc-005-mixed");
    await ensureGroupSeven(api);
    await desk.open(bench.baseUrl);
    const firstSession = await browserSession(page);
    const secondContext = await browser.newContext();
    const secondPage = await secondContext.newPage();
    await secondPage.goto(bench.baseUrl);
    await expect(secondPage.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
    await expect(secondPage.locator(".connection-banner")).toBeHidden({ timeout: 10_000 });
    const secondSession = await browserSession(secondPage);
    expect(secondSession.desk.osc_alias).not.toBe(firstSession.desk.osc_alias);
    const firstHardware = await bench.osc();
    const secondHardware = await bench.osc();
    try {
      for (const key of ["GRP", "7", "+"]) await page.getByRole("button", { name: key, exact: true }).click();
      for (const key of ["GRP", "1", "+"]) await secondPage.getByRole("button", { name: key, exact: true }).click();
      await expect.poll(async () => programmerCommand(api, firstSession)).toBe("G7 +");
      await expect.poll(async () => programmerCommand(api, secondSession)).toBe("G1 +");
      await firstHardware.subscribe("osc-005-first", firstSession.desk.osc_alias);
      await firstHardware.expectAfter(0, `/light/${firstSession.desk.osc_alias}/feedback/speed-group/5`);
      const firstSecondSubscriptionMark = firstHardware.mark();
      const secondSubscriptionMark = secondHardware.mark();
      await secondHardware.subscribe("osc-005-second", secondSession.desk.osc_alias);
      await firstHardware.expectAfter(firstSecondSubscriptionMark, `/light/${firstSession.desk.osc_alias}/feedback/speed-group/5`);
      await secondHardware.expectAfter(secondSubscriptionMark, `/light/${secondSession.desk.osc_alias}/feedback/speed-group/5`);
      const firstAfterSecondKey = firstHardware.mark();
      const secondAfterSecondKey = secondHardware.mark();
      await secondHardware.send(`/light/${secondSession.desk.osc_alias}/programmer/digit-2`, [true]);
      await expect(secondPage.getByLabel("Command line")).toHaveValue("G1 + F2");
      await expect(page.getByLabel("Command line")).toHaveValue("G7 +");
      await firstHardware.expectAfter(firstAfterSecondKey, `/light/${firstSession.desk.osc_alias}/feedback/speed-group/5`);
      await secondHardware.expectAfter(secondAfterSecondKey, `/light/${secondSession.desk.osc_alias}/feedback/speed-group/5`);
      const firstKeyMark = firstHardware.mark();
      await firstHardware.send(`/light/${firstSession.desk.osc_alias}/programmer/digit-8`, [true]);
      expect((await firstHardware.expectAfter(firstKeyMark, `/light/${firstSession.desk.osc_alias}/feedback/command-line`)).arguments).toEqual(["G7 + F8"]);
      await expect(page.getByLabel("Command line")).toHaveValue("G7 + F8");
      await expect(secondPage.getByLabel("Command line")).toHaveValue("G1 + F2");

      const commandRevision = Math.max(0, ...(await audit(api)).map((event) => event.revision));
      for (const action of ["at", "digit-5", "digit-0", "enter"]) {
        await firstHardware.send(`/light/${firstSession.desk.osc_alias}/programmer/${action}`, [true]);
      }
      const fixtures = await fixtureIdsByNumber(api);
      for (const number of [1, 2, 3, 4, 8]) {
        await expect.poll(async () => fixtureProgrammerValue(api, fixtures[number], "intensity")).toBeCloseTo(0.5, 4);
      }
      await expect(secondPage.getByLabel("Command line")).toHaveValue("G1 + F2");
      const completed = (await api.request<any[]>("GET", `/api/v1/audit?after=${commandRevision}`))
        .filter((event) => event.kind === "command_applied");
      expect(completed).toHaveLength(1);
      const art = bench.artnet.mark(); await bench.tick(3_000);
      expect(Array.from((await bench.artnet.nextAfter(art, "artnet", 1)).slots.slice(0, 12))).toEqual([
        128, 128, 128, 128, 0, 0, 0, 128, 0, 0, 0, 0,
      ]);

      await firstHardware.send("/light/unsubscribe", ["osc-005-first"]);
      await secondHardware.send("/light/unsubscribe", ["osc-005-second"]);
      await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(false);
      await secondPage.getByLabel("Command line").fill("");
      await expect(secondPage.getByLabel("Command line")).toHaveValue("FIXTURE");
      await secondPage.getByRole("button", { name: "GRP", exact: true }).click();
      await expect(secondPage.getByLabel("Command line")).toHaveValue("GROUP");
      await secondPage.getByRole("button", { name: "ENT", exact: true }).click();
      await expect(secondPage.getByLabel("Command line")).toHaveValue("GROUP");
      await expect.poll(async () => programmerCommand(api, secondSession)).toBe("GROUP");
      const groupModeMark = secondHardware.mark();
      await secondHardware.send("/light/subscribe", ["osc-005-second", secondSession.desk.osc_alias, secondHardware.feedbackPort]);
      await secondHardware.expectAfter(groupModeMark, `/light/${secondSession.desk.osc_alias}/feedback/page`);
      await secondHardware.send(`/light/${secondSession.desk.osc_alias}/programmer/digit-3`, [true]);
      await expect(secondPage.getByLabel("Command line")).toHaveValue("G3");

      await page.getByLabel("Command line").fill("G7 +");
      await expect(page.getByLabel("Command line")).toHaveValue("G7 +");
      await expect.poll(async () => programmerCommand(api, firstSession)).toBe("G7 +");
      const reconnectMark = firstHardware.mark();
      await firstHardware.send("/light/subscribe", ["osc-005-first", firstSession.desk.osc_alias, firstHardware.feedbackPort]);
      expect((await firstHardware.expectAfter(reconnectMark, `/light/${firstSession.desk.osc_alias}/feedback/page`)).arguments).toEqual([1]);
      expect((await firstHardware.expectAfter(reconnectMark, `/light/${firstSession.desk.osc_alias}/feedback/command-line`)).arguments).toEqual(["G7 +"]);

      await secondPage.getByLabel("Command line").fill("G1 + F2");
      await expect.poll(async () => programmerCommand(api, secondSession)).toBe("G1 + F2");
      const reattachMark = firstHardware.mark();
      await firstHardware.send("/light/subscribe", ["osc-005-first", secondSession.desk.osc_alias, firstHardware.feedbackPort]);
      await firstHardware.expectAfter(reattachMark, `/light/${secondSession.desk.osc_alias}/feedback/page`);
      await firstHardware.send(`/light/${secondSession.desk.osc_alias}/programmer/digit-9`, [true]);
      await expect(secondPage.getByLabel("Command line")).toHaveValue("G1 + F29");
      await expect(page.getByLabel("Command line")).toHaveValue("G7 +");
    } finally {
      await unsubscribeAndClose(firstHardware, "osc-005-first");
      await unsubscribeAndClose(secondHardware, "osc-005-second");
      await secondContext.close();
    }
  });

  test("OSC-006 @osc › current-page follows each desk while explicit-page ignores it", async ({ api, bench, desk, page }) => {
    test.setTimeout(60_000);
    await loadCanonicalCopy(api, bench, "osc-006-wire");
    await installPlayback(api);
    await desk.open(bench.baseUrl);
    const firstSession = await browserSession(page);
    const secondSession = await createSession(api, crypto.randomUUID());
    await setDeskPage(api, secondSession, 2);
    const firstHardware = await bench.osc();
    const secondHardware = await bench.osc();
    const firstAlias = firstSession.desk.osc_alias;
    const secondAlias = secondSession.desk.osc_alias;
    try {
      await firstHardware.subscribe("osc-006-first", firstAlias);
      await secondHardware.subscribe("osc-006-second", secondAlias);
      expect(await deskPage(api, firstSession)).toBe(1);
      expect(await deskPage(api, secondSession)).toBe(2);

      const firstAddress = `/light/${firstAlias}/page-playback/1/button/1`;
      await firstHardware.send(firstAddress, [true]);
      await expect.poll(async () => (await activePlayback(api, 1))?.current_cue_number).toBe(1);
      await firstHardware.send(firstAddress, [false]);
      await new Promise((resolve) => setTimeout(resolve, 50));
      expect(await activePlayback(api, 1)).toMatchObject({ current_cue_number: 1 });

      await secondHardware.send(`/light/${secondAlias}/page-playback/1/button/1`, [true]);
      await expect.poll(async () => (await activePlayback(api, 2))?.current_cue_number).toBe(1);

      const pageFeedbackMark = firstHardware.mark();
      await selectPlaybackPage(page, "Page 2");
      await expect.poll(async () => deskPage(api, firstSession)).toBe(2);
      expect((await firstHardware.expectAfter(pageFeedbackMark, `/light/${firstAlias}/feedback/page`)).arguments).toEqual([2]);
      await firstHardware.send(firstAddress, [true]);
      await expect.poll(async () => (await activePlayback(api, 2))?.current_cue_number).toBe(2);
      expect(await deskPage(api, secondSession)).toBe(2);

      const returnFeedbackMark = firstHardware.mark();
      await selectPlaybackPage(page, "Main");
      await expect.poll(async () => deskPage(api, firstSession)).toBe(1);
      expect((await firstHardware.expectAfter(returnFeedbackMark, `/light/${firstAlias}/feedback/page`)).arguments).toEqual([1]);

      for (const level of [0, 0.5, 1]) {
        await firstHardware.sendFloat(`/light/${firstAlias}/page-playback/1/fader`, level);
        await expect.poll(async () => (await activePlayback(api, 1))?.fader_position).toBeCloseTo(level, 4);
      }
      for (const level of [0, 0.5, 1]) {
        await firstHardware.sendFloat("/light/playback/2/1/fader", level);
        await expect.poll(async () => (await activePlayback(api, 2))?.fader_position).toBeCloseTo(level, 4);
        expect(await deskPage(api, firstSession)).toBe(1);
      }
    } finally {
      await unsubscribeAndClose(firstHardware, "osc-006-first");
      await unsubscribeAndClose(secondHardware, "osc-006-second");
    }
  });

  test("CROSS-001 @osc › OSC matches UI and API normalized Group output", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "cross-001-osc");
    const hardware = await bench.osc(); const alias = api.session!.desk.osc_alias;
    try {
      await hardware.subscribe("cross-001-osc", alias);
      for (const action of ["grp", "digit-1", "at", "digit-5", "digit-0", "enter"]) await hardware.send(`/light/${alias}/programmer/${action}`, [true]);
      const art = bench.artnet.mark(); await bench.tick(3_000);
      expect(Array.from((await bench.artnet.nextAfter(art, "artnet", 1)).slots.slice(0, 12))).toEqual(Array(12).fill(128));
    } finally { await unsubscribeAndClose(hardware, "cross-001-osc"); }
  });
});

function registerGroupOutputPair(id: string, percent: number, byte: number, title: string) {
  pairedScenario<{}>({
    id, title,
    arrange: async ({ api, bench }, surface) => { await loadCanonicalCopy(api, bench, `${id.toLowerCase()}-${surface}`); return {}; },
    api: async ({ api }) => { await api.command("programmer.group.set", { group_id: "1", attribute: "intensity", value: percent / 100 }); },
    ui: async ({ bench, desk, page }) => { await desk.open(bench.baseUrl); await pressCommand(page, `GROUP 1 AT ${percent}`); },
    assert: async ({ bench }) => {
      const mark = bench.artnet.mark(); await bench.tick(3_000);
      expect(Array.from((await bench.artnet.nextAfter(mark, "artnet", 1)).slots.slice(0, 12))).toEqual(Array(12).fill(byte));
    },
  });
}

async function createSession(api: ApiDriver, clientId: string): Promise<Session> {
  return api.request<Session>("POST", "/api/v1/sessions", { username: "Operator", client_id: clientId }, false);
}

async function unsubscribeAndClose(hardware: OscHardware, clientId: string): Promise<void> {
  try {
    await hardware.send("/light/unsubscribe", [clientId]);
    await new Promise((resolve) => setTimeout(resolve, 25));
  } catch {
    // Cleanup must not hide the scenario's original assertion or server failure.
  } finally {
    await hardware.close();
  }
}

async function subscribeIsolatedHardware(bench: any, state: OscIsolationState): Promise<void> {
  state.firstHardware = await bench.osc();
  state.secondHardware = await bench.osc();
  state.firstMark = state.firstHardware.mark();
  await state.firstHardware.subscribe("osc-003-a", state.first!.desk.osc_alias);
  await state.firstHardware.expectAfter(state.firstMark, `/light/${state.first!.desk.osc_alias}/feedback/speed-group/5`);
  const firstSecondSubscriptionMark = state.firstHardware.mark();
  state.secondMark = state.secondHardware.mark();
  await state.secondHardware.subscribe("osc-003-b", state.second.desk.osc_alias);
  await state.firstHardware.expectAfter(firstSecondSubscriptionMark, `/light/${state.first!.desk.osc_alias}/feedback/speed-group/5`);
  await state.secondHardware.expectAfter(state.secondMark, `/light/${state.second.desk.osc_alias}/feedback/speed-group/5`);
}

async function assertRevisionConflict(api: ApiDriver, state: RevisionConflictState): Promise<void> {
  const url = `${api.baseUrl}/api/v1/shows/${await activeShowId(api)}/objects/group/3`;
  const unauthenticated = await fetch(url);
  expect(unauthenticated.status).toBe(401);
  expect(await unauthenticated.json()).toEqual({ error: "missing session token" });
  const invalid = await fetch(url, { headers: { authorization: "Bearer invalid" } });
  expect(invalid.status).toBe(401);
  expect(await invalid.json()).toEqual({ error: "invalid session token" });

  const stale = await fetch(url, {
    method: "PUT",
    headers: {
      authorization: `Bearer ${api.session!.token}`,
      "content-type": "application/json",
      "if-match": String(state.originalRevision),
    },
    body: JSON.stringify({ ...state.originalBody, name: "stale name" }),
  });
  expect(stale.status).toBe(409);
  expect(await stale.json()).toMatchObject({ error: expect.stringContaining("revision conflict") });

  const current = await object(api, "group", "3");
  expect(current.revision).toBe(state.originalRevision + 1);
  expect(current.body).toEqual(state.successfulBody);
}

async function programmerForSession(api: ApiDriver, session: Session): Promise<any> {
  const states = await api.request<any[]>("GET", "/api/v1/programmers", undefined, false);
  const found = states.find((state) => state.session_id === session.session_id);
  expect(found).toBeDefined();
  return found!;
}

function programmerFixtureValue(programmer: any, fixtureId: string, attribute: string): number | null {
  const value = programmer.values.find((entry: any) => entry.fixture_id === fixtureId && entry.attribute === attribute);
  return normalizedValue(value);
}

async function audit(api: ApiDriver): Promise<any[]> {
  return api.request<any[]>("GET", "/api/v1/audit?after=0");
}

async function appendFixtureFive(api: ApiDriver): Promise<number> {
  const group = await object(api, "group", "3");
  const fixture = (await fixtureIdsByNumber(api))[5];
  await putObject(api, "group", "3", { ...group.body, fixtures: [...group.body.fixtures, fixture] }, group.revision);
  return (await object(api, "group", "3")).revision;
}

async function openCrossSurfaceLayout(page: Page): Promise<void> {
  const fixtureSheet = page.locator(".desk-pane").filter({ has: page.locator(".fixture-window") });
  const groupPool = page.locator(".desk-pane").filter({ has: page.locator(".group-pool-window") });
  if (await fixtureSheet.isVisible() && await groupPool.isVisible()) return;

  await expect(fixtureSheet).toBeVisible();
  const presetPane = page.locator(".desk-pane").filter({ has: page.locator(".preset-pool-window") });
  await expect(presetPane).toBeVisible();
  await presetPane.getByRole("button", { name: "Settings", exact: true }).click();
  const settings = page.getByRole("dialog", { name: "Pane Settings" });
  await expect(settings).toBeVisible();
  await settings.getByRole("button", { name: "Remove pane", exact: true }).click();
  await expect(presetPane).toBeHidden();

  const grid = page.locator(".desk-grid");
  const box = await grid.boundingBox();
  expect(box).not.toBeNull();
  await page.mouse.click(box!.x + box!.width * 0.1, box!.y + box!.height * 0.1);
  const picker = page.locator(".window-picker");
  await expect(picker).toBeVisible();
  await picker.getByRole("button", { name: "Group pool", exact: true }).click();
  await expect(groupPool).toBeVisible();
  await expect(fixtureSheet).toBeVisible();
}

async function openGroupContext(page: Page, card: Locator): Promise<void> {
  await card.scrollIntoViewIfNeeded();
  await expect(card).toBeVisible();
  const box = await card.boundingBox();
  expect(box).not.toBeNull();
  await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
  await page.mouse.down();
  await page.waitForTimeout(700);
  await page.mouse.up();
  await expect(page.locator(".group-context-menu")).toBeVisible();
}

function fixtureSheetRow(page: Page, number: number): Locator {
  return page.locator(".fixture-window .ui-data-table-row:not(.header)").filter({
    has: page.getByRole("cell", { name: String(number), exact: true }),
  }).first();
}

async function installPlayback(api: ApiDriver): Promise<{ firstCueListId: string; secondCueListId: string }> {
  const fixtures = await fixtureIdsByNumber(api);
  const firstCueListId = crypto.randomUUID();
  const secondCueListId = crypto.randomUUID();
  for (const [id, name, fixture, levels] of [
    [firstCueListId, "OSC Page One", fixtures[1], [0.25, 0.5]],
    [secondCueListId, "OSC Page Two", fixtures[2], [0.75, 1]],
  ] as const) {
    await putObject(api, "cue_list", id, {
      id, name, priority: 0, mode: "sequence", looped: false,
      chaser_step_millis: 1000, speed_group: null,
      cues: levels.map((level, index) => ({
        number: index + 1,
        name: `Cue ${index + 1}`,
        changes: [{ fixture_id: fixture, attribute: "intensity", value: { kind: "normalized", value: level }, fade_millis: 0 }],
        group_changes: [],
        fade_millis: 0,
        delay_millis: 0,
        trigger: { type: "manual" },
        phasers: [],
      })),
    });
  }
  for (const [number, name, cueListId] of [
    [1, "OSC Page One", firstCueListId],
    [2, "OSC Page Two", secondCueListId],
  ] as const) {
    const existing = (await objects(api, "playback")).find((playback) => playback.id === String(number));
    await putObject(api, "playback", String(number), {
      number,
      name,
      target: { type: "cue_list", cue_list_id: cueListId },
      buttons: ["go", "go_minus", "flash"],
      button_count: 3,
      fader: "master",
      has_fader: true,
      go_activates: true,
      auto_off: false,
      xfade_millis: 0,
      color: number === 1 ? "#20c997" : "#4d8cff",
      flash_release: "release_all",
      protect_from_swap: false,
      presentation_icon: null,
      presentation_image: null,
    }, existing?.revision ?? 0);
  }
  for (const [number, name, playback] of [[1, "Main", 1], [2, "Page 2", 2]] as const) {
    const existing = (await objects(api, "playback_page")).find((page) => page.id === String(number));
    await putObject(api, "playback_page", String(number), { number, name, slots: { "1": playback } }, existing?.revision ?? 0);
  }
  return { firstCueListId, secondCueListId };
}

async function activePlayback(api: ApiDriver, number: number): Promise<any | undefined> {
  const playbacks = await api.request<any>("GET", "/api/v1/playbacks");
  return playbacks.active.find((playback: any) => playback.playback_number === number);
}

async function deskPage(api: ApiDriver, session: Session): Promise<number> {
  return (await withSession(api, session, () => api.request<any>("GET", "/api/v1/playbacks"))).active_page;
}

async function setDeskPage(api: ApiDriver, session: Session, page: number): Promise<void> {
  await withSession(api, session, () => api.request("PUT", `/api/v1/control-desks/${session.desk.id}/page`, { page }));
}

async function withSession<T>(api: ApiDriver, session: Session, action: () => Promise<T>): Promise<T> {
  const original = api.session;
  api.session = session;
  try {
    return await action();
  } finally {
    api.session = original;
  }
}

async function ensureGroupSeven(api: ApiDriver): Promise<void> {
  const fixtures = await fixtureIdsByNumber(api);
  const existing = (await objects<any>(api, "group")).find((group) => group.id === "7");
  await putObject(api, "group", "7", {
    ...(existing?.body ?? {}),
    id: "7",
    name: "OSC Mixed Group",
    fixtures: [fixtures[1], fixtures[2], fixtures[3], fixtures[4]],
    derived_from: null,
    frozen_from: null,
    programming: existing?.body.programming ?? {},
    master: 1,
    playback_fader: null,
  }, existing?.revision ?? 0);
}

async function fixtureProgrammerValue(api: ApiDriver, fixtureId: string, attribute: string): Promise<number | null> {
  const states = await api.request<any[]>("GET", "/api/v1/programmers", undefined, false);
  const value = states.flatMap((state) => state.values).find((entry) => entry.fixture_id === fixtureId && entry.attribute === attribute);
  return normalizedValue(value);
}

async function programmerCommand(api: ApiDriver, session: Session): Promise<string | null> {
  const states = await api.request<any[]>("GET", "/api/v1/programmers", undefined, false);
  return states.find((state) => state.session_id === session.session_id)?.command_line?.trim() ?? null;
}

function normalizedValue(value: any): number | null {
  let current = value;
  while (current && typeof current === "object" && "value" in current) current = current.value;
  return typeof current === "number" ? current : null;
}

async function browserSession(page: Page): Promise<Session> {
  return page.evaluate(() => JSON.parse(localStorage.getItem("light.primary-session") ?? "null"));
}

async function selectPlaybackPage(page: Page, name: string): Promise<void> {
  const softwareControl = page.locator(".playback-page-current");
  if (await softwareControl.count()) await softwareControl.click();
  else await page.locator(".hardware-control-summary").getByRole("button", { name: /^Page \d+$/ }).click();
  const dialog = page.locator(".playback-page-modal");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button").filter({ hasText: name }).click();
}

function normalizeCommand(value: string | null | undefined): string {
  return (value ?? "").trim().replace(/^G(\d+)/, "GROUP $1");
}
