import type { FixtureProfile } from "../../../api/types";
import {
	Button,
	FormLayout,
	NumberField,
	SelectField,
	TextAreaField,
	TextField,
} from "../../common";
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
	onChange: (draft: FixtureProfile) => void;
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
						onChange={(event) =>
							onChange({ ...draft, manufacturer: event.target.value })
						}
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
					onChange={(event) => onChange({ ...draft, name: event.target.value })}
				/>
				<TextField
					label="Fixture short name"
					clearable
					value={draft.short_name}
					onChange={(event) =>
						onChange({ ...draft, short_name: event.target.value })
					}
				/>
				<SelectField
					label="Fixture type"
					value={draft.fixture_type}
					options={FIXTURE_TYPES.map((value) => ({
						value,
						label: value,
					}))}
					onChange={(fixture_type) => onChange({ ...draft, fixture_type })}
				/>
				<AssetField
					label="Fixture icon"
					value={draft.stage_icon_asset}
					extensions={["png", "jpg", "jpeg", "webp"]}
					onChange={(stage_icon_asset) =>
						onChange({ ...draft, stage_icon_asset })
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
					onChange={(event) =>
						onChange({ ...draft, notes: event.target.value })
					}
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
						onChange({ ...draft, photograph_asset })
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
					onChange={(model_asset) => onChange({ ...draft, model_asset })}
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
						onChange={(event) =>
							onChange({
								...draft,
								physical: {
									...draft.physical,
									[key]: optionalNumber(event.target.value),
								},
							})
						}
					/>
				))}
				<NumberField
					label="Color temperature (K)"
					allowDecimal
					min={0}
					value={draft.physical.color_temperature_kelvin ?? ""}
					onChange={(event) =>
						onChange({
							...draft,
							physical: {
								...draft.physical,
								color_temperature_kelvin: optionalNumber(event.target.value),
							},
						})
					}
				/>
				<NumberField
					label="Luminous output (lm)"
					allowDecimal
					min={0}
					value={draft.physical.luminous_output_lumens ?? ""}
					onChange={(event) =>
						onChange({
							...draft,
							physical: {
								...draft.physical,
								luminous_output_lumens: optionalNumber(event.target.value),
							},
						})
					}
				/>
				<NumberField
					label="Beam angle (degrees)"
					allowDecimal
					min={0}
					value={draft.physical.beam_angle_degrees ?? ""}
					onChange={(event) =>
						onChange({
							...draft,
							physical: {
								...draft.physical,
								beam_angle_degrees: optionalNumber(event.target.value),
							},
						})
					}
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
