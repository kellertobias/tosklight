// Display regions as one editable table: which slice of the canvas each screen shows.

import { DataTable, type DataTableColumn } from "@tosklight/ui/window-kit";
import type { DisplayRegionView } from "../../shared/api/generated/media-wire";
import {
	CheckCell,
	NumberCell,
	RemoveCell,
	SelectCell,
	TextCell,
} from "./pixelMapCells";
import { REGION_FITS, REGION_ROTATIONS } from "./pixelMapEditing";

type Corner = { corner: "start" | "end"; axis: "x" | "y"; label: string };

const CORNERS: Corner[] = [
	{ corner: "start", axis: "x", label: "Left" },
	{ corner: "start", axis: "y", label: "Top" },
	{ corner: "end", axis: "x", label: "Right" },
	{ corner: "end", axis: "y", label: "Bottom" },
];

export function PixelRegionTable({
	regions,
	selectedId,
	onSelect,
	onChange,
	onRemove,
}: {
	regions: DisplayRegionView[];
	selectedId: string | null;
	onSelect: (id: string) => void;
	onChange: (region: DisplayRegionView) => void;
	onRemove: (id: string) => void;
}) {
	const columns: DataTableColumn<DisplayRegionView>[] = [
		{
			id: "name",
			header: "Name",
			width: "minmax(170px,1.4fr)",
			render: (region) => (
				<TextCell
					label={`${region.name} name`}
					value={region.name}
					onChange={(name) => onChange({ ...region, name })}
				/>
			),
		},
		...CORNERS.map(
			({ corner, axis, label }): DataTableColumn<DisplayRegionView> => ({
				id: `${corner}-${axis}`,
				header: label,
				width: "100px",
				align: "right",
				render: (region) => (
					<NumberCell
						label={`${region.name} ${label.toLowerCase()}`}
						fraction
						value={region[corner][axis]}
						onChange={(value) =>
							onChange({
								...region,
								[corner]: { ...region[corner], [axis]: value },
							})
						}
					/>
				),
			}),
		),
		{
			id: "rotation",
			header: "Rotation",
			width: "minmax(150px,1fr)",
			render: (region) => (
				<SelectCell
					label={`${region.name} rotation`}
					value={region.rotation}
					options={REGION_ROTATIONS}
					onChange={(rotation) => onChange({ ...region, rotation })}
				/>
			),
		},
		{
			id: "fit",
			header: "Fit",
			width: "minmax(150px,1fr)",
			render: (region) => (
				<SelectCell
					label={`${region.name} fit`}
					value={region.fit}
					options={REGION_FITS}
					onChange={(fit) => onChange({ ...region, fit })}
				/>
			),
		},
		{
			id: "enabled",
			header: "Show",
			width: "72px",
			align: "center",
			render: (region) => (
				<CheckCell
					label={`Show ${region.name}`}
					checked={region.enabled}
					onChange={(enabled) => onChange({ ...region, enabled })}
				/>
			),
		},
		{
			id: "remove",
			header: "",
			width: "88px",
			align: "right",
			render: (region) => (
				<RemoveCell
					label={`Remove ${region.name}`}
					onRemove={() => onRemove(region.id)}
				/>
			),
		},
	];
	if (regions.length === 0) {
		return (
			<p className="media-state is-empty">
				No display region yet. Add one to choose which slice of the canvas a
				screen shows.
			</p>
		);
	}
	return (
		<div className="media-pixel-table-scroll">
			<DataTable
				className="media-pixel-table"
				columns={columns}
				rows={regions}
				rowKey={(region) => region.id}
				selected={(region) => region.id === selectedId}
				rowDataAttributes={(region) => ({
					"aria-selected": region.id === selectedId ? "true" : "false",
					"aria-label": region.name,
				})}
				activeIndex={Math.max(
					0,
					regions.findIndex((region) => region.id === selectedId),
				)}
				onActivate={(region) => onSelect(region.id)}
				rowHeight={52}
			/>
		</div>
	);
}
