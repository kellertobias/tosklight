// @bench-semantic-world

import { expect } from "@playwright/test";
import { scenario } from "./bench/core/scenario";
import { PlaybackButton } from "./bench/playbacks/playbackScenario";
import { virtualPlaybackNumber } from "./bench/playbacks/virtualPlaybackScenario";
import { Show } from "./bench/show/showScenario";
import { PaneType } from "./bench/window-system/paneTypes";

scenario(
	"PRELOAD-003",
	"Virtual Playbacks persist show-owned 300-number page banks in Follow Main and Pinned grids",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");
		await t.timing.cueFade.set("0s");
		expect(virtualPlaybackNumber(1, 1)).toBe(1001);
		expect(virtualPlaybackNumber(300, 1)).toBe(1300);
		expect(virtualPlaybackNumber(1, 2)).toBe(1301);
		expect(virtualPlaybackNumber(300, 2)).toBe(1600);
		expect(virtualPlaybackNumber(1, 3)).toBe(1601);
		expect(virtualPlaybackNumber(300, 127)).toBe(39_100);
		expect(() => virtualPlaybackNumber(301, 1)).toThrow(
			"Virtual Playback cell must be within 1-300",
		);

		const recordSource = async (
			slot: number,
			fixture: number,
			name: string,
		) => {
			await t.selection.clear();
			await t.selection.fixtures.via.api.item(fixture);
			await t.encoder.intensity.dimmer.via.api.set(20);
			const playback = await t.record.playback(slot);
			await t.encoder.clear();
			await t.encoder.intensity.dimmer.via.api.set(80);
			await t.record.cue({ playback, cue: 2 });
			await t.encoder.clear();
			await t.playback.configure(playback, {
				name,
				buttonCount: 1,
				hasFader: false,
				buttons: [
					PlaybackButton.Go,
					PlaybackButton.Empty,
					PlaybackButton.Empty,
				],
			});
			await t.playback.via.api.off(playback);
			return playback;
		};

		const firstSource = await recordSource(5, 3, "Virtual Source A");
		const secondSource = await recordSource(6, 4, "Virtual Source B");

		const desktop = t.desktop.configure("Virtual Playback Desktop");
		const followPane = desktop.addPane(
			PaneType.VirtualPlaybacks,
			{
				slug: "virtual-playbacks-follow",
				column: 1,
				row: 1,
				width: 12,
				height: 5,
			},
			{ rows: 20, columns: 15, pageMode: "follow_main" },
		);
		const pinnedPane = desktop.addPane(
			PaneType.VirtualPlaybacks,
			{
				slug: "virtual-playbacks-pinned",
				column: 1,
				row: 6,
				width: 12,
				height: 5,
			},
			{ rows: 10, columns: 15, pageMode: "pinned", pinnedPage: 2 },
		);
		await desktop.apply();
		await t.virtualPlayback.expect.logicalCells(followPane, 300);
		await t.virtualPlayback.expect.mountedCellsBelow(followPane, 200);
		await t.virtualPlayback.expect.effectivePage(followPane, 1);
		await t.virtualPlayback.expect.logicalCells(pinnedPane, 150);
		await t.virtualPlayback.expect.effectivePage(pinnedPane, 2);

		const firstPlayback = await t.virtualPlayback.assignSource(
			followPane,
			"Virtual Source A",
			1,
			1,
		);
		const secondPlayback = await t.virtualPlayback.assignSource(
			pinnedPane,
			"Virtual Source B",
			1,
			2,
		);
		await t.virtualPlayback.configureTopButton(
			pinnedPane,
			1,
			PlaybackButton.Toggle,
			2,
		);

		await t.virtualPlayback.activate(followPane, 1, 1);
		await t.virtualPlayback.activate(pinnedPane, 1, 2);
		await t.virtualPlayback.expect.runtime(firstPlayback, {
			requested: {
				kind: "virtual",
				page: 1,
				playback_number: 1001,
			},
			target: "cue_list",
			runtime: { enabled: true, current: { number: "1" } },
		});
		await t.virtualPlayback.expect.runtime(secondPlayback, {
			requested: {
				kind: "virtual",
				page: 2,
				playback_number: 1301,
			},
			target: "cue_list",
			runtime: { enabled: true, current: { number: "1" } },
		});
		await t.virtualPlayback.expect.physicalRuntimeAbsent(firstSource);
		await t.virtualPlayback.expect.physicalRuntimeAbsent(secondSource);
		await t.playback.expect(firstSource).present();
		await t.playback.expect(secondSource).present();

		await t.virtualPlayback.setMainPage(2);
		await t.virtualPlayback.expect.effectivePage(followPane, 2);
		await t.virtualPlayback.expect.effectivePage(pinnedPane, 2);
		await t.virtualPlayback.setMainPage(1);
		await t.virtualPlayback.expect.effectivePage(followPane, 1);
		await t.virtualPlayback.expect.effectivePage(pinnedPane, 2);

		await t.virtualPlayback.reload(followPane);
		await t.virtualPlayback.expect.logicalCells(followPane, 300);
		await t.virtualPlayback.expect.logicalCells(pinnedPane, 150);
		await t.virtualPlayback.expect.button(followPane, 1, "GO", 1);
		await t.virtualPlayback.expect.button(pinnedPane, 1, "TOGGLE", 2);
		await t.virtualPlayback.expect.assignment(firstPlayback, {
			number: 1001,
			button_count: 1,
			has_fader: false,
			buttons: ["go", "none", "none"],
		});
		await t.virtualPlayback.expect.assignment(secondPlayback, {
			number: 1301,
			button_count: 1,
			has_fader: false,
			buttons: ["toggle", "none", "none"],
		});
	},
);
