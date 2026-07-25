import { expect } from "../../apps/control-ui/e2e/bench/core/fixtures";
import { scenario } from "../../apps/control-ui/e2e/bench/core/scenario";
import { Show } from "../../apps/control-ui/e2e/bench/show/showScenario";
import { SpeedGroup } from "../../apps/control-ui/e2e/bench/playbacks/speedGroupScenario";

scenario(
	"BENCH-CUE-FADE-001",
	"Cue Fade remains separate from Programmer Fade and explicit Cue timing",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("1s");
		await t.timing.cueFade.set("4s");
		await t.timing.cueFade.double();
		await t.timing.cueFade.expectMillis(8_000);
		await t.timing.cueFade.half();
		await t.timing.cueFade.expectMillis(4_000);
		await t.timing.cueFade.off();
		await t.timing.cueFade.expectMillis(0);
		await t.timing.cueFade.set("4s");
		expect(await t.timing.programmerFade.currentMillis()).toBe(1_000);

		await t.selection.fixtures.via.api.item(101);
		await t.encoder.intensity.dimmer.via.api.set(20);
		const playback = await t.record.playback(1);
		await t.encoder.intensity.dimmer.via.api.set(80);
		await t.record.cue({ playback, cue: 2 });
		await t.encoder.intensity.dimmer.via.api.set(100);
		await t.record.cue({ playback, cue: 3, timing: { fade: "1" } });
		await t.cue.expect(playback, 3).metadata({ fade_millis: 1_000 });
		await t.encoder.clear();

		await t.cue.via.api.goto(playback, 1);
		await t.clock.advanceBy("4s");
		await t.playback.via.api.go(playback);
		await t.clock.advanceBy("2s");
		await t.expectFixtureDMX({ fixture: 101 }, { Intensity: 128 });
		await t.clock.advanceBy("2s");
		await t.playback.via.api.go(playback);
		await t.clock.advanceBy("500ms");
		await t.expectFixtureDMX({ fixture: 101 }, { Intensity: 230 });
	},
);

scenario(
	"BENCH-SPEED-GROUP-001",
	"enum-backed Speed Groups retain reproducible tap and settings semantics",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();

		for (const [group, bpm] of [
			[SpeedGroup.A, 100],
			[SpeedGroup.B, 110],
			[SpeedGroup.C, 120],
			[SpeedGroup.D, 130],
			[SpeedGroup.E, 140],
		] as const)
			await t.speedGroup[group].setBpm(bpm);
		await t.speedGroup.A.addBpm(5);
		await t.speedGroup.A.subtractBpm(5);
		await t.speedGroup.A.expect.bpm(100);

		await t.speedGroup.B.synchronizeFrom(SpeedGroup.A);
		await t.speedGroup.B.expect.synchronizedFrom(SpeedGroup.A);
		await t.speedGroup.B.setBpm(150);
		await t.speedGroup.B.expect.synchronizedFrom(null);
		await t.speedGroup.B.synchronizeFrom(SpeedGroup.A);

		const report = await t.speedGroup.B.via.click.tapTempo(80);
		expect(t.speedGroup.B.replayIntervals(report)).toEqual(
			report.intervalsMillis,
		);
		await t.speedGroup.B.expect.synchronizedFrom(null);
		await t.speedGroup.B.expect.bpmWithin(80, 2);

		await t.speedGroup.C.via.shiftClick.openSettings();
		await t.speedGroup.C.closeSettings();
		await t.speedGroup.D.via.hold.openSettings();
		await t.speedGroup.D.closeSettings();

		expect(t.speedGroup.reports).toContainEqual(report);
		expect(Object.values(SpeedGroup)).toEqual(["A", "B", "C", "D", "E"]);
	},
);
