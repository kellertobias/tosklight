import {
	Button,
	FormLayout,
	NumberField,
	SelectField,
	SwitchField,
	TextField,
} from "@tosklight/ui";
import { useMemo } from "react";
import { usePatchedFixturesView } from "../../features/patch/PatchState";
import { useApp } from "../../state/AppContext";
import type {
	PaneModel,
	VisualizationRow,
	VisualizationWidget,
} from "../../types";
import {
	createVisualizationRow,
	createVisualizationWidget,
} from "../../windows/visualizationPaneModel";

export function VisualizationPaneSettings({ pane }: { pane: PaneModel }) {
	const { dispatch } = useApp();
	const fixtures = usePatchedFixturesView(true);
	const rows = pane.visualizationRows ?? [];
	const persist = (next: VisualizationRow[]) =>
		dispatch({ type: "SET_PANE_VISUALIZATION_ROWS", id: pane.id, rows: next });
	return (
		<section className="visualization-pane-settings">
			<p>
				Rows stack vertically. Widgets in one row share its width side by side.
				Values sample the authoritative desk state every 250 ms while visible.
			</p>
			{rows.map((row, rowIndex) => (
				<article className="visualization-row-editor" key={row.id}>
					<header>
						<b>Row {rowIndex + 1}</b>
						<Button
							onClick={() =>
								persist(
									rows.map((candidate) =>
										candidate.id === row.id
											? {
													...candidate,
													widgets: [
														...candidate.widgets,
														createVisualizationWidget(uniqueId("widget")),
													],
												}
											: candidate,
									),
								)
							}
						>
							Add widget
						</Button>
						<Button
							className="danger"
							onClick={() =>
								persist(rows.filter((candidate) => candidate.id !== row.id))
							}
						>
							Remove row
						</Button>
					</header>
					{row.widgets.length === 0 && <p>No widgets in this row.</p>}
					{row.widgets.map((widget, widgetIndex) => (
						<WidgetEditor
							key={widget.id}
							widget={widget}
							widgetNumber={widgetIndex + 1}
							fixtures={fixtures}
							onChange={(next) =>
								persist(
									rows.map((candidate) =>
										candidate.id === row.id
											? {
													...candidate,
													widgets: candidate.widgets.map((item) =>
														item.id === widget.id ? next : item,
													),
												}
											: candidate,
									),
								)
							}
							onRemove={() =>
								persist(
									rows.map((candidate) =>
										candidate.id === row.id
											? {
													...candidate,
													widgets: candidate.widgets.filter(
														(item) => item.id !== widget.id,
													),
												}
											: candidate,
									),
								)
							}
						/>
					))}
				</article>
			))}
			<Button
				onClick={() =>
					persist([...rows, createVisualizationRow(uniqueId("row"))])
				}
			>
				Add row
			</Button>
		</section>
	);
}

function WidgetEditor({
	widget,
	widgetNumber,
	fixtures,
	onChange,
	onRemove,
}: {
	widget: VisualizationWidget;
	widgetNumber: number;
	fixtures: ReturnType<typeof usePatchedFixturesView>;
	onChange: (widget: VisualizationWidget) => void;
	onRemove: () => void;
}) {
	const selectedFixture = fixtures.find(
		(fixture) =>
			fixture.fixture_id ===
				(widget.source.kind === "fixture_attribute"
					? widget.source.fixtureId
					: "") ||
			fixture.logical_heads.some(
				(head) =>
					head.fixture_id ===
					(widget.source.kind === "fixture_attribute"
						? widget.source.fixtureId
						: ""),
			),
	);
	const fixtureOptions = useMemo(
		() =>
			fixtures.flatMap((fixture) => [
				{
					value: fixture.fixture_id,
					label: `${fixture.fixture_number} · ${fixture.name}`,
				},
				...fixture.logical_heads.map((head) => ({
					value: head.fixture_id,
					label: `${fixture.fixture_number}.${head.head_index} · Logical head ${head.head_index}`,
				})),
			]),
		[fixtures],
	);
	const attributes = useMemo(
		() =>
			[
				...new Set(
					selectedFixture?.definition.heads.flatMap((head) =>
						head.parameters.map((parameter) => parameter.attribute),
					) ?? [],
				),
			].sort(),
		[selectedFixture],
	);
	const sourceKind = widget.source.kind;
	return (
		<section className="visualization-widget-editor">
			<header>
				<b>Widget {widgetNumber}</b>
				<Button className="danger" onClick={onRemove}>
					Remove widget
				</Button>
			</header>
			<FormLayout labelPlacement="side">
				<TextField
					label="Title"
					value={widget.title}
					onChange={(event) =>
						onChange({ ...widget, title: event.target.value })
					}
				/>
				<SelectField
					label="Widget type"
					ariaLabel="Widget type"
					value={widget.type}
					onChange={(type) =>
						onChange({ ...widget, type: type as VisualizationWidget["type"] })
					}
					options={[
						{ value: "text", label: "Text / message" },
						{ value: "graph", label: "Time graph" },
						{ value: "bar", label: "Bar / progress" },
						{ value: "number", label: "Numeric text" },
					]}
				/>
				<SelectField
					label="Value source"
					ariaLabel="Value source"
					value={sourceKind}
					onChange={(kind) =>
						onChange({
							...widget,
							source:
								kind === "fixture_attribute"
									? {
											kind: "fixture_attribute",
											fixtureId: fixtureOptions[0]?.value ?? "",
											attribute: "intensity",
										}
									: { kind: "raw_dmx", universe: 1, address: 1 },
						})
					}
					options={[
						{ value: "raw_dmx", label: "Raw DMX" },
						{ value: "fixture_attribute", label: "Fixture attribute" },
					]}
				/>
				{widget.source.kind === "raw_dmx" ? (
					<>
						<NumberField
							label="Universe"
							min="1"
							max="63999"
							value={widget.source.universe}
							onChange={(event) =>
								onChange({
									...widget,
									source: {
										kind: "raw_dmx",
										universe: Number(event.target.value),
										address:
											widget.source.kind === "raw_dmx"
												? widget.source.address
												: 1,
									},
								})
							}
						/>
						<NumberField
							label="Address"
							min="1"
							max="512"
							value={widget.source.address}
							onChange={(event) =>
								onChange({
									...widget,
									source: {
										kind: "raw_dmx",
										universe:
											widget.source.kind === "raw_dmx"
												? widget.source.universe
												: 1,
										address: Number(event.target.value),
									},
								})
							}
						/>
					</>
				) : (
					<>
						<SelectField
							ariaLabel="Fixture"
							label="Fixture"
							value={widget.source.fixtureId}
							onChange={(fixtureId) =>
								onChange({
									...widget,
									source: {
										kind: "fixture_attribute",
										fixtureId,
										attribute:
											widget.source.kind === "fixture_attribute"
												? widget.source.attribute
												: "intensity",
									},
								})
							}
							options={fixtureOptions}
						/>
						<SelectField
							ariaLabel="Attribute"
							label="Attribute"
							value={widget.source.attribute}
							onChange={(attribute) =>
								onChange({
									...widget,
									source: {
										kind: "fixture_attribute",
										fixtureId:
											widget.source.kind === "fixture_attribute"
												? widget.source.fixtureId
												: "",
										attribute,
									},
								})
							}
							options={attributes.map((attribute) => ({
								value: attribute,
								label: attribute,
							}))}
						/>
					</>
				)}
				<SelectField
					ariaLabel="Processing"
					label="Processing"
					value={widget.operation}
					onChange={(operation) =>
						onChange({
							...widget,
							operation: operation as "multiply" | "divide",
						})
					}
					options={[
						{ value: "multiply", label: "Multiply" },
						{ value: "divide", label: "Divide" },
					]}
				/>
				<NumberField
					label="Factor"
					min="0.000001"
					step="0.01"
					value={widget.factor}
					onChange={(event) =>
						onChange({ ...widget, factor: Number(event.target.value) })
					}
				/>
				<SelectField
					ariaLabel="Display scale"
					label="Display scale"
					value={widget.displayScale}
					onChange={(displayScale) =>
						onChange({
							...widget,
							displayScale: displayScale as "percent" | "dmx",
						})
					}
					options={[
						{ value: "percent", label: "0–100 %" },
						{ value: "dmx", label: "0–255" },
					]}
				/>
				<NumberField
					label="Minimum"
					value={widget.minimum}
					onChange={(event) =>
						onChange({ ...widget, minimum: Number(event.target.value) })
					}
				/>
				<NumberField
					label="Maximum"
					value={widget.maximum}
					onChange={(event) =>
						onChange({ ...widget, maximum: Number(event.target.value) })
					}
				/>
				{widget.type === "graph" && (
					<GraphSettings widget={widget} onChange={onChange} />
				)}
				{widget.type === "bar" && (
					<SelectField
						ariaLabel="Orientation"
						label="Orientation"
						value={widget.bar?.orientation ?? "horizontal"}
						onChange={(orientation) =>
							onChange({
								...widget,
								bar: { orientation: orientation as "horizontal" | "vertical" },
							})
						}
						options={[
							{ value: "horizontal", label: "Horizontal" },
							{ value: "vertical", label: "Vertical" },
						]}
					/>
				)}
				{widget.type === "number" && (
					<NumberSettings widget={widget} onChange={onChange} />
				)}
			</FormLayout>
		</section>
	);
}

function GraphSettings({
	widget,
	onChange,
}: {
	widget: VisualizationWidget;
	onChange: (widget: VisualizationWidget) => void;
}) {
	const graph = widget.graph;
	const update = (changes: Partial<typeof graph>) =>
		onChange({ ...widget, graph: { ...graph, ...changes } });
	return (
		<>
			<NumberField
				label="Time window (seconds)"
				min="1"
				max="3600"
				value={graph.timeWindowSeconds}
				onChange={(event) =>
					update({ timeWindowSeconds: Number(event.target.value) })
				}
			/>
			<SelectField
				ariaLabel="Y scale"
				label="Y scale"
				value={graph.yScale}
				onChange={(yScale) =>
					update({ yScale: yScale as "linear" | "logarithmic" })
				}
				options={[
					{ value: "linear", label: "Linear" },
					{ value: "logarithmic", label: "Logarithmic" },
				]}
			/>
			<SwitchField
				label="Area fill"
				offLabel="Line only"
				onLabel="Filled"
				checked={graph.filled}
				onChange={(event) => update({ filled: event.target.checked })}
			/>
			<TextField
				label="Y-axis name"
				value={graph.yAxisName}
				onChange={(event) => update({ yAxisName: event.target.value })}
			/>
			<TextField
				label="Line low colour"
				value={graph.lineLowColor}
				onChange={(event) => update({ lineLowColor: event.target.value })}
			/>
			<TextField
				label="Line high colour"
				value={graph.lineHighColor}
				onChange={(event) => update({ lineHighColor: event.target.value })}
			/>
			<TextField
				label="Fill low colour"
				value={graph.fillLowColor}
				onChange={(event) => update({ fillLowColor: event.target.value })}
			/>
			<TextField
				label="Fill high colour"
				value={graph.fillHighColor}
				onChange={(event) => update({ fillHighColor: event.target.value })}
			/>
		</>
	);
}

function NumberSettings({
	widget,
	onChange,
}: {
	widget: VisualizationWidget;
	onChange: (widget: VisualizationWidget) => void;
}) {
	const number = widget.number;
	const update = (changes: Partial<typeof number>) =>
		onChange({ ...widget, number: { ...number, ...changes } });
	return (
		<>
			<NumberField
				label="Decimal places"
				min="0"
				max="6"
				value={number.decimalPlaces}
				onChange={(event) =>
					update({ decimalPlaces: Number(event.target.value) })
				}
			/>
			<TextField
				label="Unit suffix"
				value={number.unit}
				onChange={(event) => update({ unit: event.target.value })}
			/>
			<TextField
				label="Low-value colour"
				value={number.lowColor}
				onChange={(event) => update({ lowColor: event.target.value })}
			/>
			<TextField
				label="High-value colour"
				value={number.highColor}
				onChange={(event) => update({ highColor: event.target.value })}
			/>
		</>
	);
}

function uniqueId(part: string) {
	return `visualization-${part}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}
