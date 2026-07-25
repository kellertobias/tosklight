// @bench-semantic-world

import { fixture } from "../apps/control-ui/e2e/bench/command-selection/selectionContract";
import { scenario } from "../apps/control-ui/e2e/bench/core/scenario";
import { PlaybackButton } from "../apps/control-ui/e2e/bench/playbacks/playbackScenario";
import { Show } from "../apps/control-ui/e2e/bench/show/showScenario";
import { PaneType } from "../apps/control-ui/e2e/bench/window-system/paneTypes";

scenario(
	"VPB-007",
	"named Virtual Playback exclusion zones are inert on creation and authoritative on activation",
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
				buttons: [
					PlaybackButton.Toggle,
					PlaybackButton.Empty,
					PlaybackButton.Empty,
				],
			});
			await t.playback.via.api.off(playback);
		};

		await recordSource(5, 3, 25, "Touring A");
		await recordSource(6, 4, 50, "Touring B");
		await recordSource(7, 5, 75, "Touring C");

		const desktop = t.desktop.configure("Touring Virtual Playbacks");
		const pane = desktop.addPane(
			PaneType.VirtualPlaybacks,
			{
				slug: "touring-virtual-playbacks",
				column: 1,
				row: 1,
				width: 12,
				height: 10,
			},
			{ rows: 1, columns: 3 },
		);
		await desktop.apply();

		const firstPlayback = await t.virtualPlayback.assignSource(
			pane,
			"Touring A",
			1,
		);
		const secondPlayback = await t.virtualPlayback.assignSource(
			pane,
			"Touring B",
			2,
		);
		await t.virtualPlayback.assignSource(pane, "Touring C", 3);
		await t.playback.via.api.on(firstPlayback);
		await t.playback.via.api.on(secondPlayback);

		await t.virtualPlayback.createExclusionZone(pane, "Touring pair", [1, 2]);
		await t.virtualPlayback.expect.zones([
			{ name: "Touring pair", slots: [1, 2] },
		]);
		await t.playback.expect(firstPlayback).runtime({ enabled: true });
		await t.playback.expect(secondPlayback).runtime({ enabled: true });

		await t.virtualPlayback.activate(pane, 1);
		await t.virtualPlayback.activate(pane, 2);
		await t.virtualPlayback.activate(pane, 1);
		await t.virtualPlayback.activate(pane, 2);
		await t.playback.expect(firstPlayback).runtime({ enabled: false });
		await t.playback.expect(secondPlayback).runtime({ enabled: true });
		await t.clock.advanceStep();
		await t.expectFixtureValue(fixture(3), { intensity: 0 });
		await t.expectFixtureValue(fixture(4), { intensity: 0.5 });
		await t.expectFixtureValue(fixture(5), { intensity: 0 });
	},
);
