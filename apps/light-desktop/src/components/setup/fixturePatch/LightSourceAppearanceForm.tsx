import { ColorPickerField, Select, TextField } from "@tosklight/ui";
import type { Dispatch, SetStateAction } from "react";
import type {
	GelAssignment,
	InstalledLightSource,
	PatchedFixture,
} from "../../../api/types";
import type { PatchController } from "./controller";
import { GelCatalogPanel } from "./GelCatalogPanel";
import {
	type AppearanceDraft,
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
			{/* biome-ignore lint/a11y/noLabelWithoutControl: Select renders its native control inside this label. */}
			<label>
				Gel / filter
				<Select
					aria-label="Installed gel or filter"
					value={draft.gelType}
					onChange={(event) =>
						update({ gelType: event.target.value as GelAssignment["type"] })
					}
				>
					<option value="open_white">Open white</option>
					<option value="built_in">Catalog gel</option>
					<option value="custom">Custom color</option>
				</Select>
			</label>
			<GelCatalogPanel
				active={draft.gelType === "built_in"}
				api={controller.server}
				selectedGel={draft.catalogGel}
				onSelect={(catalogGel) => update({ catalogGel })}
				onError={onCatalogError}
			/>
			{draft.gelType === "custom" && (
				<CustomGelFields draft={draft} update={update} />
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
