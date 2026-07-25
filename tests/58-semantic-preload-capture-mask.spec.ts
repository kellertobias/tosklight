// @bench-semantic-world

import { scenario } from "../apps/control-ui/e2e/bench/core/scenario";
import { Show } from "../apps/control-ui/e2e/bench/show/showScenario";

scenario(
	"PRELOAD-005",
	"all eight capture-domain switch masks persist independently",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();

		const settings = await t.preload.openSettings();
		await settings.configure({
			programmer: false,
			physicalPlaybacks: false,
			virtualPlaybacks: false,
		});
		await settings.expect.mask({
			programmer: false,
			physicalPlaybacks: false,
			virtualPlaybacks: false,
		});
		await settings.configure({
			programmer: true,
			physicalPlaybacks: false,
			virtualPlaybacks: false,
		});
		await settings.expect.mask({
			programmer: true,
			physicalPlaybacks: false,
			virtualPlaybacks: false,
		});
		await settings.configure({
			programmer: false,
			physicalPlaybacks: true,
			virtualPlaybacks: false,
		});
		await settings.expect.mask({
			programmer: false,
			physicalPlaybacks: true,
			virtualPlaybacks: false,
		});
		await settings.configure({
			programmer: true,
			physicalPlaybacks: true,
			virtualPlaybacks: false,
		});
		await settings.expect.mask({
			programmer: true,
			physicalPlaybacks: true,
			virtualPlaybacks: false,
		});
		await settings.configure({
			programmer: false,
			physicalPlaybacks: false,
			virtualPlaybacks: true,
		});
		await settings.expect.mask({
			programmer: false,
			physicalPlaybacks: false,
			virtualPlaybacks: true,
		});
		await settings.configure({
			programmer: true,
			physicalPlaybacks: false,
			virtualPlaybacks: true,
		});
		await settings.expect.mask({
			programmer: true,
			physicalPlaybacks: false,
			virtualPlaybacks: true,
		});
		await settings.configure({
			programmer: false,
			physicalPlaybacks: true,
			virtualPlaybacks: true,
		});
		await settings.expect.mask({
			programmer: false,
			physicalPlaybacks: true,
			virtualPlaybacks: true,
		});
		await settings.configure({
			programmer: true,
			physicalPlaybacks: true,
			virtualPlaybacks: true,
		});
		await settings.expect.mask({
			programmer: true,
			physicalPlaybacks: true,
			virtualPlaybacks: true,
		});
	},
);
