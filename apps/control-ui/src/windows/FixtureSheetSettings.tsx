import { useServer } from "../api/ServerContext";
import { Select, SwitchField } from "../components/common";
import { WindowSettings } from "../components/window-kit";
import { useApp } from "../state/AppContext";
import type {
	FixtureSheetColumn,
	FixtureSheetIncludedHeads,
	FixtureSheetOrder,
} from "../types";

const columnOrder: FixtureSheetColumn[] = [
	"id",
	"icon",
	"name",
	"patch",
	"dimmer",
	"color",
	"position",
	"beam",
	"focus",
];

export const DEFAULT_FIXTURE_SHEET_COLUMNS = columnOrder.filter(
	(column) => column !== "patch",
);

const columnLabels: Record<FixtureSheetColumn, string> = {
	id: "Fixture ID",
	icon: "Icon",
	name: "Name",
	patch: "Patch address",
	dimmer: "Dimmer",
	color: "Color",
	position: "Position",
	beam: "Beam",
	focus: "Focus",
};

function FixtureSheetViewSettings({
	activeOnly,
	cueListId,
	fixtureOrder,
	includedHeads,
}: {
	activeOnly: boolean;
	cueListId: string;
	fixtureOrder: FixtureSheetOrder;
	includedHeads: FixtureSheetIncludedHeads;
}) {
	const server = useServer();
	const { dispatch } = useApp();
	return (
		<div className="fixture-sheet-settings-sections">
			<section>
				<h3>Fixture heads</h3>
				<Select
					aria-label="Included heads"
					value={includedHeads}
					onChange={(event) =>
						dispatch({
							type: "SET_FIXTURE_SHEET_OPTIONS",
							includedHeads: event.target.value as FixtureSheetIncludedHeads,
						})
					}
				>
					<option value="all">All</option>
					<option value="no-sub-heads">No sub heads</option>
					<option value="no-master-heads">No master heads</option>
				</Select>
			</section>
			<section>
				<h3>Ordering</h3>
				<div className="pane-option-toggle">
					Order fixtures{" "}
					<Select
						aria-label="Fixture sheet ordering"
						value={fixtureOrder}
						onChange={(event) =>
							dispatch({
								type: "SET_FIXTURE_SHEET_OPTIONS",
								order: event.target.value as FixtureSheetOrder,
							})
						}
					>
						<option value="fixture-id">Fixture ID</option>
						<option value="active">Active fixtures first</option>
					</Select>
				</div>
			</section>
			<section>
				<h3>Filters</h3>
				<SwitchField
					label="Show active fixtures only"
					checked={activeOnly}
					onChange={(event) =>
						dispatch({
							type: "SET_FIXTURE_SHEET_OPTIONS",
							activeOnly: event.target.checked,
						})
					}
				/>
				<div className="pane-option-toggle">
					Cuelist{" "}
					<Select
						aria-label="Fixture sheet Cuelist filter"
						value={cueListId}
						onChange={(event) =>
							dispatch({
								type: "SET_FIXTURE_SHEET_OPTIONS",
								cueListId: event.target.value,
							})
						}
					>
						<option value="">All fixtures</option>
						{(server.playbacks?.cue_lists ?? []).map((cueList) => (
							<option key={cueList.id} value={cueList.id}>
								{cueList.name}
							</option>
						))}
					</Select>
				</div>
			</section>
		</div>
	);
}

function FixtureSheetColumnSettings() {
	const { state, dispatch } = useApp();
	const toggleColumn = (column: FixtureSheetColumn, checked: boolean) =>
		dispatch({
			type: "SET_FIXTURE_SHEET_OPTIONS",
			columns: checked
				? [...state.fixtureSheetColumns, column]
				: state.fixtureSheetColumns.filter((candidate) => candidate !== column),
		});
	return (
		<div className="fixture-sheet-settings-sections">
			<section>
				<h3>Visible columns</h3>
				<div className="fixture-sheet-column-options">
					{columnOrder.map((column) => (
						<SwitchField
							key={column}
							label={columnLabels[column]}
							checked={state.fixtureSheetColumns.includes(column)}
							disabled={
								state.fixtureSheetColumns.length === 1 &&
								state.fixtureSheetColumns.includes(column)
							}
							onChange={(event) => toggleColumn(column, event.target.checked)}
						/>
					))}
				</div>
			</section>
			<section>
				<h3>Name details</h3>
				<SwitchField
					label="Show fixture type"
					checked={state.fixtureSheetShowType}
					disabled={!state.fixtureSheetColumns.includes("name")}
					onChange={(event) =>
						dispatch({
							type: "SET_FIXTURE_SHEET_OPTIONS",
							showType: event.target.checked,
						})
					}
				/>
			</section>
		</div>
	);
}

function FixtureSheetGroupSettings({ visible }: { visible: boolean }) {
	const { dispatch } = useApp();
	return (
		<SwitchField
			label="Enable group shortcuts"
			checked={visible}
			onChange={(event) =>
				dispatch({
					type: "SET_BUILTIN_GROUPS_VISIBLE",
					window: "fixtures",
					value: event.target.checked,
				})
			}
		/>
	);
}

export function FixtureSheetSettings({
	activeOnly,
	anchor,
	cueListId,
	fixtureOrder,
	groupsVisible,
	includedHeads,
	onClose,
}: {
	activeOnly: boolean;
	anchor: DOMRect;
	cueListId: string;
	fixtureOrder: FixtureSheetOrder;
	groupsVisible: boolean;
	includedHeads: FixtureSheetIncludedHeads;
	onClose: () => void;
}) {
	return (
		<WindowSettings
			modal={false}
			anchor={anchor}
			title="Fixture Sheet"
			onClose={onClose}
			tabs={[
				{
					id: "view",
					label: "View",
					content: (
						<FixtureSheetViewSettings
							activeOnly={activeOnly}
							cueListId={cueListId}
							fixtureOrder={fixtureOrder}
							includedHeads={includedHeads}
						/>
					),
				},
				{
					id: "columns",
					label: "Columns",
					content: <FixtureSheetColumnSettings />,
				},
				{
					id: "groups",
					label: "Groups",
					content: <FixtureSheetGroupSettings visible={groupsVisible} />,
				},
			]}
		/>
	);
}
