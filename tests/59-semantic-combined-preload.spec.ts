// @bench-semantic-world

import { fixture } from "./bench/command-selection/selectionContract";
import { scenario } from "./bench/core/scenario";
import { PlaybackButton } from "./bench/playbacks/playbackScenario";
import { Show } from "./bench/show/showScenario";
import { PaneType } from "./bench/window-system/paneTypes";

scenario(
	"PRELOAD-006",
	"combined Preload commits atomically and releases only programmer data",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");
		await t.timing.cueFade.set("0s");

		const recordSource = async (
			slot: number,
			fixtureNumber: number,
			intensity: number,
			name: string,
			button: PlaybackButton,
		) => {
			await t.selection.clear();
			await t.selection.fixtures.via.api.item(fixtureNumber);
			await t.encoder.intensity.dimmer.via.api.set(intensity);
			const playback = await t.record.playback(slot);
			await t.encoder.clear();
			await t.playback.configure(playback, {
				name,
				buttonCount: 1,
				hasFader: false,
				buttons: [button, PlaybackButton.Empty, PlaybackButton.Empty],
			});
			await t.playback.via.api.off(playback);
			return playback;
		};

		const underlying = await recordSource(
			5,
			1,
			25,
			"Underlying source",
			PlaybackButton.Go,
		);
		const physicalPlayback = await recordSource(
			1,
			3,
			60,
			"Physical combined",
			PlaybackButton.Go,
		);
		await recordSource(6, 4, 80, "Virtual combined", PlaybackButton.Toggle);
		await t.preload.configure({
			programmer: true,
			physicalPlaybacks: true,
			virtualPlaybacks: true,
			programmerFade: 1_500,
			cueFade: 8_000,
		});
		await t.playback.via.api.go(underlying);
		await t.clock.advanceBy("8s");
		await t.expectFixtureValue(fixture(1), { intensity: 0.25 });

		const desktop = t.desktop.configure("Combined Preload");
		const pane = desktop.addPane(
			PaneType.VirtualPlaybacks,
			{
				slug: "combined-preload",
				column: 1,
				row: 1,
				width: 12,
				height: 10,
			},
			{ rows: 1, columns: 2 },
		);
		await desktop.apply();
		const virtualPlayback = await t.virtualPlayback.assignSource(
			pane,
			"Virtual combined",
			2,
		);

		await t.preload.start();
		await t.command.execute("GROUP 1 AT 80");
		await t.playback.open();
		await t.playback.go(physicalPlayback);
		await t.virtualPlayback.activate(pane, 2);
		await t.preload.expect.pending({
			groupIds: ["1"],
			playbackActions: [
				[physicalPlayback, "go", "physical"],
				[virtualPlayback, "toggle", "virtual"],
			],
		});

		await t.preload.commit();
		await t.preload.expect.atomicCommit("1", [
			physicalPlayback,
			virtualPlayback,
		]);
		await t.clock.advanceBy("1500ms");
		await t.expectFixtureValue(fixture(1), { intensity: 0.8 });
		await t.playback.expect(physicalPlayback).runtime({
			enabled: true,
			current_cue_number: 1,
		});
		await t.playback.expect(virtualPlayback).runtime({
			enabled: true,
			current_cue_number: 1,
		});

		await t.preload.via.ui.release();
		await t.expectFixtureValue(fixture(1), { intensity: 0.25 });
		await t.playback.expect(physicalPlayback).runtime({ enabled: true });
		await t.playback.expect(virtualPlayback).runtime({ enabled: true });
	},
);
