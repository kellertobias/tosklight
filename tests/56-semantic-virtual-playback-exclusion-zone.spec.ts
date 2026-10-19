// @bench-semantic-world

import { fixture } from "./bench/command-selection/selectionContract";
import { scenario } from "./bench/core/scenario";
import { expectPlaybackPaneFits } from "./bench/playbacks/playbackPaneGeometry";
import { PlaybackButton } from "./bench/playbacks/playbackScenario";
import { Show } from "./bench/show/showScenario";
import { PaneType } from "./bench/window-system/paneTypes";

scenario(
	"VPB-007",
	"show-owned Solo Region selection is inert, saved winners are authoritative, and regions are explicitly removable",
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
			return playback;
		};

		const firstSource = await recordSource(5, 3, 25, "Touring A");
		const secondSource = await recordSource(6, 4, 50, "Touring B");
		const thirdSource = await recordSource(7, 5, 75, "Touring C");

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
			{ rows: 3, columns: 6 },
		);
		await desktop.apply();
		await expectPlaybackPaneFits(pane, 3, 6);

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
		const thirdPlayback = await t.virtualPlayback.assignSource(
			pane,
			"Touring C",
			3,
		);
		await t.virtualPlayback.activate(pane, 1);
		await t.virtualPlayback.activate(pane, 2);
		await t.virtualPlayback.expect.runtime(firstPlayback, { runtime: { enabled: true } });
		await t.virtualPlayback.expect.runtime(secondPlayback, { runtime: { enabled: true } });

		await t.virtualPlayback.createSoloRegionWithAttachedShift(
			pane,
			"Touring pair",
			[1, 2],
		);
		await t.virtualPlayback.expect.zones([
			{ name: "Touring pair", members: [{ page: 1, position: 401 }, { page: 1, position: 402 }] },
		]);
		await t.virtualPlayback.expect.fence(pane, 1, "top bottom left");
		await t.virtualPlayback.expect.fence(pane, 2, "top right bottom");
		// Six authored columns fill the width and three rows fill the height independently.
		// Recheck actual SVG strokes after each resize, not only member CSS classes.
		for (const size of [{ width: 16, height: 15 }, { width: 10, height: 10 }]) {
			await pane.resize(size);
			await expectPlaybackPaneFits(pane, 3, 6);
			await t.virtualPlayback.expect.fence(pane, 1, "top bottom left");
			await t.virtualPlayback.expect.fence(pane, 2, "top right bottom");
		}
		// Saving the new region reconciles permanent owners by their activation order.
		// Selection above is inert; the newer second Playback is the exact saved winner.
		await t.virtualPlayback.expect.runtime(firstPlayback, {
			identity: {
				kind: "playback",
				page: 1,
				position: 401,
			},
			target: "cue_list",
			runtime: { enabled: false },
		});
		await t.virtualPlayback.expect.runtime(secondPlayback, {
			identity: {
				kind: "playback",
				page: 1,
				position: 402,
			},
			target: "cue_list",
			runtime: { enabled: true },
		});

		await t.virtualPlayback.activate(pane, 1);
		await t.virtualPlayback.activate(pane, 2);
		await t.virtualPlayback.activate(pane, 1);
		await t.virtualPlayback.activate(pane, 2);
		await t.virtualPlayback.expect.runtime(firstPlayback, {
			target: "cue_list",
			runtime: { enabled: false },
		});
		await t.virtualPlayback.expect.runtime(secondPlayback, {
			target: "cue_list",
			runtime: { enabled: true },
		});
		await t.virtualPlayback.expect.runtime(thirdPlayback, {
			target: "cue_list",
			runtime: { enabled: false },
		});
		await t.playback.expect(firstSource).runtime({ enabled: false });
        await t.playback.expect(secondSource).runtime({ enabled: true });
        await t.playback.expect(thirdSource).runtime({ enabled: false });
		await t.clock.advanceStep();
		await t.expectFixtureValue(fixture(3), { intensity: 0 });
		await t.expectFixtureValue(fixture(4), { intensity: 0.5 });
		await t.expectFixtureValue(fixture(5), { intensity: 0 });

		await t.virtualPlayback.deleteSoloRegion(pane, "Touring pair");
		await t.virtualPlayback.expect.zones([]);
		await t.virtualPlayback.activate(pane, 2);
		await t.virtualPlayback.activate(pane, 1);
		await t.virtualPlayback.activate(pane, 2);
		await t.virtualPlayback.expect.runtime(firstPlayback, {
			runtime: { enabled: true },
		});
		await t.virtualPlayback.expect.runtime(secondPlayback, {
			runtime: { enabled: true },
		});
	},
);
