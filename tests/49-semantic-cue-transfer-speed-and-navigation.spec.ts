// @bench-semantic-world

import { fixture } from "./bench/command-selection/selectionContract";
import { scenario } from "./bench/core/scenario";
import { PlaybackButton } from "./bench/playbacks/playbackScenario";
import { Show } from "./bench/show/showScenario";

scenario(
	"CUE-009",
	"explicit Plain/Status Move/Copy choices preserve both independent axes",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();

		await t.selection.fixtures.via.api.item(1);
		await t.encoder.intensity.dimmer.via.api.set(25);
		const source = await t.record.playback(1);
		await t.encoder.clear();
		await t.encoder.intensity.dimmer.via.api.set(50);
		await t.record.cue({ playback: source, cue: 2 });
		await t.encoder.clear();
		await t.encoder.intensity.dimmer.via.api.set(75);
		const destination = await t.record.playback(2);
		await t.encoder.clear();

		await t.cue.expect(source, 2).present();
		await t.cue.expect(destination, 2).absent();
		await t.command.execute("COPY SET 1 CUE 2 AT SET 2 CUE 2");
		await t.cue.transferChoice("COPY").cancel();
		await t.cue.expect(source, 2).present();
		await t.cue.expect(destination, 2).absent();
	},
);

scenario(
	"CMD-002",
	"Speed Group commands address, synchronize, display, and manually unlink all five groups",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();

		await t.command.execute("SPD GRP 1 AT 120");
		await t.speedGroup.A.expect.bpm(120);
		await t.command.execute("SPD GRP 2 AT 127.5");
		await t.speedGroup.B.expect.bpm(127.5);
		await t.command.execute("SPD GRP 3 AT 131");
		await t.speedGroup.C.expect.bpm(131);
		await t.command.execute("SPD GRP 4 AT 142");
		await t.speedGroup.D.expect.bpm(142);
		await t.command.execute("SPD GRP 5 AT 153");
		await t.speedGroup.E.expect.bpm(153);

		await t.command.execute("SPD GRP 1 AT + 5");
		await t.speedGroup.A.expect.bpm(125);
		await t.command.execute("SPD GRP 1 AT - 5");
		await t.speedGroup.A.expect.bpm(120);
		await t.command.execute("SPD GRP 3 AT 90");
		await t.command.execute("SPD GRP 1 AT SPD GRP 3");
		await t.speedGroup.C.expect.synchronizedFrom(t.speedGroup.A.group);
		await t.speedGroup.C.expect.bpm(120);

		await t.speedGroup.A.via.click.tapTempo(80, 5);
		await t.speedGroup.A.expect.bpmWithin(80, 2);
		await t.speedGroup.A.expect.synchronizedFrom(null);
		await t.speedGroup.C.expect.bpm(120);
		await t.speedGroup.C.expect.synchronizedFrom(null);

		await t.command.execute("SPD GRP 1 AT 85");
		await t.speedGroup.A.expect.bpm(85);
		await t.speedGroup.C.expect.bpm(120);
	},
);

scenario(
	"CUE-014",
	"Cue Go To and Load preserve desk-local selection and authoritative output controls",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");
		await t.timing.cueFade.set("3s");

		const recordSequence = async (slot: number, name: string) => {
			await t.selection.fixtures.via.api.item(1);
			await t.encoder.intensity.dimmer.via.api.set(20);
			const playback = await t.record.playback(slot);
			await t.encoder.clear();
			await t.encoder.intensity.dimmer.via.api.set(50);
			await t.record.cue({ playback, cue: 2 });
			await t.encoder.clear();
			await t.encoder.intensity.dimmer.via.api.set(80);
			await t.record.cue({ playback, cue: 3 });
			await t.encoder.clear();
			await t.playback.configure(playback, {
				name,
				buttons: [PlaybackButton.GoBack, PlaybackButton.Go, PlaybackButton.Off],
			});
			await t.playback.via.api.off(playback);
			return playback;
		};

		const first = await recordSequence(1, "Twin A");
		const second = await recordSequence(2, "Twin B");
		await t.command.clear();
		await t.playback.select(second);
		await t.playback.go(first);
		await t.playback.expect(first).runtime({ current_cue_number: "1" });
		await t.playback.expect(second).selected();

		await t.command.execute("GO TO PBK 2 CUE 3");
		await t.playback.expect(second).runtime({
			current_cue_number: "3",
			master: 1,
			enabled: true,
		});
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(fixture(1), { Intensity: 204 });

		await t.command.execute("LOAD PBK 2 CUE 2");
		await t.playback.expect(second).runtime({
			current_cue_number: "3",
			effective_next_cue_number: "2",
			effective_next_is_loaded: true,
		});
		await t.playback.go(second);
		await t.clock.advanceBy("3s");
		await t.playback.expect(second).runtime({
			current_cue_number: "2",
			effective_next_cue_number: "3",
			effective_next_is_loaded: false,
		});
		await t.expectFixtureDMX(fixture(1), { Intensity: 128 });

		await t.command.execute("LOAD PBK 2 CUE 3");
		await t.playback.goBack(second);
		await t.playback.expect(second).runtime({
			current_cue_number: "1",
			loaded_cue_number: "3",
			effective_next_is_loaded: true,
		});
		await t.playback.via.api.off(second);
		await t.playback.expect(second).runtime({
			enabled: false,
			effective_next_is_loaded: false,
		});
	},
);
