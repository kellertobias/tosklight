import {
	PaneType,
	StageView,
	type PaneConfiguration,
} from "./paneTypes";

const stage: PaneConfiguration<PaneType.Stage> = {
	view: StageView.ThreeDimensional,
	followPreload: false,
};
const fixtures: PaneConfiguration<PaneType.Fixtures> = {
	showGroupShortcuts: true,
};

// @ts-expect-error Stage-only configuration must not compile for a Fixture Sheet pane.
const mismatched: PaneConfiguration<PaneType.Fixtures> = { view: StageView.TwoDimensional };

void [stage, fixtures, mismatched];
