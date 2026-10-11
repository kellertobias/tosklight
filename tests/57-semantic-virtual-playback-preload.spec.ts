// @bench-semantic-world

import { expect } from "@playwright/test";
import { fixture } from "./bench/command-selection/selectionContract";
import { scenario } from "./bench/core/scenario";
import { PlaybackButton } from "./bench/playbacks/playbackScenario";
import { Show } from "./bench/show/showScenario";
import { PaneType } from "./bench/window-system/paneTypes";

scenario(
	"PRELOAD-004",
	"virtual GO and TOGGLE alone remain pending and share Programmer Fade",
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
			button: Exclude<PlaybackButton, PlaybackButton.Release>,
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

		const goSource = await recordSource(
			5,
			3,
			100,
			"Virtual GO",
			PlaybackButton.Go,
		);
		const toggleSource = await recordSource(
			6,
			4,
			80,
			"Virtual TOGGLE",
			PlaybackButton.Toggle,
		);
		const physicalPlayback = await recordSource(
			3,
			5,
			60,
			"Physical live",
			PlaybackButton.Go,
		);

		const desktop = t.desktop.configure("Preload Virtual Playbacks");
		const pane = desktop.addPane(
			PaneType.VirtualPlaybacks,
			{
				slug: "preload-virtual-playbacks",
				column: 1,
				row: 1,
				width: 12,
				height: 10,
			},
			{ rows: 1, columns: 3 },
		);
		await desktop.apply();
		const virtualGo = await t.virtualPlayback.assignSource(
			pane,
			"Virtual GO",
			1,
		);
		const virtualToggle = await t.virtualPlayback.assignSource(
			pane,
			"Virtual TOGGLE",
			2,
		);
		await t.playback.expect(goSource).present();
		await t.playback.expect(toggleSource).present();

		await t.preload.configure({
			programmer: false,
			playbackScope: { position_ranges: [{ start: 401, end: 402 }], pages: null },
			programmerFade: 2_500,
			cueFade: 8_000,
		});
		await t.preload.start();
		await t.command.execute("1 AT 35");
		await t.playback.open();
		await t.playback.go(physicalPlayback);
		await t.virtualPlayback.activate(pane, 1);
		await t.virtualPlayback.activate(pane, 2);
		await t.preload.expect.pendingMatching({ groupIds: [], playbackActions: [
            [virtualGo, "go", { surface_id: expect.any(String), control_id: expect.any(Number) }],
            [virtualToggle, "toggle", { surface_id: expect.any(String), control_id: expect.any(Number) }],
        ] });
		await t.playback.expect(goSource).runtime({ enabled: false });
		await t.playback.expect(toggleSource).runtime({ enabled: false });
		await t.playback.expect(physicalPlayback).runtime({
			enabled: true,
			current_cue_number: "1",
		});
		await t.clock.advanceStep();
		await t.expectFixtureValue(fixture(3), { intensity: 0 });
		await t.expectFixtureValue(fixture(4), { intensity: 0 });

		await t.preload.commit();
		await t.virtualPlayback.expect.runtime(virtualGo, {
			identity: {
				kind: "playback",
				page: 1,
				position: 401,
			},
			target: "cue_list",
			runtime: { enabled: true, current: { number: "1" } },
		});
		await t.virtualPlayback.expect.runtime(virtualToggle, {
			identity: {
				kind: "playback",
				page: 1,
				position: 402,
			},
			target: "cue_list",
			runtime: { enabled: true, current: { number: "1" } },
		});
		await t.clock.advanceBy("2500ms");
		await t.expectFixtureValue(fixture(3), { intensity: 1 });
		await t.expectFixtureValue(fixture(4), { intensity: 0.8 });

		await t.preload.release();
		await t.virtualPlayback.expect.runtime(virtualGo, {
			target: "cue_list",
			runtime: { enabled: true },
		});
		await t.virtualPlayback.expect.runtime(virtualToggle, {
			target: "cue_list",
			runtime: { enabled: true },
		});
		await t.expectFixtureValue(fixture(1), { intensity: 0.35 });
	},
);
