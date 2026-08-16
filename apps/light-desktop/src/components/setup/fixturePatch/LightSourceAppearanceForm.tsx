import {
	Button,
	ColorPickerField,
	ModalFrame,
	Select,
	TextField,
} from "@tosklight/ui";
import { type Dispatch, type SetStateAction, useState } from "react";
import type {
	GelAssignment,
	InstalledLightSource,
	PatchedFixture,
} from "../../../api/types";
import type { PatchController } from "./controller";
import { GelCatalogPanel } from "./GelCatalogPanel";
import {
	type AppearanceDraft,
	gelSummary,
	profileOutputDescription,
	profileTemperatureDescription,
	SOURCE_OPTIONS,
} from "./lightSourceAppearanceModel";

export function LightSourceAppearanceForm({
	draft,
	setDraft,
	fixture,
	controller,
	onCatalogError,
}: {
	draft: AppearanceDraft;
	setDraft: Dispatch<SetStateAction<AppearanceDraft>>;
	fixture: PatchedFixture;
	controller: PatchController;
	onCatalogError: (message: string) => void;
}) {
	const [gelPickerOpen, setGelPickerOpen] = useState(false);
	const update = (change: Partial<AppearanceDraft>) =>
		setDraft((current) => ({ ...current, ...change }));
	return (
		<div className="light-source-editor-grid">
			{/* biome-ignore lint/a11y/noLabelWithoutControl: Select renders its native control inside this label. */}
			<label>
				Light source
				<Select
					autoFocus
					aria-label="Installed light source"
					value={draft.sourceType}
					onChange={(event) =>
						update({
							sourceType: event.target.value as InstalledLightSource["type"],
						})
					}
				>
					{SOURCE_OPTIONS.map((option) => (
						<option key={option.value} value={option.value}>
							{option.label}
						</option>
					))}
				</Select>
			</label>
			{draft.sourceType === "other" && (
				<TextField
					label="Other light-source name"
					value={draft.otherLabel}
					onChange={(event) => update({ otherLabel: event.target.value })}
				/>
			)}
			<TextField
				label="Color temperature (K)"
				description={profileTemperatureDescription(fixture)}
				inputMode="numeric"
				value={draft.colorTemperature}
				onChange={(event) => update({ colorTemperature: event.target.value })}
			/>
			<TextField
				label="Luminous output (lm)"
				description={profileOutputDescription(fixture)}
				inputMode="decimal"
				value={draft.luminousOutputLumens}
				onChange={(event) =>
					update({ luminousOutputLumens: event.target.value })
				}
			/>
			{/* biome-ignore lint/a11y/noLabelWithoutControl: Select renders its native control inside this label. */}
			<label>
				Gel / filter
				<Select
					aria-label="Installed gel or filter"
					value={draft.gelType}
					onChange={(event) => {
						const gelType = event.target.value as GelAssignment["type"];
						update({ gelType });
						if (gelType === "built_in") setGelPickerOpen(true);
					}}
				>
					<option value="open_white">Open white</option>
					<option value="built_in">Catalog gel</option>
					<option value="custom">Custom color</option>
				</Select>
			</label>
			{draft.gelType === "built_in" && (
				<Button onClick={() => setGelPickerOpen(true)}>
					{draft.catalogGel
						? `Selected gel · ${gelSummary(draft.catalogGel)}`
						: "Choose gel…"}
				</Button>
			)}
			{draft.gelType === "custom" && (
				<CustomGelFields draft={draft} update={update} />
			)}
			{gelPickerOpen && (
				<ModalFrame
					id="gel-selection"
					ariaLabel="Select gel"
					title="Select gel"
					closeLabel="Close gel selection"
					dialogClassName="gel-selection-modal"
					onClose={() => setGelPickerOpen(false)}
				>
					<GelCatalogPanel
						active
						api={controller.server}
						selectedGel={draft.catalogGel}
						onSelect={(catalogGel) => {
							update({ catalogGel, gelType: "built_in" });
							setGelPickerOpen(false);
						}}
						onError={onCatalogError}
					/>
				</ModalFrame>
			)}
		</div>
	);
}

function CustomGelFields({
	draft,
	update,
}: {
	draft: AppearanceDraft;
	update: (change: Partial<AppearanceDraft>) => void;
}) {
	return (
		<>
			<TextField
				label="Custom gel name"
				value={draft.customName}
				onChange={(event) => update({ customName: event.target.value })}
			/>
			<ColorPickerField
				label="Custom gel color"
				value={draft.customColor}
				onChange={(value) => update({ customColor: value })}
			/>
			<TextField
				label="Custom gel note (optional)"
				value={draft.customNote}
				onChange={(event) => update({ customNote: event.target.value })}
			/>
		</>
	);
}
