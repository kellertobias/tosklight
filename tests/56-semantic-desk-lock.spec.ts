// @bench-semantic-world

import { scenario } from "../apps/control-ui/e2e/bench/core/scenario";
import { Show } from "../apps/control-ui/e2e/bench/show/showScenario";

scenario(
	"LOCK-001",
	"Desk Lock protects every screen and keeps its configuration in Screens",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();

		await t.deskLock.expectPinProtectionAcrossScreens();
		await t.deskLock.expectButtonFallback();
		await t.deskLock.expectScreenSettingsOwnership();
	},
);
