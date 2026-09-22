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
	FixtureProfileMounting,
	FixtureProfileOptics,
} from "../wire";
import {
	OPTICS_PERCENT_PRECISION,
	opticsPercentMessage,
	PHYSICAL_PRECISION,
	percentFraction,
	percentText,
	precisionMessage,
} from "../sheet/fixtureProfileModel";
import { AssetField } from "./assets";
import { BodyPickerField } from "./bodyPicker";

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
				{PHYSICAL_PRECISION.map(({ key, label, unit, decimals }) => {
					const error = precisionMessage(
						label,
						unit,
						decimals,
						draft.physical[key],
					);
					return (
						<NumberField
							key={key}
							label={`${label} (${unit})`}
							// A point is always typeable so a decimal is named as wrong, not silently
							// dropped into a different whole number.
							allowDecimal
							inputMode={decimals === 0 ? "numeric" : "decimal"}
							step={decimals === 0 ? 1 : 0.01}
							min={0}
							value={draft.physical[key] ?? ""}
							error={error}
							aria-invalid={error ? true : undefined}
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
					);
				})}
			</FormLayout>
		</section>
	);
}

/** What a fixture can be hung by, in the words a rigger would use. */
const MOUNTING_HARDWARE = [
	{ value: "clamp", label: "Hook clamp over a pipe" },
	{ value: "yoke", label: "Yoke or bracket, bolted down" },
	{ value: "none", label: "Nothing — it stands or sits" },
] as const;

/** The clip a fixture with nothing declared starts from: a hand's width above its own top. */
const NEW_CLIP: FixtureProfileMounting = {
	hardware: "clamp",
	centre_millimetres: { x: 0, y: 0, z: 0 },
	half_extent_millimetres: { x: 0, y: 0, z: 0 },
	pipe_millimetres: { x: 0, y: 0, z: 0 },
	body_millimetres: { x: 0, y: 0, z: 0 },
};

/**
 * Where this fixture is held, so the plan can hang it on a pipe.
 *
 * The plan rigs a lamp by its clip: drag it near a truss and the pipe line of the clip lands on
 * the chord it reaches. A shipped lantern is authored from the hardware it really carries; a
 * model an operator imported is authored here, because only they know where its clamp is.
 *
 * Six figures say it, all in millimetres from the centre of the body — across, deep and up. The
 * pipe is where the bar ends up; the clip is how big the hardware around it is, which decides how
 * near a pipe has to come. The middle of the clip is taken to sit directly under the pipe, which
 * is how a hook clamp hangs, and the body the figures were measured against is the fixture's own
 * declared size, so a lamp drawn larger or smaller keeps its clamp in proportion.
 */
function MountingSection({ draft, onChange }: GenericSectionProps) {
	const clip = draft.mounting ?? NEW_CLIP;
	const body = draft.physical;
	const setClip = (change: Partial<FixtureProfileMounting>) =>
		onChange((current) => {
			const base = current.mounting ?? NEW_CLIP;
			const next = { ...base, ...change };
			// The clamp hangs under the pipe it closes around, so its middle follows the two.
			next.centre_millimetres = {
				x: next.pipe_millimetres.x,
				y: next.pipe_millimetres.y,
				z: next.pipe_millimetres.z - next.half_extent_millimetres.z,
			};
			// The figures are read against the body the profile declares, so a fixture drawn at
			// another size carries its clamp over in proportion.
			next.body_millimetres = {
				x: current.physical.width_millimetres ?? 0,
				y: current.physical.depth_millimetres ?? 0,
				z: current.physical.height_millimetres ?? 0,
			};
			return { ...current, mounting: next };
		});
	const vectorField = (
		label: string,
		key: "pipe_millimetres" | "half_extent_millimetres",
		axis: "x" | "y" | "z",
		scale = 1,
	) => (
		<NumberField
			key={`${key}-${axis}`}
			label={label}
			allowDecimal
			step={1}
			value={clip[key][axis] * scale}
			onChange={(event) =>
				setClip({
					[key]: {
						...clip[key],
						[axis]: (Number(event.target.value) || 0) / scale,
					},
				} as Partial<FixtureProfileMounting>)
			}
		/>
	);
	return (
		<section>
			<h3>Mounting</h3>
			<p className="field-hint">
				Where a pipe meets this fixture, measured in millimetres from the centre of its body:
				across, deep and up. Leave it on <b>Nothing</b> for a fixture that is never flown.
			</p>
			<FormLayout columns={5} minColumnWidth={145}>
				<SelectField
					label="Hangs by"
					value={clip.hardware}
					options={MOUNTING_HARDWARE.map((option) => ({ ...option }))}
					onChange={(hardware) => setClip({ hardware })}
				/>
				{clip.hardware === "none" ? null : (
					<>
						{vectorField("Pipe across (mm)", "pipe_millimetres", "x")}
						{vectorField("Pipe deep (mm)", "pipe_millimetres", "y")}
						{vectorField("Pipe up (mm)", "pipe_millimetres", "z")}
						{vectorField("Clip width (mm)", "half_extent_millimetres", "x", 2)}
						{vectorField("Clip depth (mm)", "half_extent_millimetres", "y", 2)}
						{vectorField("Clip height (mm)", "half_extent_millimetres", "z", 2)}
					</>
				)}
			</FormLayout>
			{clip.hardware === "none" || body.height_millimetres ? null : (
				<p className="field-hint">
					This fixture declares no height, so the plan draws it at whatever its family of
					bodies falls back to and the clip is taken as it stands. Fill in <b>Physical</b>
					above to have it carried over in proportion.
				</p>
			)}
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
				{OPTICS_PERCENT_PRECISION.map(({ key, label }) => {
					const error = opticsPercentMessage(label, optics[key]);
					return (
						<NumberField
							key={key}
							label={`${label} (%)`}
							allowDecimal
							inputMode="decimal"
							step={0.1}
							min={0}
							max={100}
							value={percentText(optics[key])}
							error={error}
							aria-invalid={error ? true : undefined}
							onChange={(event) => {
								const value = percentFraction(event.target.value);
								setOptics((current) => ({ ...current, [key]: value }));
							}}
						/>
					);
				})}
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
	return (
		<section>
			<h3>Body</h3>
			<p className="field-hint">
				{draft.model_asset
					? "This fixture ships its own 3D model, which is what the Stage draws."
					: "Leave this unset to keep the body guessed from the fixture type and its channels."}
			</p>
			<BodyPickerField
				value={draft.body_model ?? null}
				bodyCatalogue={bodyCatalogue}
				onChange={(body_model) =>
					onChange((current) => ({ ...current, body_model }))
				}
			/>
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
 * Physical is how the lantern is built, Mounting is what holds it up, and Optics is what comes
 * out of it. Colour temperature, luminous output, and beam angle are the light, not the lantern,
 * so they belong on the right.
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
			<MountingSection draft={draft} onChange={onChange} />
			<OpticsSection draft={draft} onChange={onChange} />
		</div>
	);
}
