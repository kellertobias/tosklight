import { fixture as dmxFixture } from "../../apps/control-ui/e2e/bench/fixtureDmx";
import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import { scenario } from "../../apps/control-ui/e2e/bench/scenario";
import { fixture } from "../../apps/control-ui/e2e/bench/selectionContract";
import { Show } from "../../apps/control-ui/e2e/bench/showScenario";

scenario(
	"BENCH-PROGRAMMER-FADE-001",
	"API set, double, half, and off retain exact Programmer timing",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("4s");
		expect(await t.timing.programmerFade.currentMillis()).toBe(4_000);

		await t.selection.fixtures.via.api.item(1);
		await t.expect.selection(fixture(1));
		await t.encoder.intensity.dimmer.via.api.set(100);
		await t.clock.advanceBy("2s");
		await t.expectFixtureDMX(dmxFixture(1), { Intensity: 128 });
		await t.clock.advanceBy("2s");
		await t.expectFixtureDMX(dmxFixture(1), { Intensity: 255 });

		await t.timing.programmerFade.double();
		expect(await t.timing.programmerFade.currentMillis()).toBe(8_000);
		await t.timing.programmerFade.half();
		expect(await t.timing.programmerFade.currentMillis()).toBe(4_000);
		await t.timing.programmerFade.off();
		expect(await t.timing.programmerFade.currentMillis()).toBe(0);
	},
);

scenario(
	"BENCH-PROGRAMMER-FADE-002",
	"visible value entry and pointer fader converge on the same authority",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();

		await t.timing.programmerFade.via.valueEntry.set("2s");
		expect(await t.timing.programmerFade.currentMillis()).toBe(2_000);
		await t.timing.programmerFade.via.fader.set("4s");
		expect(await t.timing.programmerFade.currentMillis()).toBe(4_000);

		await t.timing.programmerFade.set("1s");
		expect(t.timing.programmerFade.routeReports.at(-1)).toMatchObject({
			operation: "set",
			duration: "1s",
			candidates: ["api", "fader", "valueEntry"],
			selected: expect.stringMatching(/^(api|fader|valueEntry)$/),
		});
	},
);

scenario(
	"BENCH-PROGRAMMER-FADE-003",
	"attached-hardware OSC fader writes the shared Programmer Fade authority",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.hardware.connect();
		try {
			await t.timing.programmerFade.via.osc.set("5s");
			expect(await t.timing.programmerFade.currentMillis()).toBe(5_000);
		} finally {
			await t.hardware.disconnect();
		}
	},
);
