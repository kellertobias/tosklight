import {
	DataTable,
	type DataTableColumn,
	WindowScrollArea,
} from "../components/window-kit";
import type { FixtureSheetRow } from "./fixtureSheetProjection";
import type { FixtureStepPresenter } from "./fixtureSheetStep";

function fixtureRowClass(
	fixture: FixtureSheetRow,
	present: FixtureStepPresenter,
) {
	const step = present(fixture);
	return [
		`fixture-${fixture.targetKind}-row`,
		fixture.indented ? "fixture-head-indented-row" : "",
		step.base ? "fixture-step-base" : "",
		step.current ? "fixture-step-current" : "",
		step.containedBase ? "fixture-step-contained-base" : "",
		step.containedCurrent ? "fixture-step-contained-current" : "",
	]
		.filter(Boolean)
		.join(" ");
}

function fixtureRowData(
	fixture: FixtureSheetRow,
	present: FixtureStepPresenter,
) {
	const step = present(fixture);
	return {
		"data-fixture-id": fixture.fixtureId || undefined,
		"data-fixture-kind": fixture.targetKind,
		"data-parent-fixture-id": fixture.parentFixtureId || undefined,
		"data-step-selection": step.current
			? "active"
			: step.base
				? "base"
				: undefined,
		"data-step-contained": step.containedCurrent
			? "active"
			: step.containedBase
				? "base"
				: undefined,
	};
}

export function FixtureSheetTable({
	activeRow,
	columns,
	onActivate,
	onActiveRowChange,
	presentStep,
	rows,
	selectedFixtureIds,
}: {
	activeRow: number;
	columns: DataTableColumn<FixtureSheetRow>[];
	onActivate: (fixtureId: string) => void;
	onActiveRowChange: (index: number) => void;
	presentStep: FixtureStepPresenter;
	rows: FixtureSheetRow[];
	selectedFixtureIds: ReadonlySet<string>;
}) {
	return (
		<WindowScrollArea className="fixture-table">
			<DataTable
				columns={columns}
				rows={rows}
				rowKey={(fixture) => fixture.fixtureId || String(fixture.id)}
				selected={(fixture) =>
					Boolean(
						fixture.fixtureId && selectedFixtureIds.has(fixture.fixtureId),
					)
				}
				rowClassName={(fixture) => fixtureRowClass(fixture, presentStep)}
				rowDataAttributes={(fixture) => fixtureRowData(fixture, presentStep)}
				activeIndex={activeRow}
				onActiveIndexChange={onActiveRowChange}
				onActivate={(fixture) =>
					fixture.fixtureId && onActivate(fixture.fixtureId)
				}
			/>
		</WindowScrollArea>
	);
}
