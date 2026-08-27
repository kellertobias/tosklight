// Drawing a rig onto the picture: which parts of this output's canvas become DMX, where those
// values are sent, and which screens show which slice of it.
//
// The canvas here is a proportional stand-in rather than a live preview. What matters when placing
// a zone is where it sits relative to the picture and to the other zones, and a still frame of
// whatever happened to be playing would make that harder to see rather than easier.

import {
	Button,
	CheckboxField,
	NumberField,
	SelectField,
	TextField,
} from "@tosklight/ui/controls";
import { useState } from "react";
import type {
	DisplayRegionView,
	OutputConfigurationView,
	PixelMapView,
	PixelRouteView,
	PixelZoneView,
} from "../../shared/api/generated/media-wire";
import {
	PIXEL_LAYOUTS,
	PIXEL_ORDERS,
	REGION_FITS,
	REGION_ROTATIONS,
	footprintOf,
	newRegion,
	newRoute,
	newZone,
	pixelMapProblems,
} from "./pixelMapEditing";

/** A percentage for CSS, from a canvas fraction. */
function percent(value: number): string {
	return `${Math.max(0, Math.min(1, value)) * 100}%`;
}

/// The zones and regions drawn over a stand-in for the canvas, at the output's own aspect ratio.
function CanvasOverlay({
	map,
	width,
	height,
	selectedZoneId,
	onSelectZone,
}: {
	map: PixelMapView;
	width: number;
	height: number;
	selectedZoneId: string | null;
	onSelectZone: (id: string) => void;
}) {
	return (
		<div
			className="media-pixel-canvas"
			style={{ aspectRatio: `${Math.max(width, 1)} / ${Math.max(height, 1)}` }}
			aria-label={`Canvas, ${width} by ${height}`}
		>
			{map.regions.map((region) => (
				<span
					key={region.id}
					className="media-pixel-region"
					title={`${region.name} shows this slice`}
					style={{
						left: percent(Math.min(region.start.x, region.end.x)),
						top: percent(Math.min(region.start.y, region.end.y)),
						width: percent(Math.abs(region.end.x - region.start.x)),
						height: percent(Math.abs(region.end.y - region.start.y)),
					}}
				/>
			))}
			{map.zones.map((zone) => (
				<button
					key={zone.id}
					type="button"
					className="media-pixel-zone"
					aria-pressed={zone.id === selectedZoneId}
					data-enabled={zone.enabled ? "true" : "false"}
					title={`${zone.name}: ${zone.columns} by ${zone.rows}`}
					onClick={() => onSelectZone(zone.id)}
					style={{
						left: percent(Math.min(zone.start.x, zone.end.x)),
						top: percent(Math.min(zone.start.y, zone.end.y)),
						width: percent(Math.abs(zone.end.x - zone.start.x)),
						height: percent(Math.abs(zone.end.y - zone.start.y)),
					}}
				>
					<span>{zone.name}</span>
				</button>
			))}
		</div>
	);
}

function ZoneEditor({
	zone,
	onChange,
	onRemove,
}: {
	zone: PixelZoneView;
	onChange: (zone: PixelZoneView) => void;
	onRemove: () => void;
}) {
	const edit = (patch: Partial<PixelZoneView>) => {
		const next = { ...zone, ...patch };
		onChange({ ...next, footprint: footprintOf(next) });
	};
	const corner = (
		corner: "start" | "end",
		axis: "x" | "y",
		label: string,
	) => (
		<NumberField
			label={label}
			description="A fraction of the canvas, from zero to one."
			min={0}
			max={1}
			step={0.01}
			value={String(zone[corner][axis])}
			onChange={(event) =>
				edit({ [corner]: { ...zone[corner], [axis]: Number(event.target.value) } })
			}
		/>
	);
	return (
		<div className="media-pixel-zone-editor">
			<TextField
				label="Name"
				value={zone.name}
				onChange={(event) => edit({ name: event.target.value })}
			/>
			{corner("start", "x", "Left")}
			{corner("start", "y", "Top")}
			{corner("end", "x", "Right")}
			{corner("end", "y", "Bottom")}
			<NumberField
				label="Pixels across"
				min={1}
				step={1}
				value={String(zone.columns)}
				onChange={(event) => edit({ columns: Number(event.target.value) })}
			/>
			<NumberField
				label="Pixels down"
				min={1}
				step={1}
				value={String(zone.rows)}
				onChange={(event) => edit({ rows: Number(event.target.value) })}
			/>
			<SelectField
				label="Fixture type"
				description="The channels each pixel occupies, in wire order."
				value={zone.layout.name}
				options={PIXEL_LAYOUTS.map((layout) => ({
					value: layout.name,
					label: `${layout.name} · ${layout.components.length} channels`,
				}))}
				onChange={(name) => {
					const layout = PIXEL_LAYOUTS.find((candidate) => candidate.name === name);
					if (layout) {
						edit({ layout: { name: layout.name, components: [...layout.components] } });
					}
				}}
			/>
			<SelectField
				label="Wiring order"
				description="The path addresses run along, which is how the strip is actually wired."
				value={zone.order}
				options={PIXEL_ORDERS}
				onChange={(order) => edit({ order })}
			/>
			<NumberField
				label="Universe"
				min={0}
				step={1}
				value={String(zone.universe)}
				onChange={(event) => edit({ universe: Number(event.target.value) })}
			/>
			<NumberField
				label="First address"
				description={`This zone occupies ${footprintOf(zone)} slots.`}
				min={1}
				max={512}
				step={1}
				value={String(zone.startAddress)}
				onChange={(event) => edit({ startAddress: Number(event.target.value) })}
			/>
			<CheckboxField
				label="Send this zone"
				checked={zone.enabled}
				onChange={(event) => edit({ enabled: event.target.checked })}
			/>
			<Button onClick={onRemove}>Remove zone</Button>
		</div>
	);
}

function RouteRow({
	route,
	onChange,
	onRemove,
}: {
	route: PixelRouteView;
	onChange: (route: PixelRouteView) => void;
	onRemove: () => void;
}) {
	return (
		<div className="media-pixel-route">
			<TextField
				label="Name"
				value={route.name}
				onChange={(event) => onChange({ ...route, name: event.target.value })}
			/>
			<SelectField
				label="Protocol"
				value={route.protocol}
				options={[
					{ value: "art-net", label: "Art-Net" },
					{ value: "sacn", label: "sACN" },
				]}
				onChange={(protocol) => onChange({ ...route, protocol })}
			/>
			<NumberField
				label="Universe"
				min={0}
				step={1}
				value={String(route.universe)}
				onChange={(event) => onChange({ ...route, universe: Number(event.target.value) })}
			/>
			<TextField
				label="Destination"
				description="A host, or a host and port. Leave empty to broadcast, or to use sACN's own multicast group."
				value={route.destination ?? ""}
				onChange={(event) =>
					onChange({ ...route, destination: event.target.value.trim() || null })
				}
			/>
			<CheckboxField
				label="Send on this route"
				checked={route.enabled}
				onChange={(event) => onChange({ ...route, enabled: event.target.checked })}
			/>
			<Button onClick={onRemove}>Remove route</Button>
		</div>
	);
}

function RegionRow({
	region,
	onChange,
	onRemove,
}: {
	region: DisplayRegionView;
	onChange: (region: DisplayRegionView) => void;
	onRemove: () => void;
}) {
	const corner = (corner: "start" | "end", axis: "x" | "y", label: string) => (
		<NumberField
			label={label}
			min={0}
			max={1}
			step={0.01}
			value={String(region[corner][axis])}
			onChange={(event) =>
				onChange({
					...region,
					[corner]: { ...region[corner], [axis]: Number(event.target.value) },
				})
			}
		/>
	);
	return (
		<div className="media-pixel-region-editor">
			<TextField
				label="Name"
				value={region.name}
				onChange={(event) => onChange({ ...region, name: event.target.value })}
			/>
			{corner("start", "x", "Left")}
			{corner("start", "y", "Top")}
			{corner("end", "x", "Right")}
			{corner("end", "y", "Bottom")}
			<SelectField
				label="Rotation"
				description="Applied to this screen alone; the canvas and the other screens are untouched."
				value={region.rotation}
				options={REGION_ROTATIONS}
				onChange={(rotation) => onChange({ ...region, rotation })}
			/>
			<SelectField
				label="Fit"
				value={region.fit}
				options={REGION_FITS}
				onChange={(fit) => onChange({ ...region, fit })}
			/>
			<CheckboxField
				label="Show this region"
				checked={region.enabled}
				onChange={(event) => onChange({ ...region, enabled: event.target.checked })}
			/>
			<Button onClick={onRemove}>Remove region</Button>
		</div>
	);
}

/// The whole pixel map of one output.
export function PixelMapSettings({
	output,
	busy,
	onSave,
}: {
	output: OutputConfigurationView;
	busy: boolean;
	onSave: (map: PixelMapView) => void;
}) {
	const [map, setMap] = useState<PixelMapView>(output.pixelMap);
	const [selectedZoneId, setSelectedZoneId] = useState<string | null>(
		output.pixelMap.zones[0]?.id ?? null,
	);
	const selected = map.zones.find((zone) => zone.id === selectedZoneId);
	const problems = pixelMapProblems(map);

	const replaceZone = (zone: PixelZoneView) =>
		setMap((current) => ({
			...current,
			zones: current.zones.map((candidate) => (candidate.id === zone.id ? zone : candidate)),
		}));

	return (
		<article className="media-settings-section" aria-label={`${output.name} pixel map`}>
			<div className="media-settings-section-heading">
				<h3>{output.name} pixel map</h3>
			</div>
			<SelectField
				label="Output mode"
				description="Direct sends the mapped values from here. Desk merge hands them to the lighting desk instead."
				value={map.mode}
				options={[
					{ value: "direct", label: "Direct Media Server output" },
					{ value: "desk-merge", label: "Hand to the lighting desk" },
				]}
				onChange={(mode) => setMap((current) => ({ ...current, mode }))}
			/>
			<CanvasOverlay
				map={map}
				width={output.width}
				height={output.height}
				selectedZoneId={selectedZoneId}
				onSelectZone={setSelectedZoneId}
			/>
			<div className="media-settings-actions">
				<Button
					onClick={() => {
						const zone = newZone(map.zones);
						setMap((current) => ({ ...current, zones: [...current.zones, zone] }));
						setSelectedZoneId(zone.id);
					}}
				>
					Add pixel zone
				</Button>
				<Button
					onClick={() =>
						setMap((current) => ({ ...current, routes: [...current.routes, newRoute(current.routes)] }))
					}
				>
					Add output route
				</Button>
				<Button
					onClick={() =>
						setMap((current) => ({
							...current,
							regions: [...current.regions, newRegion(current.regions)],
						}))
					}
				>
					Add display region
				</Button>
			</div>
			{selected ? (
				<ZoneEditor
					key={selected.id}
					zone={selected}
					onChange={replaceZone}
					onRemove={() => {
						setMap((current) => ({
							...current,
							zones: current.zones.filter((zone) => zone.id !== selected.id),
						}));
						setSelectedZoneId(null);
					}}
				/>
			) : (
				<p>No pixel zone selected. Add one, or choose one on the canvas above.</p>
			)}
			{map.routes.map((route) => (
				<RouteRow
					key={route.id}
					route={route}
					onChange={(next) =>
						setMap((current) => ({
							...current,
							routes: current.routes.map((candidate) =>
								candidate.id === next.id ? next : candidate,
							),
						}))
					}
					onRemove={() =>
						setMap((current) => ({
							...current,
							routes: current.routes.filter((candidate) => candidate.id !== route.id),
						}))
					}
				/>
			))}
			{map.regions.map((region) => (
				<RegionRow
					key={region.id}
					region={region}
					onChange={(next) =>
						setMap((current) => ({
							...current,
							regions: current.regions.map((candidate) =>
								candidate.id === next.id ? next : candidate,
							),
						}))
					}
					onRemove={() =>
						setMap((current) => ({
							...current,
							regions: current.regions.filter((candidate) => candidate.id !== region.id),
						}))
					}
				/>
			))}
			{problems.length > 0 && (
				<ul className="media-settings-problems" aria-label="Pixel map problems">
					{problems.map((problem) => (
						<li key={problem}>{problem}</li>
					))}
				</ul>
			)}
			<div className="media-settings-actions">
				<Button
					disabled={busy || problems.length > 0}
					onClick={() => onSave(map)}
				>
					Save pixel map
				</Button>
			</div>
		</article>
	);
}
