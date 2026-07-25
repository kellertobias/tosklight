// @bench-semantic-world

import { scenario } from "./bench/core/scenario";
import { ProgrammerToken } from "./bench/encoders/encoderCatalog";
import { fixture } from "./bench/output/fixtureDmxContract";
import { Show } from "./bench/show/showScenario";

scenario(
	"COMMAND-HISTORY-001",
	"Command Line history shows accepted and rejected desk commands once",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.command.history.expectAcceptedAndRejected();
	},
);

scenario(
	"PROG-002",
	"software encoder value dialog spreads a two-point range over the ordered selection",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.command.execute("FIXTURE 1 THRU 5");
		await t.encoder.intensity.dimmer.via.ui.set([
			0,
			ProgrammerToken.Thru,
			50,
		]);
		await t.clock.advanceBy("3s");
		for (const [number, value] of [0, 32, 64, 96, 128].entries())
			await t.expectFixtureDMX(fixture(number + 1), { Intensity: value });
	},
);

scenario(
	"PROG-002",
	"software encoder value dialog lands a multi-point intensity spread once over the ordered selection",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.command.execute("FIXTURE 1 THRU 5");
		await t.encoder.intensity.dimmer.via.ui.set([
			100,
			ProgrammerToken.Thru,
			0,
			ProgrammerToken.Thru,
			100,
		]);
		await t.clock.advanceBy("3s");
		for (const [number, value] of [255, 128, 0, 128, 255].entries())
			await t.expectFixtureDMX(fixture(number + 1), { Intensity: value });
	},
);

scenario(
	"PROG-002",
	"software encoder value dialog spreads a multi-point Pan over the ordered moving-head selection",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.command.execute("FIXTURE 101 THRU 105");
		await t.encoder.position.pan.via.ui.set([
			100,
			ProgrammerToken.Thru,
			0,
			ProgrammerToken.Thru,
			100,
		]);
		await t.clock.advanceBy("3s");
		for (const [number, value] of [255, 128, 0, 128, 255].entries())
			await t.expectFixtureDMX(fixture(number + 101), {
				Pan: value,
			});
	},
);

scenario(
	"ENCODER-DISPLAY-001",
	"OSC NAV wraps families while non-first encoder turn, held-turn, and click follow the displayed cell",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.command.execute("FIXTURE 101");
		await t.attachedEncoder.expectNavigationAndSecondaryEncoder();
	},
);

scenario(
	"PROG-002",
	"hardware encoder modal spreads a typed value over the ordered selection",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.command.execute("FIXTURE 1 THRU 5");
		await t.attachedEncoder.expectTypedIntensitySpread();
		await t.clock.advanceBy("3s");
		for (const [number, value] of [0, 32, 64, 96, 128].entries())
			await t.expectFixtureDMX(fixture(number + 1), { Intensity: value });
	},
);

scenario(
	"PROG-002",
	"hardware encoder modal lands a multi-point intensity spread once over the ordered selection",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.command.execute("FIXTURE 1 THRU 5");
		await t.attachedEncoder.expectMultiPointIntensitySpread();
		await t.clock.advanceBy("3s");
		for (const [number, value] of [255, 128, 0, 128, 255].entries())
			await t.expectFixtureDMX(fixture(number + 1), { Intensity: value });
	},
);

scenario(
	"PROG-002",
	"hardware encoder modal spreads a multi-point Pan over the ordered moving-head selection",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.command.execute("FIXTURE 101 THRU 105");
		await t.attachedEncoder.expectMultiPointPanSpread();
		await t.clock.advanceBy("3s");
		for (const [number, value] of [255, 128, 0, 128, 255].entries())
			await t.expectFixtureDMX(fixture(number + 101), {
				Pan: value,
			});
	},
);
