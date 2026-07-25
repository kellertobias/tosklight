// @bench-semantic-world

import { fixture } from "./bench/command-selection/selectionContract";
import { scenario } from "./bench/core/scenario";
import { Show } from "./bench/show/showScenario";

scenario(
	"MIB-001",
	"a dark fixture prepositions for its next lit Cue",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.moveInBlack.install({
			enabledFixture: 101,
			disabledFixture: 102,
			playback: 1,
			delay: "1s",
		});
		await t.app.open();
		await t.app.expect.ready();

		await t.moveInBlack.expectConfiguration(101, {
			enabled: true,
			delay: "1s",
		});
		await t.moveInBlack.expectConfiguration(102, {
			enabled: false,
			delay: "1s",
		});
		await t.moveInBlack.via.ui.selectFixture(101);
		await t.moveInBlack.via.ui.setEnabled(101, false);
		await t.moveInBlack.via.ui.setEnabled(101, true);
		await t.moveInBlack.via.ui.setDelay(101, "1200ms");
		await t.moveInBlack.via.ui.setDelay(101, "1s");
		await t.moveInBlack.reopenPatch();
		await t.moveInBlack.expectConfiguration(101, {
			enabled: true,
			delay: "1s",
		});

		await t.moveInBlack.reset();
		await t.playback.open();
		await t.playback.go(1);
		await t.playback.go(1);

		await t.clock.advanceBy("1999ms");
		await t.expectFixtureValue(fixture(101), { pan: 0.2 });
		await t.moveInBlack.expectState(101, {
			state: "blocked",
			currentCue: 2,
			targetCue: 3,
		});

		await t.clock.advanceBy("1ms");
		await t.expectFixtureValue(fixture(101), { intensity: 0 });
		await t.moveInBlack.expectState(101, {
			state: "delaying",
			currentCue: 2,
			targetCue: 3,
		});
		await t.moveInBlack.expectSafetyDelay(101, "1s");
		await t.moveInBlack.expectState(102, {
			state: "disabled",
			currentCue: 2,
			targetCue: 3,
		});

		await t.clock.advanceBy("999ms");
		await t.expectFixtureValue(fixture(101), { pan: 0.2 });
		await t.clock.advanceBy("1ms");
		await t.moveInBlack.expectState(101, {
			state: "moving",
			currentCue: 2,
			targetCue: 3,
		});
		await t.expectFixtureValue(fixture(101), { pan: 0.2 });

		await t.clock.advanceBy("1500ms");
		await t.expectFixtureValue(fixture(101), { intensity: 0, pan: 0.5 });
		await t.expectFixtureValue(fixture(102), { pan: 0.2 });
		await t.clock.advanceBy("1500ms");
		await t.expectFixtureValue(fixture(101), { intensity: 0, pan: 0.8 });
		await t.expectFixtureValue(fixture(102), { pan: 0.2 });
		await t.moveInBlack.expectState(101, {
			state: "completed",
			currentCue: 2,
			targetCue: 3,
		});

		await t.playback.go(1);
		await t.clock.advanceBy("0ms");
		await t.expectFixtureValue(fixture(101), { pan: 0.8 });
		await t.expectFixtureValue(fixture(102), { pan: 0.2 });
		await t.clock.advanceBy("1500ms");
		await t.expectFixtureValue(fixture(101), { pan: 0.8 });
		await t.expectFixtureValue(fixture(102), { pan: 0.5 });
	},
);
