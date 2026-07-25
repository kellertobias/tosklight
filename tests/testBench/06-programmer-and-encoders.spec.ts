import {
	fixture as dmxFixture,
} from "../../apps/control-ui/e2e/bench/output/fixtureDmx";
import { expect } from "../../apps/control-ui/e2e/bench/core/fixtures";
import { scenario } from "../../apps/control-ui/e2e/bench/core/scenario";
import {
	fixture,
	fixtureRange,
} from "../../apps/control-ui/e2e/bench/command-selection/selectionContract";
import { Show } from "../../apps/control-ui/e2e/bench/show/showScenario";
import { ProgrammerToken } from "../../apps/control-ui/e2e/bench/encoders/encoderCatalog";

scenario(
	"BENCH-ENCODER-002",
	"Position Pan resolves its live software encoder without exposing a physical slot",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.show.expect.active(Show.DefaultStage);
		await t.selection.fixtures.via.fixtureSheet.item(101);
		await t.expect.selection(fixture(101));

		await t.encoder.position.pan.via.ui.set(25);
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(dmxFixture(101), { Pan: 64 });
	},
);

scenario(
	"BENCH-ENCODER-003",
	"typed multi-point THRU entry spreads over the ordered selection",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.show.expect.active(Show.DefaultStage);
		await t.selection.fixtures.via.fixtureSheet.range(1, 5);
		await t.expect.selection(fixtureRange(1, 5));

		await t.encoder.intensity.dimmer.via.ui.set([
			100,
			ProgrammerToken.Thru,
			0,
			ProgrammerToken.Thru,
			100,
		]);
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(dmxFixture(1), { Intensity: 255 });
		await t.expectFixtureDMX(dmxFixture(3), { Intensity: 0 });
		await t.expectFixtureDMX(dmxFixture(5), { Intensity: 255 });

		await t.encoder.intensity.dimmer.set(50);
		const report = t.encoder.routeReports.at(-1);
		expect(report).toMatchObject({
			operation: "set",
			group: "intensity",
			attribute: "dimmer",
			candidates: ["api", "ui"],
			selected: expect.stringMatching(/^(api|ui)$/),
		});
		await t.expect.selection(fixture(1), fixture(2), fixture(3), fixture(4), fixture(5));
	},
);

scenario(
	"BENCH-ENCODER-004",
	"OSC relative detents resolve Tilt from the live attached-hardware slot",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.show.expect.active(Show.DefaultStage);
		await t.selection.fixtures.via.fixtureSheet.item(101);
		await t.expect.selection(fixture(101));
		await t.hardware.connect();
		try {
			await t.encoder.position.tilt.via.osc.add(1);
			await t.clock.advanceBy("3s");
			await t.expectFixtureDMX(dmxFixture(101), { Tilt: 131 });
		} finally {
			await t.hardware.disconnect();
		}
	},
);
