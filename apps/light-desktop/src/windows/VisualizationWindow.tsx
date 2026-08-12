import { WindowHeader, WindowScrollArea } from "@tosklight/ui/window-kit";
import { type CSSProperties, useEffect, useMemo, useState } from "react";
import type { DmxSnapshot } from "../api/types";
import { useDmxDiagnostics } from "../features/dmxDiagnostics/DmxDiagnosticsContext";
import { useConnectionStatus } from "../features/shellStatus/ShellStatusState";
import { useVisualizationRuntimeSnapshot } from "../features/visualizationRuntime/VisualizationRuntimeView";
import { usePollingResource } from "../hooks/usePollingResource";
import type { VisualizationWidget } from "../types";
import {
	mixedColor,
	resolveVisualizationValue,
	valueRatio,
} from "./visualizationPaneModel";
import type { WindowProps } from "./windowTypes";

const SAMPLE_INTERVAL_MILLIS = 250;

export function VisualizationWindow({
	active = true,
	compact,
	visualizationRows = [],
}: WindowProps) {
	const dmx = useDmxDiagnostics();
	const connection = useConnectionStatus();
	const [dmxSnapshot, setDmxSnapshot] = useState<DmxSnapshot | null>(null);
	usePollingResource({
		enabled: active && connection === "connected" && Boolean(dmx),
		intervalMillis: SAMPLE_INTERVAL_MILLIS,
		load:
			dmx?.readDmx ??
			(async () => ({ revision: 0, universes: [], overrides: [] })),
		onValue: setDmxSnapshot,
	});
	const visualization = useVisualizationRuntimeSnapshot({
		enabled: active,
		intervalMillis: SAMPLE_INTERVAL_MILLIS,
		consumerId: "visualization-pane",
	});
	const content = visualizationRows.length ? (
		<div className="visualization-pane-rows">
			{visualizationRows.map((row) => (
				<div className="visualization-pane-row" key={row.id}>
					{row.widgets.map((widget) => (
						<VisualizationWidgetView
							key={widget.id}
							widget={widget}
							value={resolveVisualizationValue(
								widget,
								dmxSnapshot,
								visualization,
							)}
							revision={`${dmxSnapshot?.revision ?? 0}:${visualization?.revision ?? 0}`}
						/>
					))}
				</div>
			))}
		</div>
	) : (
		<p className="empty-window-message">
			Open Pane Settings to add a row and its first value widget.
		</p>
	);
	return (
		<section className="visualization-window">
			{!compact && <WindowHeader title="Visualization" />}
			<WindowScrollArea>{content}</WindowScrollArea>
		</section>
	);
}

export function VisualizationWidgetView({
	widget,
	value,
	revision,
}: {
	widget: VisualizationWidget;
	value: number | null;
	revision: string;
}) {
	const ratio = value == null ? 0 : valueRatio(widget, value);
	const numberColor = mixedColor(
		widget.number?.lowColor ?? "#32a9c5",
		widget.number?.highColor ?? "#e7b74b",
		ratio,
	);
	return (
		<article
			className={`visualization-widget visualization-widget-${widget.type}`}
		>
			<header>{widget.title || "Value"}</header>
			{value == null ? (
				<p className="visualization-value-unavailable">Value unavailable</p>
			) : widget.type === "graph" ? (
				<TimeGraph widget={widget} value={value} revision={revision} />
			) : widget.type === "bar" ? (
				<div
					className={`visualization-bar visualization-bar-${widget.bar?.orientation ?? "horizontal"}`}
				>
					<meter
						className="visualization-bar-meter"
						aria-label={widget.title}
						min={widget.minimum}
						max={widget.maximum}
						value={value}
					/>
					<i
						style={
							{ "--visualization-level": `${ratio * 100}%` } as CSSProperties
						}
					/>
					<span>{formatValue(widget, value)}</span>
				</div>
			) : widget.type === "text" ? (
				<p className="visualization-text-value">{formatValue(widget, value)}</p>
			) : (
				<strong
					className="visualization-number-value"
					style={{ color: numberColor }}
				>
					{formatValue(widget, value)}
				</strong>
			)}
		</article>
	);
}

function TimeGraph({
	widget,
	value,
	revision,
}: {
	widget: VisualizationWidget;
	value: number;
	revision: string;
}) {
	const windowMillis = (widget.graph?.timeWindowSeconds ?? 30) * 1_000;
	const [samples, setSamples] = useState<Array<{ at: number; value: number }>>(
		[],
	);
	useEffect(() => {
		const at = Date.now();
		setSamples((current) => [
			...current.filter((sample) => sample.at >= at - windowMillis),
			{ at, value },
		]);
	}, [revision, value, windowMillis]);
	const path = useMemo(
		() => graphPath(samples, widget, windowMillis),
		[samples, widget, windowMillis],
	);
	const ratio = valueRatio(widget, value);
	const line = mixedColor(
		widget.graph?.lineLowColor ?? "#32a9c5",
		widget.graph?.lineHighColor ?? "#e7b74b",
		ratio,
	);
	const fill = mixedColor(
		widget.graph?.fillLowColor ?? "#102b32",
		widget.graph?.fillHighColor ?? "#57451d",
		ratio,
	);
	return (
		<div className="visualization-graph">
			<span>{widget.graph?.yAxisName || "Value"}</span>
			<svg
				viewBox="0 0 100 100"
				preserveAspectRatio="none"
				aria-label={`${widget.title} over time`}
			>
				{widget.graph?.filled !== false && path && (
					<path d={`${path} L 100 100 L 0 100 Z`} fill={fill} />
				)}
				{path && (
					<path
						d={path}
						fill="none"
						stroke={line}
						strokeWidth="2"
						vectorEffect="non-scaling-stroke"
					/>
				)}
			</svg>
			<small>
				{widget.graph?.timeWindowSeconds ?? 30}s · {formatValue(widget, value)}
			</small>
		</div>
	);
}

function graphPath(
	samples: Array<{ at: number; value: number }>,
	widget: VisualizationWidget,
	windowMillis: number,
) {
	if (!samples.length) return "";
	const newest = samples.at(-1)?.at ?? Date.now();
	return samples
		.map((sample, index) => {
			const x = Math.max(0, 100 - ((newest - sample.at) / windowMillis) * 100);
			const linear = valueRatio(widget, sample.value);
			const ratio =
				widget.graph?.yScale === "logarithmic"
					? Math.log10(1 + linear * 9)
					: linear;
			return `${index ? "L" : "M"} ${x.toFixed(2)} ${(100 - ratio * 100).toFixed(2)}`;
		})
		.join(" ");
}

function formatValue(widget: VisualizationWidget, value: number) {
	const decimals =
		widget.type === "number" ? (widget.number?.decimalPlaces ?? 1) : 1;
	const unit =
		widget.type === "number"
			? (widget.number?.unit ?? "")
			: widget.displayScale === "percent"
				? "%"
				: "";
	return `${value.toFixed(decimals)}${unit}`;
}
