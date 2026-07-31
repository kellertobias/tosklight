import {
	Button,
	FormLayout,
	NumberField,
	SelectField,
	TextAreaField,
	TextField,
} from "@tosklight/ui";
import type { Dispatch, SetStateAction } from "react";
import type { FixtureProfile } from "../../../api/types";
import { AssetField } from "./assets";

const FIXTURE_TYPES = [
	"dimmer",
	"fogger",
	"profile",
	"wash",
	"wash mover",
	"spot mover",
	"beam mover",
	"strobe",
	"media server",
	"pixel fixture",
	"other",
];

type GenericSectionProps = {
	draft: FixtureProfile;
	onChange: Dispatch<SetStateAction<FixtureProfile>>;
};

function optionalNumber(value: string) {
	return value === "" ? null : Number(value);
}

function IdentitySection({
	draft,
	onChange,
	onLookup,
}: GenericSectionProps & { onLookup: () => void }) {
	return (
		<section>
			<h3>Identity</h3>
			<FormLayout columns={3} minColumnWidth={190}>
				<div className="fixture-manufacturer-field">
					<TextField
						required
						label="Manufacturer"
						clearable
						value={draft.manufacturer}
						onChange={(event) => {
							const manufacturer = event.target.value;
							onChange((current) => ({
								...current,
								manufacturer,
							}));
						}}
					/>
					<Button
						iconOnly
						aria-label="Look up manufacturer"
						title="Look up manufacturer"
						onClick={onLookup}
					>
						⌕
					</Button>
				</div>
				<TextField
					required
					label="Fixture name"
					clearable
					value={draft.name}
					onChange={(event) => {
						const name = event.target.value;
						onChange((current) => ({ ...current, name }));
					}}
				/>
				<TextField
					label="Fixture short name"
					clearable
					value={draft.short_name}
					onChange={(event) => {
						const short_name = event.target.value;
						onChange((current) => ({
							...current,
							short_name,
						}));
					}}
				/>
				<SelectField
					label="Fixture type"
					value={draft.fixture_type}
					options={FIXTURE_TYPES.map((value) => ({
						value,
						label: value,
					}))}
					onChange={(fixture_type) =>
						onChange((current) => ({ ...current, fixture_type }))
					}
				/>
				<AssetField
					label="Fixture icon"
					value={draft.stage_icon_asset}
					extensions={["png", "jpg", "jpeg", "webp"]}
					onChange={(stage_icon_asset) =>
						onChange((current) => ({ ...current, stage_icon_asset }))
					}
				/>
			</FormLayout>
		</section>
	);
}

function NotesAssetsSection({ draft, onChange }: GenericSectionProps) {
	return (
		<section className="fixture-notes-assets">
			<div>
				<h3>Notes</h3>
				<TextAreaField
					label="Fixture notes"
					rows={9}
					value={draft.notes}
					onChange={(event) => {
						const notes = event.target.value;
						onChange((current) => ({ ...current, notes }));
					}}
				/>
			</div>
			<div>
				<h3>Fixture photograph</h3>
				<AssetField
					label="Photograph"
					preview="image"
					value={draft.photograph_asset}
					extensions={["png", "jpg", "jpeg", "gif", "webp"]}
					onChange={(photograph_asset) =>
						onChange((current) => ({ ...current, photograph_asset }))
					}
				/>
			</div>
			<div>
				<h3>Visualizer</h3>
				<AssetField
					label="Visualizer GLB model"
					preview="glb"
					value={draft.model_asset}
					extensions={["glb"]}
					onChange={(model_asset) =>
						onChange((current) => ({ ...current, model_asset }))
					}
				/>
			</div>
		</section>
	);
}

function PhysicalSection({ draft, onChange }: GenericSectionProps) {
	return (
		<section>
			<h3>Physical</h3>
			<FormLayout columns={5} minColumnWidth={145}>
				{(
					[
						["width_millimetres", "Width", "mm"],
						["height_millimetres", "Height", "mm"],
						["depth_millimetres", "Depth", "mm"],
						["weight_kilograms", "Weight", "kg"],
						["power_watts", "Power consumption", "W"],
					] as const
				).map(([key, label, unit]) => (
					<NumberField
						key={key}
						label={`${label} (${unit})`}
						allowDecimal
						min={0}
						value={draft.physical[key] ?? ""}
						onChange={(event) => {
							const value = optionalNumber(event.target.value);
							onChange((current) => ({
								...current,
								physical: {
									...current.physical,
									[key]: value,
								},
							}));
						}}
					/>
				))}
				<NumberField
					label="Color temperature (K)"
					allowDecimal
					min={0}
					value={draft.physical.color_temperature_kelvin ?? ""}
					onChange={(event) => {
						const color_temperature_kelvin = optionalNumber(event.target.value);
						onChange((current) => ({
							...current,
							physical: {
								...current.physical,
								color_temperature_kelvin,
							},
						}));
					}}
				/>
				<NumberField
					label="Luminous output (lm)"
					allowDecimal
					min={0}
					value={draft.physical.luminous_output_lumens ?? ""}
					onChange={(event) => {
						const luminous_output_lumens = optionalNumber(event.target.value);
						onChange((current) => ({
							...current,
							physical: {
								...current.physical,
								luminous_output_lumens,
							},
						}));
					}}
				/>
				<NumberField
					label="Beam angle (degrees)"
					allowDecimal
					min={0}
					value={draft.physical.beam_angle_degrees ?? ""}
					onChange={(event) => {
						const beam_angle_degrees = optionalNumber(event.target.value);
						onChange((current) => ({
							...current,
							physical: {
								...current.physical,
								beam_angle_degrees,
							},
						}));
					}}
				/>
			</FormLayout>
		</section>
	);
}

export function GenericProfileTab({
	draft,
	onChange,
	onLookup,
}: GenericSectionProps & { onLookup: () => void }) {
	return (
		<div className="fixture-generic-tab">
			<IdentitySection draft={draft} onChange={onChange} onLookup={onLookup} />
			<NotesAssetsSection draft={draft} onChange={onChange} />
			<PhysicalSection draft={draft} onChange={onChange} />
		</div>
	);
}
