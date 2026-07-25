// @bench-semantic-world

import { expect } from "./bench/core/fixtures";
import {
	fixture,
	fixtureRange,
} from "./bench/output/fixtureDmx";
import { PaneType } from "./bench/window-system/paneTypes";
import { scenario } from "./bench/core/scenario";
import { Show } from "./bench/show/showScenario";

scenario(
	"BENCH-UI-003",
	"captures semantic surfaces and emits typed secondary-screen intent",
	async (t) => {
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
	},
);

scenario(
	"BENCH-SHOW-004",
	"runs named create, autosave, Save As, revision, and reopen through visible operator controls",
	async (t) => {
		await t.app.open();
		const empty = await t.show.create(`Operator Empty ${crypto.randomUUID()}`);
		await t.show.expect.active(empty);
		await t.show.expect.dirty(false);
		await t.show.save();

		await t.show.use(Show.TwelveDimmers);
		const source = await t.show.saveAs(
			`Portable Source ${crypto.randomUUID()}`,
		);
		await t.show.expect.active(source);
		const revision = await t.show.saveRevision("Approved operator state");
		await t.show.expect.revision({
			number: revision,
			name: "Approved operator state",
		});
		const laterCopy = await t.show.saveAs(
			`Portable Later Copy ${crypto.randomUUID()}`,
		);
		await t.show.expect.active(laterCopy);
		await t.show.load(source);
		await t.show.expect.active(source);
		const revisionCopy = await t.show.loadRevision(source, revision);
		await t.show.expect.active(revisionCopy);
		await t.show.save();
	},
);

scenario(
	"BENCH-CLOCK-DMX-001",
	"uses exact clock boundaries and fixture-aware logical DMX without hidden ticks",
	async (t) => {
		const first = await t.clock.advanceStep();
		expect(first.now).toBe("2020-01-01T00:00:00Z");
		expect(first.packets_sent).toBe(2);

		const boundaries: string[] = [];
		await t.clock.at(
			[
				{ name: "before", at: "999ms" },
				{ name: "exact", at: "1s" },
				{ name: "after", at: "1.001s" },
			],
			({ name, frame }) => boundaries.push(`${name}:${frame.now}`),
		);
		expect(boundaries).toEqual([
			"before:2020-01-01T00:00:00.999Z",
			"exact:2020-01-01T00:00:01Z",
			"after:2020-01-01T00:00:01.001Z",
		]);

		await t.expectFixtureDMX(fixture(1), { Intensity: 0 });
		await t.expectFixtureDMX(fixtureRange(1, 3), { Intensity: 0 });
		await expect(
			t.expectFixtureDMX(fixture(1), { "Missing component": 0 }),
		).rejects.toThrow(/Valid channels: Intensity/u);
		await expect(
			t.expectFixtureDMX(fixture(1), { Intensity: 256 }),
		).rejects.toThrow(/integer from 0 through 255/u);
	},
);
