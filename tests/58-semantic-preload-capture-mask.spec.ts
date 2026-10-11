// @bench-semantic-world

import type { PlaybackPreloadScope } from "../apps/light-desktop/src/api/generated/light-wire";
import { scenario } from "./bench/core/scenario";
import { Show } from "./bench/show/showScenario";

scenario(
	"PRELOAD-005",
	"programmer capture and four range/page eligibility scopes persist independently",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();

		const scopes: PlaybackPreloadScope[] = [
			{ position_ranges: [{ start: 1, end: 1000 }], pages: null },
			{ position_ranges: [{ start: 1, end: 15 }], pages: null },
			{
				position_ranges: [
					{ start: 301, end: 305 },
					{ start: 401, end: 680 },
				],
				pages: [1, 2],
			},
			{ position_ranges: [{ start: 101, end: 115 }], pages: [99] },
		];
		const settings = await t.preload.openSettings();
		for (const playbackScope of scopes) {
			for (const programmer of [false, true]) {
				const mask = { programmer, playbackScope };
				await settings.configure(mask);
				await settings.expect.mask(mask);
			}
		}
	},
);
