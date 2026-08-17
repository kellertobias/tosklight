// @bench-semantic-world

import { scenario } from "./bench/core/scenario";
import { Show } from "./bench/show/showScenario";

scenario(
	"POSITION-HOME-001",
	"Return Home applies per-head Position defaults as one programmer gesture",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.special.position.prepareReturnHomeContract();

		await t.special.position.returnHome();
		await t.special.position.expectAtHome();
		await t.keypad.press(["SHIFT", "ESC", "SHIFT"]);
		await t.special.position.expectBeforeReturnHome();

		await t.hardware.connect();
		try {
			await t.special.position.returnHome();
		} finally {
			await t.hardware.disconnect();
		}
		await t.special.position.expectAtHome();

		await t.keypad.press(["CLR"]);
		await t.expect.selection();
		await t.special.position.expectUnavailable();
	},
);

scenario(
	"COLOR-RANGE-001",
	"Shift-drag applies an ordered Color range from software and attached hardware",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.special.color.prepareRangeContract();

		await t.special.color.setUniform();
		await t.keypad.press(["SHIFT", "ESC", "SHIFT"]);
		await t.special.color.expectPrior();

		await t.special.color.applyRangeWithShift();
		await t.keypad.press(["SHIFT", "ESC", "SHIFT"]);
		await t.special.color.expectPrior();
		await t.special.color.cancelRangeWithShift();

		await t.hardware.connect();
		try {
			await t.special.color.applyRangeWithHardwareShift();
		} finally {
			await t.hardware.disconnect();
		}
		await t.special.color.expectRange();
		await t.special.color.expectSelectionPreserved();
	},
);

scenario(
	"PLAYBACK-SELECT-001",
	"an attached-hardware playback card selects its concrete Cuelist playback",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.selection.fixtures.via.api.item(101);
		await t.encoder.intensity.dimmer.via.api.set(73);
		const playback = await t.record.playback(1);
		await t.playback.nameTargetCuelist(playback, "Front Cuelist");
		await t.encoder.clear();

		await t.hardware.connect();
		try {
			await t.playback.selectFromHardwareCard(playback);
		} finally {
			await t.hardware.disconnect();
		}
		await t.playback.expect(playback).selected();
	},
);
