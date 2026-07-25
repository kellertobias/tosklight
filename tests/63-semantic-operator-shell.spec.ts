// @bench-semantic-world

import { scenario } from "./bench/core/scenario";
import { Show } from "./bench/show/showScenario";

scenario(
	"MANUAL-019",
	"saved workspaces are Desktops while physical control surfaces remain desks",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await t.operatorShell.expectDesktopAndDeskTerminology();
	},
);

scenario(
	"MANUAL-019",
	"fixture browsers share title-bar search and readable name/detail alignment",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await t.operatorShell.expectFixtureBrowserAlignment();
	},
);

scenario(
	"MANUAL-019",
	"every operator file field uses its confined extension contract",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await t.operatorShell.expectOperatorFilePickerContracts();
	},
);

scenario(
	"MANUAL-019",
	"Development stays out of operator panes and remains available through Desk Status",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await t.operatorShell.expectDevelopmentDiagnosticsBoundary();
	},
);
