import type {
	AttributeValue,
	DmxSnapshot,
	VisualizationSnapshot,
} from "../api/types";
import type {
	VisualizationRow,
	VisualizationWidget,
	VisualizationWidgetSource,
} from "../types";

const color = /^#[0-9a-f]{6}$/i;

export function createVisualizationWidget(
	id = `visualization-widget-${Date.now()}`,
): VisualizationWidget {
	return {
		id,
		title: "Value",
		type: "number",
		source: { kind: "raw_dmx", universe: 1, address: 1 },
		operation: "multiply",
		factor: 1,
		displayScale: "percent",
		minimum: 0,
		maximum: 100,
		graph: {
			timeWindowSeconds: 30,
			yScale: "linear",
			filled: true,
			lineLowColor: "#32a9c5",
			lineHighColor: "#e7b74b",
			fillLowColor: "#102b32",
			fillHighColor: "#57451d",
			yAxisName: "Value",
		},
		bar: { orientation: "horizontal" },
		number: {
			decimalPlaces: 1,
			unit: "%",
			lowColor: "#32a9c5",
			highColor: "#e7b74b",
		},
	};
}

export function createVisualizationRow(
	id = `visualization-row-${Date.now()}`,
): VisualizationRow {
	return { id, widgets: [createVisualizationWidget(`${id}-widget-1`)] };
}

export function normalizeVisualizationRows(value: unknown): VisualizationRow[] {
	if (!Array.isArray(value)) return [];
	return value.slice(0, 12).flatMap((candidate, rowIndex) => {
		if (!candidate || typeof candidate !== "object") return [];
		const record = candidate as Record<string, unknown>;
		const widgets = Array.isArray(record.widgets)
			? record.widgets.slice(0, 8).flatMap((widget, widgetIndex) => {
					const normalized = normalizeWidget(widget, rowIndex, widgetIndex);
					return normalized ? [normalized] : [];
				})
			: [];
		return [
			{
				id: stringOr(record.id, `visualization-row-${rowIndex + 1}`),
				widgets,
			},
		];
	});
}

function normalizeWidget(
	value: unknown,
	rowIndex: number,
	widgetIndex: number,
): VisualizationWidget | null {
	if (!value || typeof value !== "object") return null;
	const record = value as Record<string, unknown>;
	const fallback = createVisualizationWidget(
		`visualization-row-${rowIndex + 1}-widget-${widgetIndex + 1}`,
	);
	const minimum = finiteOr(record.minimum, fallback.minimum);
	const maximum = Math.max(
		minimum + 0.001,
		finiteOr(record.maximum, fallback.maximum),
	);
	return {
		...fallback,
		id: stringOr(record.id, fallback.id),
		title: stringOr(record.title, fallback.title).slice(0, 80),
		type:
			record.type === "text" || record.type === "graph" || record.type === "bar"
				? record.type
				: "number",
		source: normalizeSource(record.source),
		operation: record.operation === "divide" ? "divide" : "multiply",
		factor: Math.max(0.000001, finiteOr(record.factor, 1)),
		displayScale: record.displayScale === "dmx" ? "dmx" : "percent",
		minimum,
		maximum,
		graph: normalizeGraph(record.graph, fallback.graph),
		bar: {
			orientation:
				(record.bar as Record<string, unknown> | undefined)?.orientation ===
				"vertical"
					? "vertical"
					: "horizontal",
		},
		number: normalizeNumber(record.number, fallback.number),
	};
}

function normalizeSource(value: unknown): VisualizationWidgetSource {
	const source =
		value && typeof value === "object"
			? (value as Record<string, unknown>)
			: {};
	if (source.kind === "fixture_attribute") {
		return {
			kind: "fixture_attribute",
			fixtureId: stringOr(source.fixtureId, ""),
			attribute: stringOr(source.attribute, "intensity"),
		};
	}
	return {
		kind: "raw_dmx",
		universe: boundedInteger(source.universe, 1, 63_999, 1),
		address: boundedInteger(source.address, 1, 512, 1),
	};
}

function normalizeGraph(
	value: unknown,
	fallback: NonNullable<VisualizationWidget["graph"]>,
) {
	const graph =
		value && typeof value === "object"
			? (value as Record<string, unknown>)
			: {};
	return {
		timeWindowSeconds: boundedInteger(graph.timeWindowSeconds, 1, 3_600, 30),
		yScale:
			graph.yScale === "logarithmic"
				? ("logarithmic" as const)
				: ("linear" as const),
		filled: graph.filled !== false,
		lineLowColor: colorOr(graph.lineLowColor, fallback.lineLowColor),
		lineHighColor: colorOr(graph.lineHighColor, fallback.lineHighColor),
		fillLowColor: colorOr(graph.fillLowColor, fallback.fillLowColor),
		fillHighColor: colorOr(graph.fillHighColor, fallback.fillHighColor),
		yAxisName: stringOr(graph.yAxisName, fallback.yAxisName).slice(0, 40),
	};
}

function normalizeNumber(
	value: unknown,
	fallback: NonNullable<VisualizationWidget["number"]>,
) {
	const number =
		value && typeof value === "object"
			? (value as Record<string, unknown>)
			: {};
	return {
		decimalPlaces: boundedInteger(number.decimalPlaces, 0, 6, 1),
		unit: stringOr(number.unit, fallback.unit).slice(0, 16),
		lowColor: colorOr(number.lowColor, fallback.lowColor),
		highColor: colorOr(number.highColor, fallback.highColor),
	};
}

export function resolveVisualizationValue(
	widget: VisualizationWidget,
	dmx: DmxSnapshot | null,
	visualization: VisualizationSnapshot | null,
): number | null {
	const source = widget.source;
	const raw =
		source.kind === "raw_dmx"
			? dmx?.universes.find((frame) => frame.universe === source.universe)
					?.slots[source.address - 1]
			: attributeNumber(
					visualization?.values.find(
						(value) =>
							value.fixture_id === source.fixtureId &&
							value.attribute === source.attribute,
					)?.value,
				);
	if (raw == null || !Number.isFinite(raw)) return null;
	const normalized = source.kind === "raw_dmx" ? raw / 255 : raw;
	const scaled = normalized * (widget.displayScale === "dmx" ? 255 : 100);
	const processed =
		widget.operation === "divide"
			? scaled / Math.max(widget.factor, 0.000001)
			: scaled * widget.factor;
	return Math.min(widget.maximum, Math.max(widget.minimum, processed));
}

function attributeNumber(value: AttributeValue | undefined): number | null {
	if (!value) return null;
	if (value.kind === "normalized") return value.value;
	if (value.kind === "raw_dmx" || value.kind === "raw_dmx_exact")
		return value.value / 255;
	if (value.kind === "spread") return value.value[0] ?? null;
	if (value.kind === "color_xyz") return value.value.y;
	return null;
}

export function valueRatio(widget: VisualizationWidget, value: number): number {
	return Math.min(
		1,
		Math.max(0, (value - widget.minimum) / (widget.maximum - widget.minimum)),
	);
}

export function mixedColor(low: string, high: string, ratio: number): string {
	const channel = (offset: number) =>
		Math.round(
			Number.parseInt(low.slice(offset, offset + 2), 16) * (1 - ratio) +
				Number.parseInt(high.slice(offset, offset + 2), 16) * ratio,
		)
			.toString(16)
			.padStart(2, "0");
	return `#${channel(1)}${channel(3)}${channel(5)}`;
}

function stringOr(value: unknown, fallback: string) {
	return typeof value === "string" ? value : fallback;
}

function finiteOr(value: unknown, fallback: number) {
	return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function boundedInteger(
	value: unknown,
	minimum: number,
	maximum: number,
	fallback: number,
) {
	return Math.min(
		maximum,
		Math.max(minimum, Math.trunc(finiteOr(value, fallback))),
	);
}

function colorOr(value: unknown, fallback: string) {
	return typeof value === "string" && color.test(value) ? value : fallback;
}
