// @bench-semantic-world

import {
	CueRecordMode,
} from "../apps/control-ui/e2e/bench/playbacks/cuePlaybackScenario";
import { PaneType } from "../apps/control-ui/e2e/bench/window-system/paneTypes";
import {
	currentPagePlayback,
	explicitPagePlayback,
} from "../apps/control-ui/e2e/bench/playbacks/playbackScenario";
import { scenario } from "../apps/control-ui/e2e/bench/core/scenario";
import {
	fixture,
	fixtureRange,
	groupRange,
} from "../apps/control-ui/e2e/bench/command-selection/selectionContract";
import { Show } from "../apps/control-ui/e2e/bench/show/showScenario";

scenario(
	"BENCH-CUE-PLAYBACK-001",
	"visible recording and typed runtime actions preserve Cue timing and concrete Playback identity",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");
		await t.selection.fixtures.via.api.item(101);

		await t.encoder.intensity.dimmer.via.api.set(20);
		const playback = await t.record.via.ui.playback(1);
		await t.playback.expect(playback).present();
		await t.cue.expect(playback, 1).present();

		await t.encoder.clear();
		await t.encoder.intensity.dimmer.via.api.set(80);
		await t.record.via.api.cue({
			playback,
			cue: 2,
			mode: CueRecordMode.Overwrite,
			timing: { fade: "1" },
		});
		await t.cue.expect(playback, 2).metadata({ fade_millis: 1_000 });
		await t.encoder.clear();

		await t.playback.via.api.off(playback);
		await t.playback.via.ui.go(playback);
		await t.playback.expect(playback).runtime({
			current_cue_number: 1,
			enabled: true,
		});
		await t.expectFixtureDMX({ fixture: 101 }, { Intensity: 51 });

		await t.playback.via.api.go(playback);
		await t.clock.advanceBy("500ms");
		await t.expectFixtureDMX({ fixture: 101 }, { Intensity: 128 });
		await t.clock.advanceBy("500ms");
		await t.expectFixtureDMX({ fixture: 101 }, { Intensity: 204 });

		await t.playback.via.api.select(playback);
		await t.playback.expect(playback).selected();
	},
);

scenario(
	"BENCH-PAGE-PLAYBACK-001",
	"current and explicit Page targets retain distinct Playback authority",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");

		await t.selection.fixtures.via.api.item(101);
		await t.encoder.intensity.dimmer.via.api.set(25);
		const first = await t.record.playback(1);
		await t.encoder.clear();
		await t.selection.fixtures.via.api.item(102);
		await t.encoder.intensity.dimmer.via.api.set(75);
		const second = await t.record.playback(2);
		await t.encoder.clear();

		await t.page.create(2);
		await t.page.rename(2, "Second");
		await t.page.map({ page: 2, slot: 1, playback: second });
		await t.page.expect(2).named("Second");

		await t.playback.via.api.off(first);
		await t.playback.via.api.off(second);
		await t.page.via.api.select(1);
		await t.playback.via.api.go(currentPagePlayback(1));
		await t.playback.expect(first).runtime({ enabled: true });
		await t.playback.expect(second).runtime({ enabled: false });

		await t.page.via.ui.select(2);
		await t.playback.via.api.go(currentPagePlayback(1));
		await t.playback.expect(second).runtime({ enabled: true });

		await t.playback.via.api.off(first);
		await t.playback.via.api.go(explicitPagePlayback(1, 1));
		await t.playback.expect(first).runtime({ enabled: true });
		await t.page.expect(2).selected();

		const screen = await t.screen.create({
			name: "Independent Playbacks",
			showPlaybacks: true,
			showPageControls: true,
			playbacks: {
				perRow: 1,
				rows: [{ first: 1, fader: true, buttons: 3 }],
				pageMode: "dedicated",
			},
		});
		await screen.page.select(1);
		await screen.page.expectSelected(1);
		await t.page.expect(2).selected();
	},
);

scenario(
	"BENCH-SELECTION-ROUTES-001",
	"visible panes, keypad, API, and OSC converge on the ordered selection oracle",
	async (t) => {
		await t.app.open();
		await t.app.expect.ready();
		const desktop = t.desktop.configure("Selection route proof");
		desktop.addPane(PaneType.Fixtures, {
			slug: "fixtures",
			column: 1,
			row: 1,
			width: 8,
			height: 18,
		});
		desktop.addPane(PaneType.Stage, {
			slug: "stage",
			column: 9,
			row: 1,
			width: 8,
			height: 18,
		});
		desktop.addPane(PaneType.Groups, {
			slug: "groups",
			column: 17,
			row: 1,
			width: 8,
			height: 18,
		});
		await desktop.apply();

		await t.selection.clear();
		await t.selection.fixtures.via.fixtureSheet.items(1, 3, 2);
		await t.expect.selection(fixture(1), fixture(3), fixture(2));

		await t.selection.clear();
		await t.selection.fixtures.via.stage.range(1, 4);
		await t.expect.selection(fixtureRange(1, 4));

		await t.selection.clear();
		await t.selection.groups.via.pool.range(1, 4);
		await t.expect.selection(groupRange(1, 4));

		await t.selection.fixtures.via.keypad.range(2, 5);
		await t.expect.selection(fixtureRange(2, 5));

		await t.selection.fixtures.via.api.items(5, 2, 4);
		await t.expect.selection(fixture(5), fixture(2), fixture(4));

		await t.hardware.connect();
		try {
			await t.selection.fixtures.via.osc.item(6);
			await t.expect.selection(fixture(6));
		} finally {
			await t.hardware.disconnect();
		}
	},
);
