import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import { scenario } from "../../apps/control-ui/e2e/bench/scenario";
import {
	builtInPaneTypes,
	PaneType,
	PresetFamily,
	StageView,
} from "../../apps/control-ui/e2e/bench/paneTypes";

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

scenario("BENCH-UI-003", "captures semantic surfaces and emits typed secondary-screen intent", async (t) => {
	await t.app.open();
	await t.builtIn.open(PaneType.Stage);
	await t.screenshot.application("bench-application");
	await t.screenshot.builtIn(PaneType.Stage, "bench-stage-built-in");

	await t.desktop.create("Screenshot Desktop");
	await t.desktop.openSettingsFor("Screenshot Desktop");
	await t.screenshot.dialog("Desktop settings", "bench-desktop-dialog");
	await t.screenshot.application("bench-application-with-portal");
	await t.desktop.closeSettings();

	const screen = await t.screen.create({
		name: "Bench output",
		desktop: "Screenshot Desktop",
		showDock: false,
		showPlaybacks: true,
		showPageControls: true,
		display: { id: "display-2", name: "Operator display" },
		bounds: { x: 100, y: 80, width: 1024, height: 768 },
		fullscreen: false,
		desiredOpen: true,
		playbacks: {
			perRow: 8,
			rows: [{ first: 41, fader: true, buttons: 3 }],
			pageMode: "dedicated",
		},
	});
	await screen.expectBridgeAction("open_console_screen");
	await screen.close();
	await screen.expectBridgeAction("close_console_screen");
	await screen.remove();
});
