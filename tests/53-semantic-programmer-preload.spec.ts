// @bench-semantic-world

import { fixture } from "./bench/command-selection/selectionContract";
import { scenario } from "./bench/core/scenario";
import { Show } from "./bench/show/showScenario";

scenario(
	"PRELOAD-001",
	"programmer-only Preload stays blind, commits, and releases",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");

		await t.clock.advanceStep();
		await t.expectFixtureDMX(fixture(1), { Intensity: 0 });

		await t.preload.start();
		await t.preload.expect.active();
		await t.preload.setFixtureValue({
			fixture: 1,
			attribute: "intensity",
			value: { kind: "normalized", value: 0.5 },
		});
		await t.clock.advanceStep();
		await t.expectFixtureDMX(fixture(1), { Intensity: 0 });

		await t.preload.via.api.commit();
		await t.clock.advanceBy("0ms");
		await t.expectFixtureDMX(fixture(1), { Intensity: 128 });

		await t.preload.release();
		await t.preload.expect.inactive();
		await t.clock.advanceStep();
		await t.expectFixtureDMX(fixture(1), { Intensity: 0 });
	},
);
