import { GridDesktop, PaneView } from "@tosklight/ui/desktop";
import { FixtureSheetTableView } from "@tosklight/ui/tables";
import { memo } from "react";
import { fixtureTypeIconAsset } from "../../components/setup/fixtureTypeIconAssets";
import { FixtureSheetWindowView } from "../../windows/FixtureSheetWindow";
import { DEFAULT_FIXTURE_SHEET_COLUMNS } from "../../windows/FixtureSheetSettings";
import { fixtureSheetColumns } from "../../windows/fixtureSheetColumns";
import type { FixtureSheetRow } from "../../windows/fixtureSheetProjection";

const presentStep = () => ({ base: false, containedBase: false, current: false, containedCurrent: false });
const columns = fixtureSheetColumns(true, presentStep, "off").filter(column => DEFAULT_FIXTURE_SHEET_COLUMNS.includes(column.id as never));
const selected = new Set(["fixture-101", "fixture-102", "fixture-103", "fixture-104"]);
const rows: FixtureSheetRow[] = Array.from({ length: 8 }, (_, index) => ({
	id: String(101 + index), fixtureId: `fixture-${101 + index}`, name: `${index < 4 ? "Front" : "Back"} wash ${index % 4 + 1}`,
	fixtureType: "Generic moving wash", icon: fixtureTypeIconAsset("led wash moving light"), type: "Fixture",
	beam: "Open", childFixtureIds: [], color: "#ffffff", colorLabel: "Open White", dimmer: 0,
	focus: "Sharp", indented: false, limitingGroups: [], parentFixtureId: "", pan: 50,
	patch: `U1.${index * 24 + 1}`, positionLabel: "Home", preloadColor: null, preloadDimmer: null,
	preloadPan: null, preloadTilt: null, targetKind: "fixture", tilt: 50,
	sources: { beam: "default", color: "default", dimmer: "default", focus: "default", position: "default" },
}));

/** An existing window, deliberately independent of the proposed programming UI. */
const mixedRows = rows.slice(0, 3).map((row, index) => ({ ...row,
	name: ["JBLED A7", "Cameo ROOT PAR 6", "Cameo AURO SPOT"][index],
	fixtureType: ["RGB", "RGBWAUV", "7-slot color wheel"][index],
}));
const mixedSelected = new Set(mixedRows.map(row => row.fixtureId));

export const ProgrammingWorkspace = memo(function ProgrammingWorkspace({ mixedSelection = false }: { mixedSelection?: boolean }) {
	return <div className="fam-workspace" data-testid="existing-workspace">
		<GridDesktop id="fixture-abstraction" name="Fixtures">
			<PaneView showHeader={false} pane={{ id: "fixtures", title: "Fixture Sheet", type: "fixtures", x: 1, y: 1, width: 24, height: 18 }}>
				<FixtureSheetWindowView selectionCount={mixedSelection ? 3 : 4} table={<FixtureSheetTableView rows={mixedSelection ? mixedRows : rows} columns={columns}
					activeRow={0} onActivate={() => {}} onActiveRowChange={() => {}} presentStep={presentStep}
					rowHeight={43} selectedFixtureIds={mixedSelection ? mixedSelected : selected} />} />
			</PaneView>
		</GridDesktop>
	</div>;
});
