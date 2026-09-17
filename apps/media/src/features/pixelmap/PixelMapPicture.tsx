// The output picture with the display regions and pixel zones drawn over it.
//
// The live frame sits underneath so an operator places a zone against what is actually on the
// wall. The open tab's shapes are the ones that answer a press; the other tab's are drawn faintly
// for reference, so a region can be placed around the zones it has to leave alone.

import { useEffect, useState } from "react";
import { api } from "../../shared/api/client";
import type {
	OutputConfigurationView,
	PixelMapView,
} from "../../shared/api/generated/media-wire";
import type { PixelMapTab } from "./PixelMapPage";

const PREVIEW_REFRESH_MS = 1_000;

/** A percentage for CSS, from a canvas fraction. */
function percent(value: number): string {
	return `${Math.max(0, Math.min(1, value)) * 100}%`;
}

function box(shape: {
	start: { x: number; y: number };
	end: { x: number; y: number };
}) {
	return {
		left: percent(Math.min(shape.start.x, shape.end.x)),
		top: percent(Math.min(shape.start.y, shape.end.y)),
		width: percent(Math.abs(shape.end.x - shape.start.x)),
		height: percent(Math.abs(shape.end.y - shape.start.y)),
	};
}

function area(shape: Parameters<typeof box>[0]): number {
	return (
		Math.abs(shape.end.x - shape.start.x) *
		Math.abs(shape.end.y - shape.start.y)
	);
}

/** Larger shapes first, so a smaller one drawn inside them can always be pressed. */
function largestFirst<T extends Parameters<typeof box>[0]>(shapes: T[]): T[] {
	return [...shapes].sort((left, right) => area(right) - area(left));
}

function useLivePreview(output: OutputConfigurationView): string {
	const [revision, setRevision] = useState(0);
	useEffect(() => {
		const timer = window.setInterval(
			() => setRevision((current) => current + 1),
			PREVIEW_REFRESH_MS,
		);
		return () => window.clearInterval(timer);
	}, []);
	return api.outputPreviewUrl(output.id, revision, {
		width: Math.max(output.width, 1),
		height: Math.max(output.height, 1),
	});
}

export function PixelMapPicture({
	output,
	map,
	tab,
	selectedRegionId,
	selectedZoneId,
	onSelectRegion,
	onSelectZone,
}: {
	output: OutputConfigurationView;
	map: PixelMapView;
	tab: PixelMapTab;
	selectedRegionId: string | null;
	selectedZoneId: string | null;
	onSelectRegion: (id: string) => void;
	onSelectZone: (id: string) => void;
}) {
	const src = useLivePreview(output);
	const [pictureFailed, setPictureFailed] = useState(false);
	const { width, height } = output;
	return (
		<figure className="media-pixel-picture">
			{/* biome-ignore lint/a11y/useSemanticElements: a fieldset cannot hold the picture's aspect ratio reliably. */}
			<div
				className="media-pixel-canvas"
				data-tab={tab}
				style={{
					aspectRatio: `${Math.max(width, 1)} / ${Math.max(height, 1)}`,
				}}
				role="group"
				aria-label={`Output picture, ${width} by ${height}`}
			>
				{!pictureFailed && (
					<img
						className="media-pixel-canvas-frame"
						src={src}
						alt=""
						draggable={false}
						onError={() => setPictureFailed(true)}
						onLoad={() => setPictureFailed(false)}
					/>
				)}
				{largestFirst(map.regions).map((region) => (
					<button
						key={region.id}
						type="button"
						className="media-pixel-region"
						aria-label={`${region.name} display region`}
						aria-pressed={region.id === selectedRegionId}
						data-enabled={region.enabled ? "true" : "false"}
						disabled={tab !== "regions"}
						tabIndex={tab === "regions" ? 0 : -1}
						onClick={() => onSelectRegion(region.id)}
						style={box(region)}
					>
						{tab === "regions" && <span>{region.name}</span>}
					</button>
				))}
				{largestFirst(map.zones).map((zone) => (
					<button
						key={zone.id}
						type="button"
						className="media-pixel-zone"
						aria-label={`${zone.name} pixel zone`}
						aria-pressed={zone.id === selectedZoneId}
						data-enabled={zone.enabled ? "true" : "false"}
						title={`${zone.name}: ${zone.columns} by ${zone.rows}`}
						disabled={tab !== "zones"}
						tabIndex={tab === "zones" ? 0 : -1}
						onClick={() => onSelectZone(zone.id)}
						style={box(zone)}
					>
						{tab === "zones" && <span>{zone.name}</span>}
					</button>
				))}
			</div>
			<figcaption>
				{output.name} · {width} × {height}
				{pictureFailed ? " · Live picture unavailable" : " · Live picture"}
			</figcaption>
		</figure>
	);
}
