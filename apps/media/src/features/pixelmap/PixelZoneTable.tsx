// Pixel zones and the routes that carry them, each as one editable table.

import { DataTable, type DataTableColumn } from "@tosklight/ui/window-kit";
import type {
	PixelRouteView,
	PixelZoneView,
} from "../../shared/api/generated/media-wire";
import {
	CheckCell,
	NumberCell,
	RemoveCell,
	SelectCell,
	TextCell,
} from "./pixelMapCells";
import { footprintOf, PIXEL_LAYOUTS, PIXEL_ORDERS } from "./pixelMapEditing";

const CORNERS = [
	{ corner: "start", axis: "x", label: "Left" },
	{ corner: "start", axis: "y", label: "Top" },
	{ corner: "end", axis: "x", label: "Right" },
	{ corner: "end", axis: "y", label: "Bottom" },
] as const;

const PROTOCOLS = [
	{ value: "art-net", label: "Art-Net" },
	{ value: "sacn", label: "sACN" },
];

type EditZone = (zone: PixelZoneView, patch: Partial<PixelZoneView>) => void;

/** Where the zone sits on the canvas and how many pixels it holds. */
function placementColumns(edit: EditZone): DataTableColumn<PixelZoneView>[] {
	return [
		{
			id: "name",
			header: "Name",
			width: "minmax(170px,1.4fr)",
			render: (zone) => (
				<TextCell
					label={`${zone.name} name`}
					value={zone.name}
					onChange={(name) => edit(zone, { name })}
				/>
			),
		},
		...CORNERS.map(
			({ corner, axis, label }): DataTableColumn<PixelZoneView> => ({
				id: `${corner}-${axis}`,
				header: label,
				width: "100px",
				align: "right",
				render: (zone) => (
					<NumberCell
						label={`${zone.name} ${label.toLowerCase()}`}
						fraction
						value={zone[corner][axis]}
						onChange={(value) =>
							edit(zone, { [corner]: { ...zone[corner], [axis]: value } })
						}
					/>
				),
			}),
		),
		{
			id: "columns",
			header: "Across",
			width: "100px",
			align: "right",
			render: (zone) => (
				<NumberCell
					label={`${zone.name} pixels across`}
					min={1}
					value={zone.columns}
					onChange={(columns) => edit(zone, { columns })}
				/>
			),
		},
		{
			id: "rows",
			header: "Down",
			width: "100px",
			align: "right",
			render: (zone) => (
				<NumberCell
					label={`${zone.name} pixels down`}
					min={1}
					value={zone.rows}
					onChange={(rows) => edit(zone, { rows })}
				/>
			),
		},
	];
}

/** What each pixel is and where its values are sent. */
function outputColumns(
	edit: EditZone,
	onRemove: (id: string) => void,
): DataTableColumn<PixelZoneView>[] {
	return [
		{
			id: "layout",
			header: "Fixture type",
			width: "minmax(150px,1fr)",
			render: (zone) => (
				<SelectCell
					label={`${zone.name} fixture type`}
					value={zone.layout.name}
					options={PIXEL_LAYOUTS.map((layout) => ({
						value: layout.name,
						label: `${layout.name} · ${layout.components.length} ch`,
					}))}
					onChange={(name) => {
						const layout = PIXEL_LAYOUTS.find((entry) => entry.name === name);
						if (layout)
							edit(zone, {
								layout: {
									name: layout.name,
									components: [...layout.components],
								},
							});
					}}
				/>
			),
		},
		{
			id: "order",
			header: "Wiring",
			width: "minmax(170px,1fr)",
			render: (zone) => (
				<SelectCell
					label={`${zone.name} wiring order`}
					value={zone.order}
					options={PIXEL_ORDERS}
					onChange={(order) => edit(zone, { order })}
				/>
			),
		},
		{
			id: "universe",
			header: "Universe",
			width: "100px",
			align: "right",
			render: (zone) => (
				<NumberCell
					label={`${zone.name} output universe`}
					min={0}
					value={zone.universe}
					onChange={(universe) => edit(zone, { universe })}
				/>
			),
		},
		{
			id: "address",
			header: "Address",
			width: "100px",
			align: "right",
			render: (zone) => (
				<NumberCell
					label={`${zone.name} output address`}
					min={1}
					max={512}
					value={zone.startAddress}
					onChange={(startAddress) => edit(zone, { startAddress })}
				/>
			),
		},
		{
			id: "footprint",
			header: "Slots",
			width: "56px",
			align: "right",
			render: (zone) => <span>{footprintOf(zone)}</span>,
		},
		{
			id: "enabled",
			header: "Send",
			width: "64px",
			align: "center",
			render: (zone) => (
				<CheckCell
					label={`Send ${zone.name}`}
					checked={zone.enabled}
					onChange={(enabled) => edit(zone, { enabled })}
				/>
			),
		},
		{
			id: "remove",
			header: "",
			width: "88px",
			align: "right",
			render: (zone) => (
				<RemoveCell
					label={`Remove ${zone.name}`}
					onRemove={() => onRemove(zone.id)}
				/>
			),
		},
	];
}

export function PixelZoneTable({
	zones,
	selectedId,
	onSelect,
	onChange,
	onRemove,
}: {
	zones: PixelZoneView[];
	selectedId: string | null;
	onSelect: (id: string) => void;
	onChange: (zone: PixelZoneView) => void;
	onRemove: (id: string) => void;
}) {
	const edit = (zone: PixelZoneView, patch: Partial<PixelZoneView>) => {
		const next = { ...zone, ...patch };
		onChange({ ...next, footprint: footprintOf(next) });
	};
	if (zones.length === 0) {
		return (
			<p className="media-state is-empty">
				No pixel zone yet. Add one to send part of the picture as DMX.
			</p>
		);
	}
	return (
		<div className="media-pixel-table-scroll">
			<DataTable
				className="media-pixel-table"
				columns={[...placementColumns(edit), ...outputColumns(edit, onRemove)]}
				rows={zones}
				rowKey={(zone) => zone.id}
				selected={(zone) => zone.id === selectedId}
				rowDataAttributes={(zone) => ({
					"aria-selected": zone.id === selectedId ? "true" : "false",
					"aria-label": zone.name,
				})}
				activeIndex={Math.max(
					0,
					zones.findIndex((zone) => zone.id === selectedId),
				)}
				onActivate={(zone) => onSelect(zone.id)}
				rowHeight={52}
			/>
		</div>
	);
}

export function PixelRouteTable({
	routes,
	onChange,
	onRemove,
}: {
	routes: PixelRouteView[];
	onChange: (route: PixelRouteView) => void;
	onRemove: (id: string) => void;
}) {
	const columns: DataTableColumn<PixelRouteView>[] = [
		{
			id: "name",
			header: "Name",
			width: "minmax(170px,1fr)",
			render: (route) => (
				<TextCell
					label={`${route.name} route name`}
					value={route.name}
					onChange={(name) => onChange({ ...route, name })}
				/>
			),
		},
		{
			id: "protocol",
			header: "Protocol",
			width: "minmax(130px,0.8fr)",
			render: (route) => (
				<SelectCell
					label={`${route.name} protocol`}
					value={route.protocol}
					options={PROTOCOLS}
					onChange={(protocol) => onChange({ ...route, protocol })}
				/>
			),
		},
		{
			id: "universe",
			header: "Universe",
			width: "100px",
			align: "right",
			render: (route) => (
				<NumberCell
					label={`${route.name} route universe`}
					min={0}
					value={route.universe}
					onChange={(universe) => onChange({ ...route, universe })}
				/>
			),
		},
		{
			id: "destination",
			header: "Destination (empty = broadcast)",
			width: "minmax(190px,1.4fr)",
			render: (route) => (
				<TextCell
					label={`${route.name} destination`}
					value={route.destination ?? ""}
					onChange={(value) =>
						onChange({ ...route, destination: value.trim() || null })
					}
				/>
			),
		},
		{
			id: "enabled",
			header: "Send",
			width: "64px",
			align: "center",
			render: (route) => (
				<CheckCell
					label={`Send on ${route.name}`}
					checked={route.enabled}
					onChange={(enabled) => onChange({ ...route, enabled })}
				/>
			),
		},
		{
			id: "remove",
			header: "",
			width: "88px",
			align: "right",
			render: (route) => (
				<RemoveCell
					label={`Remove route ${route.name}`}
					onRemove={() => onRemove(route.id)}
				/>
			),
		},
	];
	if (routes.length === 0) {
		return (
			<p className="media-state is-empty">
				No output route yet. Zones are not sent until a route carries their
				universe.
			</p>
		);
	}
	return (
		<div className="media-pixel-table-scroll">
			<DataTable
				className="media-pixel-table"
				columns={columns}
				rows={routes}
				rowKey={(route) => route.id}
				rowHeight={52}
			/>
		</div>
	);
}
