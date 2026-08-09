// @bench-semantic-world

import { scenario } from "./bench/core/scenario";
import { StoreMode } from "./bench/groups-presets/groupScenario";
import { Show } from "./bench/show/showScenario";

scenario(
	"UPDATE-001",
	"Update Add New appends ordered Group membership",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.selection.fixtures.via.api.item(5);
		await t.group.via.keypad.store(3, { mode: StoreMode.Merge });
		await t.group.expect(3).fixtures(1, 2, 3, 4, 5);
	},
);

scenario(
	"HIGHLIGHT-001",
	"HIGH follows the actual stepped selection",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.selection.fixtures.via.api.items(101, 102, 103);
		await t.highlight.on();
		await t.keypad.press(["NEXT"]);
		await t.highlight.expectSelection(101);
		await t.highlight.waitForControlDebounce();
		await t.keypad.press(["NEXT"]);
		await t.highlight.expectSelection(102);
		await t.highlight.off();
	},
);

scenario(
	"HIGHLIGHT-002",
	"HIGH follows external selection and survives an empty selection",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.selection.fixtures.via.api.items(101, 102, 103, 104);
		await t.highlight.on();
		await t.selection.clear();
		await t.selection.fixtures.via.api.items(103, 104);
		await t.highlight.expectSelection(103, 104);
		await t.highlight.off();
	},
);

scenario(
	"HIGHLIGHT-003",
	"PREV NEXT ALL mutate the real selection",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.selection.fixtures.via.api.items(101, 102, 103, 104);
		await t.highlight.on();
		await t.keypad.press(["NEXT"]);
		await t.highlight.expectSelection(101);
		await t.highlight.waitForControlDebounce();
		await t.keypad.press(["NEXT"]);
		await t.highlight.expectSelection(102);
		await t.highlight.waitForControlDebounce();
		await t.keypad.press(["ALL"]);
		await t.highlight.expectSelection(101, 102, 103, 104);
		await t.highlight.waitForControlDebounce();
		await t.keypad.press(["PREV"]);
		await t.highlight.expectSelection(104);
		await t.highlight.off();
	},
);

scenario(
	"HIGHLIGHT-005",
	"Highlight errors remain reachable and dismissible",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.systemIntegration.expectHighlightErrorOverlay();
	},
);

scenario(
	"FIXTURE-001",
	"a complete fixture profile is created through the desk library",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await t.systemIntegration.expectFixtureProfileCreation(
			"Semantic Acceptance",
			`Revisioned profile ${crypto.randomUUID().slice(0, 8)}`,
		);
	},
);

scenario(
	"MATTER-001",
	"the desk-persistent Matter bridge toggles from Network and Inputs",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await t.systemIntegration.expectMatterBridgeToggle();
	},
);

scenario(
	"SOUND-001",
	"a desk-local audio observation drives Speed Group A",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.systemIntegration.expectSoundToLightConfiguration();
	},
);
