// @bench-semantic-world

import { scenario } from "./bench/core/scenario";
import { Show } from "./bench/show/showScenario";
import { PaneType, StageView } from "./bench/window-system/paneTypes";

scenario(
	"STAGE-001",
	"Live and Follow Preload Stage panes retain independent operator configuration",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();

		const desktop = t.desktop.configure("Stage lane proof");
		const live = desktop.addPane(
			PaneType.Stage,
			{
				slug: "live-stage",
				column: 1,
				row: 1,
				width: 12,
				height: 18,
			},
			{ view: StageView.TwoDimensional, followPreload: false },
		);
		const preload = desktop.addPane(
			PaneType.Stage,
			{
				slug: "preload-stage",
				column: 13,
				row: 1,
				width: 12,
				height: 18,
			},
			{ view: StageView.TwoDimensional, followPreload: true },
		);
		await desktop.apply();

		await desktop.expectStagePane(live, {
			lane: "live",
			view: StageView.TwoDimensional,
			followPreload: false,
			nativeRendererAvailable: false,
		});
		await desktop.expectStagePane(preload, {
			lane: "preload",
			view: StageView.TwoDimensional,
			followPreload: true,
			nativeRendererAvailable: false,
		});
	},
);
