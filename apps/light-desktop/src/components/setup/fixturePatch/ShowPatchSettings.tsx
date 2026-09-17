import { SwitchField } from "@tosklight/ui";
import { WindowSettings } from "@tosklight/ui/window-kit";
import { useApp } from "../../../state/AppContext";
import { PATCH_COLUMNS, type PatchColumn } from "../../../types";
import { MediaServerCacheSettings } from "../MediaServerCacheSettings";
import { TrackingSettingsForm } from "../PsnSourceForm";

/**
 * One switch per Show Patch column. The last column still shown cannot be switched off, so the
 * table always has something to draw.
 */
export function PatchColumnSwitches({
	hidden,
	onChange,
}: {
	hidden: readonly PatchColumn[];
	onChange: (hidden: PatchColumn[]) => void;
}) {
	const shown = PATCH_COLUMNS.length - hidden.length;
	return (
		<div className="fixture-sheet-column-options">
			{PATCH_COLUMNS.map(({ id, label }) => {
				const visible = !hidden.includes(id);
				return (
					<SwitchField
						key={id}
						label={label}
						offLabel="Hidden"
						onLabel="Visible"
						checked={visible}
						disabled={visible && shown === 1}
						onChange={(event) =>
							onChange(
								event.target.checked
									? hidden.filter((column) => column !== id)
									: [...hidden, id],
							)
						}
					/>
				);
			})}
		</div>
	);
}

export type ShowPatchSettingsTab = "columns" | "media" | "tracking";

/**
 * The Show Patch window's own settings, opened from the same top-right ⚙ on Fixtures, Media
 * Servers and Tracking so the header never changes shape between them. A pane reaches the same
 * pages through its pane settings instead.
 */
export function ShowPatchSettings({
	anchor,
	initialTab = "columns",
	onClose,
}: {
	anchor: DOMRect;
	initialTab?: ShowPatchSettingsTab;
	onClose: () => void;
}) {
	const { state, dispatch } = useApp();
	return (
		<WindowSettings
			modal={false}
			anchor={anchor}
			title="Show Patch"
			initialTab={initialTab}
			onClose={onClose}
			tabs={[
				{
					id: "columns",
					label: "Columns",
					content: (
						<section>
							<h3>Visible columns</h3>
							<PatchColumnSwitches
								hidden={state.patchHiddenColumns ?? []}
								onChange={(columns) =>
									dispatch({ type: "SET_PATCH_HIDDEN_COLUMNS", columns })
								}
							/>
						</section>
					),
				},
				{
					id: "media",
					label: "Media Servers",
					content: <MediaServerCacheSettings />,
				},
				{
					id: "tracking",
					label: "Tracking",
					content: <TrackingSettingsForm />,
				},
			]}
		/>
	);
}
