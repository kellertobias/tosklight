// @bench-semantic-world

import { scenario } from "./bench/core/scenario";
import { Show } from "./bench/show/showScenario";
import { PaneType } from "./bench/window-system/paneTypes";

scenario(
	"FILE-002",
	"File Manager provides the three-column browsing workflow and text editor",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		const desktop = t.desktop.configure("File Manager editing");
		desktop.addPane(PaneType.FileManager, {
			slug: "files",
			column: 1,
			row: 1,
			width: 24,
			height: 18,
		});
		await desktop.apply();
		await t.files.expectManagerEditsText();
	},
);

scenario(
	"TEXT-001",
	"dedicated Text Editor persists its file association and reports dirty state",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		const desktop = t.desktop.configure("Text Editor saving");
		desktop.addPane(PaneType.TextEditor, {
			slug: "editor",
			column: 1,
			row: 1,
			width: 24,
			height: 18,
		});
		await desktop.apply();
		await t.files.expectEditorDirtySave();
	},
);

scenario(
	"TEXT-015",
	"Text Editors synchronize, surface conflicts, recover files, and retain configured views",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		const desktop = t.desktop.configure("Text Editor collaboration");
		desktop.addPane(PaneType.TextEditor, {
			slug: "first-editor",
			column: 1,
			row: 1,
			width: 12,
			height: 18,
		});
		desktop.addPane(PaneType.TextEditor, {
			slug: "second-editor",
			column: 13,
			row: 1,
			width: 12,
			height: 18,
		});
		await desktop.apply();
		await t.files.expectTwoEditorLifecycle();
		await t.files.expectEditorModes();
	},
);

scenario(
	"FILE-016",
	"File Manager operations, launchers, hosted picking, and system fallback stay confined",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		const desktop = t.desktop.configure("File Manager workflows");
		desktop.addPane(PaneType.FileManager, {
			slug: "files",
			column: 1,
			row: 1,
			width: 24,
			height: 18,
		});
		await desktop.apply();
		await t.files.expectManagerOperations();
		await t.files.expectSetupLaunchers();
		await t.files.expectHostedPicker();
		await t.files.expectSystemPickerFallback();
	},
);
