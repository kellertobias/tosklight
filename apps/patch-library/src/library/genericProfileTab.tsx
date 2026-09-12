import {
	Button,
	FormLayout,
	NumberField,
	SelectField,
	TextAreaField,
	TextField,
} from "@tosklight/ui";
import { useState } from "react";
import type { Dispatch, SetStateAction } from "react";
import type {
	FixtureBodyModel,
	FixtureProfile,
	FixtureProfileLightSource,
	FixtureProfileOptics,
} from "../wire";
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
	"venue",
	"rigging",
	"other",
];

type GenericSectionProps = {
	draft: FixtureProfile;
	onChange: Dispatch<SetStateAction<FixtureProfile>>;
};

function optionalNumber(value: string) {
	return value === "" ? null : Number(value);
}

/** A stored `0..1` figure as the percentage an operator reads and types. */
function percentOf(value: number | null | undefined) {
	return value === null || value === undefined ? "" : Math.round(value * 100);
}

function fractionOf(value: string) {
	if (value === "") {
		return null;
	}
	return Math.min(Math.max(Number(value) / 100, 0), 1);
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
			</FormLayout>
		</section>
	);
}

type OpticsUpdate = (
	change: (
		current: NonNullable<FixtureProfile["optics"]>,
	) => NonNullable<FixtureProfile["optics"]>,
) => void;

/** The three figures a manufacturer actually prints: what the light is, not how it is shaped. */
function MeasuredOpticalFields({
	optics,
	setOptics,
}: {
	optics: NonNullable<FixtureProfile["optics"]>;
	setOptics: OpticsUpdate;
}) {
	return (
		<>
			{(
				[
					["color_temperature_kelvin", "Color temperature (K)"],
					["luminous_output_lumens", "Luminous output (lm)"],
					["beam_angle_degrees", "Beam angle (degrees)"],
				] as const
			).map(([key, label]) => (
				<NumberField
					key={key}
					label={label}
					allowDecimal
					min={0}
					value={optics[key] ?? ""}
					onChange={(event) => {
						const value = optionalNumber(event.target.value);
						setOptics((current) => ({ ...current, [key]: value }));
					}}
				/>
			))}
		</>
	);
}

/**
 * What this fixture's light looks like.
 *
 * Every field here is optional and blank means "whatever this fixture type normally does", which
 * is how the whole shipped library behaves today. An operator fills these in when the type's own
 * answer is not right for the lantern in front of them — a Fresnel fitted with a different lens,
 * a 400 W engine where the type assumes 100 W.
 */
function OpticsSection({ draft, onChange }: GenericSectionProps) {
	const optics = draft.optics ?? {};
	const source = optics.light_source ?? null;
	// The two dimensions are typed one at a time, and a lens with only one of them is not a lens.
	// Holding what has been typed here — rather than in the profile — lets the first number stay
	// on screen while the second is still being entered.
	const [size, setSize] = useState({
		width: source ? String(source.width_millimetres) : "",
		height: source ? String(source.height_millimetres) : "",
	});
	const setOptics = (
		patch: (current: FixtureProfileOptics) => FixtureProfileOptics,
	) =>
		onChange((current) => ({
			...current,
			optics: patch(current.optics ?? {}),
		}));
	const setDimension = (key: "width" | "height", typed: string) => {
		const next = { ...size, [key]: typed };
		setSize(next);
		const width = Number(next.width);
		const height = Number(next.height);
		setOptics((current) => ({
			...current,
			light_source:
				width > 0 && height > 0
					? {
							form: current.light_source?.form ?? "round",
							width_millimetres: width,
							height_millimetres: height,
						}
					: null,
		}));
	};
	return (
		<section>
			<h3>Optics</h3>
			<p className="field-hint">
				Leave a field empty to use whatever this fixture type normally does.
			</p>
			<FormLayout columns={5} minColumnWidth={145}>
				<NumberField
					label="Relative output"
					allowDecimal
					min={0}
					value={optics.output ?? ""}
					onChange={(event) =>
						setOptics((current) => ({
							...current,
							output: optionalNumber(event.target.value),
						}))
					}
				/>
				<NumberField
					label="Sharpness (%)"
					min={0}
					max={100}
					value={percentOf(optics.sharpness)}
					onChange={(event) =>
						setOptics((current) => ({
							...current,
							sharpness: fractionOf(event.target.value),
						}))
					}
				/>
				<NumberField
					label="Uniformity (%)"
					min={0}
					max={100}
					value={percentOf(optics.uniformity)}
					onChange={(event) =>
						setOptics((current) => ({
							...current,
							uniformity: fractionOf(event.target.value),
						}))
					}
				/>
				<SelectField
					label="Light source shape"
					value={source?.form ?? "round"}
					onChange={(form) =>
						setOptics((current) =>
							current.light_source
								? {
										...current,
										light_source: { ...current.light_source, form },
									}
								: current,
						)
					}
					disabled={source === null}
					options={[
						{ value: "round", label: "Round" },
						{ value: "oval", label: "Oval" },
						{ value: "rectangular", label: "Rectangular" },
					]}
				/>
				<NumberField
					label="Light source width (mm)"
					allowDecimal
					min={0}
					value={size.width}
					onChange={(event) => setDimension("width", event.target.value)}
				/>
				<MeasuredOpticalFields optics={optics} setOptics={setOptics} />
				<NumberField
					label="Light source height (mm)"
					allowDecimal
					min={0}
					value={size.height}
					onChange={(event) => setDimension("height", event.target.value)}
				/>
			</FormLayout>
		</section>
	);
}

/**
 * Which generic body the Stage draws this fixture as.
 *
 * A fixture that ships its own GLB is drawn with it and this is only advice. Everything else was
 * drawn from a guess — the declared type and the channels a mode happens to have — which cannot
 * tell a PAR 64 from a PAR 16 or a two-cell blinder from an eight. Leave it unset to keep that
 * guess; name a body and the fixture is drawn as that.
 */
function BodySection({
	draft,
	onChange,
	bodyCatalogue,
}: GenericSectionProps & { bodyCatalogue: FixtureBodyModel[] }) {
	const groups: { group: string; bodies: FixtureBodyModel[] }[] = [];
	for (const body of bodyCatalogue) {
		const last = groups.at(-1);
		if (last && last.group === body.group) last.bodies.push(body);
		else groups.push({ group: body.group, bodies: [body] });
	}
	return (
		<section>
			<h3>Body</h3>
			<p className="field-hint">
				{draft.model_asset
					? "This fixture ships its own 3D model, which is what the Stage draws."
					: "Leave this unset to keep the body guessed from the fixture type and its channels."}
			</p>
			<FormLayout columns={2} minColumnWidth={240}>
				<SelectField
					label="Generic body"
					value={draft.body_model ?? ""}
					disabled={bodyCatalogue.length === 0}
					options={[
						{ value: "", label: "Guess from the fixture type" },
						...groups.flatMap(({ group, bodies }) =>
							bodies.map((body) => ({
								value: body.id,
								label: `${group} · ${body.label}`,
							})),
						),
					]}
					onChange={(body_model) =>
						onChange((current) => ({
							...current,
							body_model: body_model === "" ? null : body_model,
						}))
					}
				/>
			</FormLayout>
		</section>
	);
}

/** Who this fixture is: what it is called, and what it looks like in the library. */
export function IdentityProfileTab({
	draft,
	onChange,
	onLookup,
}: GenericSectionProps & { onLookup: () => void }) {
	return (
		<div className="fixture-generic-tab">
			<IdentitySection draft={draft} onChange={onChange} onLookup={onLookup} />
			<NotesAssetsSection draft={draft} onChange={onChange} />
		</div>
	);
}

/**
 * What the fixture is made of and what its light does — the two things the Stage needs to draw it.
 *
 * Physical is how the lantern is built; Optics is what comes out of it. Colour temperature,
 * luminous output, and beam angle are the light, not the lantern, so they belong on the right.
 */
export function SimulationProfileTab({
	draft,
	onChange,
	bodyCatalogue = [],
}: GenericSectionProps & { bodyCatalogue?: FixtureBodyModel[] }) {
	return (
		<div className="fixture-generic-tab">
			<BodySection
				draft={draft}
				onChange={onChange}
				bodyCatalogue={bodyCatalogue}
			/>
			<PhysicalSection draft={draft} onChange={onChange} />
			<OpticsSection draft={draft} onChange={onChange} />
		</div>
	);
}
