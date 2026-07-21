import { expect } from "../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import { batchProgrammerValues } from "../apps/control-ui/e2e/bench/programmerValues";
import { replaceProgrammingSelection } from "../apps/control-ui/e2e/bench/programmingSelection";
import { loadCanonicalCopy, programmer } from "./support/catalog";

type Assignment = { fixture_id: string; attribute: "pan" | "tilt"; value: number };
type ReturnHomeState = {
  showId: string;
  selected: string[];
  home: Assignment[];
  before: Assignment[];
};

pairedScenario<ReturnHomeState>({
  id: "POSITION-HOME-001",
  title: "Return Home applies per-head Position defaults as one programmer gesture",
  arrange: async ({ api, bench }, surface) => {
    const show = await loadCanonicalCopy(api, bench, `position-home-001-${surface}`, "default-stage");
    const patch = await api.request<any>("GET", "/api/v1/patch", undefined, false);
    const targets = patch.fixtures.flatMap((fixture: any) => {
      const logicalByIndex = new Map(
        fixture.logical_heads.map((head: any) => [head.head_index, head.fixture_id]),
      );
      return fixture.definition.heads.flatMap((head: any) => {
        const fixtureId = head.shared ? fixture.fixture_id : logicalByIndex.get(head.index);
        if (!fixtureId) return [];
        const home = (["pan", "tilt"] as const).flatMap((attribute) => {
          const parameter = head.parameters.find((candidate: any) => candidate.attribute === attribute);
          if (!parameter) return [];
          return [{
            fixture_id: fixtureId as string,
            attribute,
            value: Number.isFinite(parameter.default) ? parameter.default : 0.5,
          }];
        });
        return home.length ? [{ fixtureId: fixtureId as string, home }] : [];
      });
    });
    expect(targets.length).toBeGreaterThanOrEqual(2);
    const chosen = [targets[1], targets[0]];
    const home = chosen.flatMap((target) => target.home);
    const before = home.map((assignment, index) => ({
      ...assignment,
      value: index % 2 === 0 ? 0.91 : 0.09,
    }));
    const nonPosition = patch.fixtures.find((fixture: any) =>
      fixture.definition.heads.every((head: any) =>
        head.parameters.every((parameter: any) => !["pan", "tilt"].includes(parameter.attribute)),
      ),
    );
    const selected = [
      chosen[0].fixtureId,
      ...(nonPosition ? [nonPosition.fixture_id] : []),
      chosen[1].fixtureId,
    ];
    await replaceProgrammingSelection(api, {
      surface: "api",
      showId: show.id,
      fixtures: selected,
    });
    await setMany(api, show.id, before);
    return { showId: show.id, selected, home, before };
  },
  api: async ({ api }, state) => {
    await setMany(api, state.showId, state.home);
  },
  ui: async ({ api, bench, desk, page }, state) => {
    await desk.open(api.baseUrl);
    await page.getByRole("button", { name: "Position", exact: true }).click();
    await page.getByRole("button", { name: "Special Dialog", exact: true }).click();
    const home = page.getByRole("button", { name: "Return Home", exact: true });
    await expect(home).toBeVisible();
    await expect(home).toBeEnabled();
    await home.click();
    await expectAssignments(api, state.home);

    await page.locator(".modal-close").click();
    await page.getByRole("button", { name: "UND", exact: true }).click();
    await expectAssignments(api, state.before);

    const hardware = await bench.osc();
    const clientId = `position-home-${crypto.randomUUID()}`;
    try {
      await hardware.subscribe(clientId, api.session!.desk.osc_alias);
      await expect.poll(async () =>
        (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected,
      ).toBe(true);
      await expect(page.locator(".control-section.hardware-connected")).toBeVisible();
      await page.getByRole("button", { name: "Special Dialog", exact: true }).click();
      const home = page.getByRole("button", { name: "Return Home", exact: true });
      await expect(home).toBeVisible();
      await home.click();
    } finally {
      await hardware.send("/light/unsubscribe", [clientId]).catch(() => undefined);
      await hardware.close();
    }
    await expectAssignments(api, state.home);
    await page.locator(".modal-close").click();
    await page.getByRole("button", { name: "CLR", exact: true }).click();
    await expect.poll(async () => (await programmer(api)).selected).toEqual([]);
    await page.getByRole("button", { name: "Special Dialog", exact: true }).click();
    await expect(page.getByRole("button", { name: "Return Home", exact: true })).toBeDisabled();
  },
  assert: async ({ api }, state, surface) => {
    expect((await programmer(api)).selected).toEqual(surface === "ui" ? [] : state.selected);
    await expectAssignments(api, state.home);
    const audit = await api.request<any[]>("GET", "/api/v1/audit?after=0");
    expect(audit.some((event) => event.kind === "programmer_changed")).toBe(true);
  },
});

async function setMany(api: any, showId: string, assignments: Assignment[]): Promise<void> {
  await batchProgrammerValues(api, {
    surface: "api",
    showId,
    mutations: assignments.map((assignment) => ({
      action: "set_fixture",
      fixtureId: assignment.fixture_id,
      attribute: assignment.attribute,
      value: { kind: "normalized", value: assignment.value },
      timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
    })),
  });
}

async function expectAssignments(api: any, expected: Assignment[]): Promise<void> {
  await expect.poll(async () => {
    const values = (await programmer(api)).values;
    return expected.every((assignment) => {
      const actual = values.find((value) =>
        value.fixture_id === assignment.fixture_id && value.attribute === assignment.attribute,
      );
      const value = actual?.value?.value ?? actual?.value;
      return typeof value === "number" && Math.abs(value - assignment.value) < 0.00001;
    });
  }).toBe(true);
}
