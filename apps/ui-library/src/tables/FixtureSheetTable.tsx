import { useCallback } from "react";
import {
	DataTable,
	type DataTableColumn,
	type RowActivationModifiers,
	WindowScrollArea,
} from "../window-kit";

export interface FixtureSheetRowView {
	id: string | number;
	fixtureId: string;
	targetKind: "fixture" | "master" | "head";
	parentFixtureId: string;
	childFixtureIds: string[];
	indented: boolean;
}

export interface FixtureStepPresentation {
	base: boolean;
	containedBase: boolean;
	containedCurrent: boolean;
	current: boolean;
}

export type FixtureStepPresenter<Row extends FixtureSheetRowView> = (
	fixture: Row,
) => FixtureStepPresentation;

function fixtureRowClass<Row extends FixtureSheetRowView>(
	fixture: Row,
	present: FixtureStepPresenter<Row>,
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

function fixtureRowData<Row extends FixtureSheetRowView>(
	fixture: Row,
	present: FixtureStepPresenter<Row>,
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

export function FixtureSheetTableView<Row extends FixtureSheetRowView>({
	activeRow,
	columns,
	onActivate,
	onActiveRowChange,
	onVisibleFixtureIdsChange,
	presentStep,
	rows,
	rowHeight = 43,
	selectedFixtureIds,
}: {
	activeRow: number;
	columns: DataTableColumn<Row>[];
	onActivate?: (
		fixtureId: string,
		modifiers: RowActivationModifiers,
	) => void;
	onActiveRowChange?: (index: number) => void;
	onVisibleFixtureIdsChange?: (fixtureIds: readonly string[]) => void;
	presentStep: FixtureStepPresenter<Row>;
	rows: Row[];
	rowHeight?: number;
	selectedFixtureIds: ReadonlySet<string>;
}) {
	const visibleRowsChanged = useCallback(
		(visibleRows: readonly Row[]) =>
			onVisibleFixtureIdsChange?.(
				visibleRows
					.map((fixture) => fixture.fixtureId)
					.filter((fixtureId) => fixtureId.length > 0),
			),
		[onVisibleFixtureIdsChange],
	);
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
				onActivate={
					onActivate
						? (fixture, _index, modifiers) =>
								fixture.fixtureId && onActivate(fixture.fixtureId, modifiers)
						: undefined
				}
				onVisibleRowsChange={visibleRowsChanged}
				virtualize
				rowHeight={rowHeight}
			/>
		</WindowScrollArea>
	);
}
