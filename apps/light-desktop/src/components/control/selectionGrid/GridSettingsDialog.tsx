import { Button, ModalPortal, NumberField, SelectField } from "@tosklight/ui";
import { useEffect, useState } from "react";
import type {
	SelectionGridConfiguration,
	SelectionGridMethod,
	SelectionGridState,
} from "../../../features/programmingInteraction/contracts";

export const SELECTION_GRID_METHOD_OPTIONS: readonly {
	value: SelectionGridMethod;
	label: string;
}[] = [
	{ value: "stage2d", label: "2D Stage" },
	{ value: "top_to_bottom", label: "Top to Bottom" },
	{ value: "bottom_to_top", label: "Bottom to Top" },
	{ value: "front_to_back", label: "Front to Back" },
	{ value: "back_to_front", label: "Back to Front" },
	{ value: "left_to_right", label: "Left to Right" },
	{ value: "right_to_left", label: "Right to Left" },
	{ value: "horizontal_axis_x", label: "Horizontal axis (X)" },
	{ value: "vertical_axis_z", label: "Vertical axis (Z)" },
	{ value: "room_depth_axis_y", label: "Room-depth axis (Y)" },
];

const AXIS_METHODS = new Set<SelectionGridMethod>([
	"horizontal_axis_x",
	"vertical_axis_z",
	"room_depth_axis_y",
]);

const CURSOR_LABELS = {
	top_left: "Top left",
	top_right: "Top right",
	bottom_left: "Bottom left",
	bottom_right: "Bottom right",
} as const;

export function GridSettingsDialog({
	grid,
	busy,
	error,
	onSave,
	onClose,
}: {
	grid: SelectionGridState;
	busy: boolean;
	error: string | null;
	onSave: (configuration: SelectionGridConfiguration) => void;
	onClose: () => void;
}) {
	const [configuration, setConfiguration] = useState(grid.configuration);

	useEffect(() => {
		setConfiguration(grid.configuration);
	}, [grid.configuration]);

	const updateOrigin = (axis: "x" | "y" | "z", value: string) => {
		const number = Number(value);
		if (!Number.isFinite(number)) return;
		setConfiguration((current) => ({
			...current,
			axisOrigin: { ...current.axisOrigin, [axis]: number },
		}));
	};

	return (
		<ModalPortal onClose={onClose}>
			<div
				className="modal-backdrop"
				onPointerDown={(event) =>
					event.target === event.currentTarget && !busy && onClose()
				}
			>
				<section
					className="modal-card selection-grid-settings"
					role="dialog"
					aria-modal="true"
					aria-label="Grid Settings"
					aria-busy={busy}
				>
					<Button
						className="modal-close"
						aria-label="Close Grid Settings"
						disabled={busy}
						onClick={onClose}
					>
						×
					</Button>
					<h2>Grid Settings</h2>
					<SelectField
						label="Grid method"
						ariaLabel="Grid method"
						value={configuration.method}
						disabled={busy}
						options={[...SELECTION_GRID_METHOD_OPTIONS]}
						onChange={(method) =>
							setConfiguration((current) => ({ ...current, method }))
						}
					/>
					{AXIS_METHODS.has(configuration.method) ? (
						<fieldset>
							<legend>Axis origin</legend>
							{(["x", "y", "z"] as const).map((axis) => (
								<NumberField
									key={axis}
									label={axis.toUpperCase()}
									allowDecimal
									showStepButtons={false}
									value={configuration.axisOrigin[axis]}
									disabled={busy}
									onValueChange={(value) => updateOrigin(axis, value)}
								/>
							))}
						</fieldset>
					) : null}
					<dl className="selection-grid-cursors">
						<div>
							<dt>Rows-first next</dt>
							<dd>{CURSOR_LABELS[grid.rowsFirst]}</dd>
						</div>
						<div>
							<dt>Columns-first next</dt>
							<dd>{CURSOR_LABELS[grid.columnsFirst]}</dd>
						</div>
					</dl>
					{error ? <p role="alert">{error}</p> : null}
					<div className="modal-actions">
						<Button disabled={busy} onClick={onClose}>
							Cancel
						</Button>
						<Button
							variant="primary"
							disabled={busy}
							onClick={() => onSave(configuration)}
						>
							{busy ? "Saving…" : "Save Grid Settings"}
						</Button>
					</div>
				</section>
			</div>
		</ModalPortal>
	);
}
