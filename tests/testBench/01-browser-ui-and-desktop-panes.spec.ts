import { expect } from "../../apps/control-ui/e2e/bench/core/fixtures";
import { scenario } from "../../apps/control-ui/e2e/bench/core/scenario";
import {
	builtInPaneTypes,
	PaneType,
	PresetFamily,
	StageView,
} from "../../apps/control-ui/e2e/bench/window-system/paneTypes";

scenario("BENCH-UI-001", "opens the application and every operator Built-in by typed intent", async (t) => {
	await t.app.open();
	await t.app.expect.ready();
	for (const type of builtInPaneTypes) {
		await t.builtIn.open(type);
		await t.builtIn.expect.active(type);
	}
});

scenario("BENCH-UI-002", "builds and operates a typed three-pane Desktop", async (t) => {
	await t.app.open();
	const invalid = t.desktop.configure("Invalid layout");
	expect(() => invalid.addPane(PaneType.Stage, { slug: "Not valid", column: 1, row: 1, width: 6, height: 6 })).toThrow(/kebab-case/);
	invalid.addPane(PaneType.Stage, { slug: "first", column: 1, row: 1, width: 6, height: 6 });
	expect(() => invalid.addPane(PaneType.Fixtures, { slug: "first", column: 7, row: 1, width: 6, height: 6 })).toThrow(/Duplicate/);
	expect(() => invalid.addPane(PaneType.Fixtures, { slug: "outside", column: 24, row: 18, width: 2, height: 2 })).toThrow(/24 × 18/);
	expect(() => invalid.addPane(PaneType.Fixtures, { slug: "collision", column: 6, row: 1, width: 6, height: 6 })).toThrow(/collides/);

	const layout = t.desktop.configure("Bench Programming");
	const stage = layout.addPane(PaneType.Stage, { slug: "main-stage", column: 1, row: 1, width: 12, height: 9 });
	await stage.configure({ view: StageView.ThreeDimensional, followPreload: false, beamGuides: true });
	const fixtures = layout.addPane(PaneType.Fixtures, { slug: "fixtures", column: 13, row: 1, width: 12, height: 9 });
	await fixtures.configure({ showGroupShortcuts: true });
	const presets = layout.addPane(PaneType.Presets, { slug: "presets", column: 1, row: 10, width: 24, height: 9 });
	await presets.configure({ family: PresetFamily.Mixed, poolColors: true });
	await layout.apply();

	await stage.expect.geometry({ column: 1, row: 1, width: 12, height: 9 });
	await fixtures.expect.geometry({ column: 13, row: 1, width: 12, height: 9 });
	await presets.expect.geometry({ column: 1, row: 10, width: 24, height: 9 });
	await stage.focus();
	await stage.maximize();
	await stage.expect.maximized();
	await stage.restore();
	await stage.resize({ width: 10, height: 8 });
	await stage.move({ column: 2, row: 2 });
	await stage.expect.geometry({ column: 2, row: 2, width: 10, height: 8 });

	await t.desktop.open("Programming");
	await t.desktop.open("Bench Programming");
	const sameStage = t.desktop.getPane(PaneType.Stage, "main-stage");
	await sameStage.expect.visible();
	expect(() => t.desktop.getPane(PaneType.Fixtures, "main-stage")).toThrow(/not requested type/);
	expect(() => t.desktop.getPane(PaneType.Stage, "missing-pane")).toThrow(/No pane is bound/);
	await fixtures.remove();
});
