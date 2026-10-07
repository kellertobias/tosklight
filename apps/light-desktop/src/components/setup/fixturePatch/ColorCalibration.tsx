import {
	colorCalibrationError,
	type InstalledColorCalibration,
	type InstalledColorPathCalibration,
	type NativeColorIdentity,
} from "@tosklight/patch";
import type {
	FixtureMode,
	HeadOpticalPath,
	OpticalProvenance,
} from "@tosklight/patch/fixture-profile";
import { nativeColorFunctionAllowed } from "@tosklight/patch/fixture-profile-model";
import {
	Button,
	FormLayout,
	ModalRegistration,
	ModalTitleBar,
	NumberField,
	SelectField,
	TextField,
} from "@tosklight/ui";
import { useState } from "react";
import { usePatchController } from "./controller";
import { fixtureDisplayId } from "./fixtureIds";

const unknown = (): OpticalProvenance => ({ quality: "unknown", revision: 0 });
const sameIdentity = (a: NativeColorIdentity, b: NativeColorIdentity) =>
	Object.keys(a).every(
		(key) =>
			a[key as keyof NativeColorIdentity] ===
			b[key as keyof NativeColorIdentity],
	);

export function ColorCalibrationDialog() {
	const controller = usePatchController();
	const target = controller.ui.colorCalibrationEdit;
	if (!target) return null;
	const fixture = controller.data.all.find(
		(f) => f.fixture_id === target.fixtureId,
	);
	const physical = target.multipatchInstanceId
		? fixture?.multipatch?.find((c) => c.id === target.multipatchInstanceId)
		: fixture;
	if (!fixture || !physical) return null;
	const context = fixture.definition.color_calibration_context;
	return (
		<ColorCalibrationEditor
			key={`${target.fixtureId}:${target.multipatchInstanceId ?? "root"}`}
			identity={String(
				target.multipatchInstanceId
					? physical.name || "Multi-patch"
					: fixtureDisplayId(fixture),
			)}
			initial={physical.color_calibration ?? null}
			mode={context?.mode ?? null}
			identities={context?.identities ?? []}
			onClose={() => controller.ui.setColorCalibrationEdit(null)}
			onSave={(calibration) =>
				controller.patch.updateFixtureIntent(
					target.fixtureId,
					target.multipatchInstanceId,
					{ type: "set_color_calibration", calibration },
				)
			}
		/>
	);
}

interface ColorCalibrationEditorProps {
	identity: string;
	initial: InstalledColorCalibration | null;
	mode: FixtureMode | null;
	identities: readonly NativeColorIdentity[];
	onClose(): void;
	onSave(value: InstalledColorCalibration | null): Promise<boolean>;
}

/** Draft, validity and save lifecycle of one lamp's Color calibration editor. */
function useColorCalibrationDraft({
	initial,
	identities,
	onClose,
	onSave,
}: Pick<
	ColorCalibrationEditorProps,
	"initial" | "identities" | "onClose" | "onSave"
>) {
	const [draft, setDraft] = useState<InstalledColorCalibration | null>(() =>
		initial ? structuredClone(initial) : null,
	);
	const [busy, setBusy] = useState(false),
		[saveError, setSaveError] = useState("");
	const stale = Boolean(
		draft?.paths.some(
			(p) => !identities.some((id) => sameIdentity(p.source_identity, id)),
		),
	);
	const error = colorCalibrationError(draft);
	const changed = JSON.stringify(draft) !== JSON.stringify(initial);
	const close = () => {
		if (!busy) onClose();
	};
	const updatePath = (value: InstalledColorPathCalibration) => {
		setSaveError("");
		const paths = [
			...(draft?.paths.filter(
				(p) => p.source_identity.path_id !== value.source_identity.path_id,
			) ?? []),
			value,
		].filter((p) => p.emitters.length || p.measurements.length);
		setDraft(
			paths.length
				? { version: 1, revision: draft?.revision ?? 0, paths }
				: null,
		);
	};
	const save = async () => {
		if (busy || error || stale || !changed) return;
		setBusy(true);
		setSaveError("");
		try {
			if (await onSave(draft)) onClose();
			else
				setSaveError(
					"The Color calibration could not be saved. The fixture may have changed; review its current profile.",
				);
		} catch (reason) {
			setSaveError(
				reason instanceof Error
					? reason.message
					: "The Color calibration could not be saved.",
			);
		} finally {
			setBusy(false);
		}
	};
	return {
		draft,
		setDraft,
		busy,
		saveError,
		setSaveError,
		stale,
		error,
		changed,
		close,
		updatePath,
		save,
	};
}

export function ColorCalibrationEditor({
	identity,
	initial,
	mode,
	identities,
	onClose,
	onSave,
}: ColorCalibrationEditorProps) {
	const {
		draft,
		setDraft,
		busy,
		saveError,
		setSaveError,
		stale,
		error,
		changed,
		close,
		updatePath,
		save,
	} = useColorCalibrationDraft({ initial, identities, onClose, onSave });
	return (
		<ModalRegistration onClose={close}>
			<div className="stacked-modal-layer">
				<section
					className="nested-modal patch-edit-modal"
					role="dialog"
					aria-modal="true"
					aria-label={`Color calibration ${identity}`}
				>
					<ModalTitleBar
						title={`Color calibration ${identity}`}
						accept={{
							id: "save",
							label: "Save",
							variant: "primary",
							disabled: busy || !!error || stale || !changed,
							onPress: () => void save(),
						}}
						closeLabel="Close Color calibration"
						onClose={close}
					/>
					<p className="field-hint">
						Observations belong to this physical lamp only. Copies have
						independent calibration. These saved values do not yet change live
						output or Stage.
					</p>
					{stale && (
						<p role="status">
							Saved calibration belongs to a different profile or optical path
							and is inactive. Its original source is retained. Clear it before
							authoring observations for the current lamp.
						</p>
					)}
					{!mode?.color_physical && (
						<p role="status">
							This profile has no authored optical paths. Configure its emitters
							and filters in the Fixture Library before adding calibration.
						</p>
					)}
					<fieldset disabled={busy || stale}>
						<NumberField
							label="Color calibration revision"
							min={0}
							max={0xffff_ffff}
							value={draft?.revision ?? 0}
							disabled={!draft}
							onChange={(event) => {
								if (draft)
									setDraft({ ...draft, revision: Number(event.target.value) });
							}}
						/>
						<ColorCalibrationPaths
							mode={mode}
							identities={identities}
							draft={draft}
							onChange={updatePath}
						/>
					</fieldset>
					{stale && <RetainedCalibrationSource draft={draft} />}
					<Button
						disabled={busy || draft === null}
						onClick={() => {
							setDraft(null);
							setSaveError("");
						}}
					>
						Clear Color calibration
					</Button>
					<p className="field-hint">
						Replacing hardware with the same profile also requires reviewing its
						calibration. A gain changes emitter output, not its hue; use
						whole-path observations to describe measured colors.
					</p>
					{(error || saveError) && <p role="alert">{error || saveError}</p>}
				</section>
			</div>
		</ModalRegistration>
	);
}

/** One editor per authored optical path, seeded from the current source identity. */
function ColorCalibrationPaths({
	mode,
	identities,
	draft,
	onChange,
}: {
	mode: FixtureMode | null;
	identities: readonly NativeColorIdentity[];
	draft: InstalledColorCalibration | null;
	onChange(value: InstalledColorPathCalibration): void;
}) {
	return (
		<>
			{mode?.color_physical?.paths.map((path) => {
				const source = identities.find((id) => id.path_id === path.id);
				if (!source)
					return (
						<p key={path.id}>
							Current source identity is unavailable for this optical path.
						</p>
					);
				const value = draft?.paths.find(
					(p) => p.source_identity.path_id === path.id,
				) ?? { source_identity: source, emitters: [], measurements: [] };
				return (
					<ColorPathEditor
						key={path.id}
						mode={mode}
						path={path}
						value={value}
						onChange={onChange}
					/>
				);
			})}
		</>
	);
}

function RetainedCalibrationSource({
	draft,
}: {
	draft: InstalledColorCalibration | null;
}) {
	return (
		<details>
			<summary>Retained calibration source</summary>
			{draft?.paths.map((p) => (
				<p key={p.source_identity.path_id}>
					Profile {p.source_identity.profile_id}, revision{" "}
					{p.source_identity.profile_revision}; {p.emitters.length} gain
					corrections and {p.measurements.length} observations.
				</p>
			))}
		</details>
	);
}

function Evidence({
	label,
	value,
	onChange,
}: {
	label: string;
	value: OpticalProvenance;
	onChange(value: OpticalProvenance): void;
}) {
	return (
		<FormLayout columns={3} minColumnWidth={140}>
			<SelectField
				label={`${label} quality`}
				ariaLabel={`${label} quality`}
				value={value.quality}
				options={[
					{ value: "unknown", label: "Unknown" },
					{ value: "estimated", label: "Estimated" },
					{ value: "manufacturer", label: "Manufacturer" },
					{ value: "measured", label: "Measured" },
				]}
				onChange={(quality) =>
					onChange({
						...value,
						quality: quality as OpticalProvenance["quality"],
					})
				}
			/>
			<TextField
				label={`${label} source`}
				value={value.source ?? ""}
				onChange={(event) =>
					onChange({ ...value, source: event.target.value || null })
				}
			/>
			<NumberField
				label={`${label} evidence revision`}
				min={0}
				max={0xffff_ffff}
				value={value.revision}
				onChange={(event) =>
					onChange({ ...value, revision: Number(event.target.value) })
				}
			/>
		</FormLayout>
	);
}

type NativeRecipe =
	InstalledColorPathCalibration["measurements"][number]["recipe"];

/** The editing template for a new observation: each control's default native value. */
function nativeRecipeTemplate(
	mode: FixtureMode,
	path: HeadOpticalPath,
): NativeRecipe {
	return path.controls.flatMap((id) => {
		const channel = mode.channels.find((c) => c.id === id);
		const functions =
			channel?.functions.filter((f) =>
				nativeColorFunctionAllowed(channel, f),
			) ?? [];
		const fn =
			functions.find(
				(f) =>
					channel &&
					channel.default_raw >= f.dmx_from &&
					channel.default_raw <= f.dmx_to,
			) ?? functions[0];
		return fn && channel
			? [
					{
						channel_id: id,
						function_id: fn.id,
						raw:
							channel.default_raw >= fn.dmx_from &&
							channel.default_raw <= fn.dmx_to
								? channel.default_raw
								: fn.dmx_from,
					},
				]
			: [];
	});
}

function ColorPathEditor({
	mode,
	path,
	value,
	onChange,
}: {
	mode: FixtureMode;
	path: HeadOpticalPath;
	value: InstalledColorPathCalibration;
	onChange(value: InstalledColorPathCalibration): void;
}) {
	const head = mode.heads.find((h) => h.id === path.head_id)?.name ?? "Head";
	const emitters = path.source.type === "additive" ? path.source.emitters : [];
	const nextEmitter = emitters.find(
		(e) => !value.emitters.some((c) => c.emitter_id === e.id),
	);
	const recipeTemplate = nativeRecipeTemplate(mode, path);
	return (
		<fieldset>
			<legend>{head}</legend>
			{value.emitters.map((gain, index) => (
				<EmitterGainEditor
					key={gain.emitter_id}
					label={`${head} ${emitters.find((e) => e.id === gain.emitter_id)?.name ?? "Unknown emitter"}`}
					value={value}
					index={index}
					onChange={onChange}
				/>
			))}
			{!!emitters.length && (
				<Button
					disabled={!nextEmitter}
					onClick={() => {
						if (nextEmitter)
							onChange({
								...value,
								emitters: [
									...value.emitters,
									{
										emitter_id: nextEmitter.id,
										output_gain: 1,
										provenance: unknown(),
									},
								],
							});
					}}
				>
					Add {head} emitter gain
				</Button>
			)}
			<p className="field-hint">
				1.0 keeps the profile output; 0.0 records no output. Gains scale XYZ and
				spectra together, and leave unknown emitter color unknown.
			</p>
			{value.measurements.map((_, index) => (
				<ObservationEditor
					key={index}
					label={`${head} observation ${index + 1}`}
					mode={mode}
					value={value}
					index={index}
					onChange={onChange}
				/>
			))}
			<Button
				disabled={recipeTemplate.length !== path.controls.length}
				onClick={() =>
					onChange({
						...value,
						measurements: [
							...value.measurements,
							{
								recipe: recipeTemplate,
								xyz: { x: 0, y: 0, z: 0 },
								provenance: unknown(),
							},
						],
					})
				}
			>
				Add {head} whole-path observation
			</Button>
		</fieldset>
	);
}

interface PathEntryEditorProps {
	label: string;
	value: InstalledColorPathCalibration;
	index: number;
	onChange(value: InstalledColorPathCalibration): void;
}

function EmitterGainEditor({
	label,
	value,
	index,
	onChange,
}: PathEntryEditorProps) {
	const gain = value.emitters[index];
	const set = (change: Partial<typeof gain>) =>
		onChange({
			...value,
			emitters: value.emitters.map((g, i) =>
				i === index ? { ...g, ...change } : g,
			),
		});
	return (
		<div>
			<NumberField
				label={`${label} output gain`}
				allowDecimal
				step={0.01}
				min={0}
				value={gain.output_gain}
				onChange={(event) => set({ output_gain: Number(event.target.value) })}
			/>
			<Evidence
				label={label}
				value={gain.provenance}
				onChange={(provenance) => set({ provenance })}
			/>
			<Button
				onClick={() =>
					onChange({
						...value,
						emitters: value.emitters.filter(
							(g) => g.emitter_id !== gain.emitter_id,
						),
					})
				}
			>
				Remove {label} gain
			</Button>
		</div>
	);
}

function ObservationEditor({
	label,
	mode,
	value,
	index,
	onChange,
}: PathEntryEditorProps & { mode: FixtureMode }) {
	const measurement = value.measurements[index];
	const set = (change: Partial<typeof measurement>) =>
		onChange({
			...value,
			measurements: value.measurements.map((m, i) =>
				i === index ? { ...m, ...change } : m,
			),
		});
	return (
		<fieldset>
			<legend>{label}</legend>
			<FormLayout columns={3} minColumnWidth={100}>
				{(["x", "y", "z"] as const).map((axis) => (
					<NumberField
						key={axis}
						label={`${label} ${axis.toUpperCase()}`}
						allowDecimal
						min={0}
						step={0.001}
						value={measurement.xyz[axis]}
						onChange={(event) =>
							set({
								xyz: {
									...measurement.xyz,
									[axis]: Number(event.target.value),
								},
							})
						}
					/>
				))}
			</FormLayout>
			<Evidence
				label={label}
				value={measurement.provenance}
				onChange={(provenance) => set({ provenance })}
			/>
			<details>
				<summary>{label} complete native recipe</summary>
				<p className="field-hint">
					Enter the exact native values used for this observation, including
					parked wheels. Defaults are an editing template; nothing is sent to a
					lamp.
				</p>
				{measurement.recipe.map((native, channelIndex) => (
					<NativeRecipeField
						key={native.channel_id}
						label={label}
						mode={mode}
						recipe={measurement.recipe}
						index={channelIndex}
						onChange={(recipe) => set({ recipe })}
					/>
				))}
			</details>
			<Button
				onClick={() =>
					onChange({
						...value,
						measurements: value.measurements.filter((_, i) => i !== index),
					})
				}
			>
				Remove {label}
			</Button>
		</fieldset>
	);
}

function NativeRecipeField({
	label,
	mode,
	recipe,
	index,
	onChange,
}: {
	label: string;
	mode: FixtureMode;
	recipe: NativeRecipe;
	index: number;
	onChange(recipe: NativeRecipe): void;
}) {
	const native = recipe[index];
	const channel = mode.channels.find((c) => c.id === native.channel_id);
	if (!channel) return <p>Missing native control</p>;
	const functions = channel.functions.filter((f) =>
		nativeColorFunctionAllowed(channel, f),
	);
	const fn = functions.find((f) => f.id === native.function_id);
	const setNative = (change: Partial<typeof native>) =>
		onChange(recipe.map((v, i) => (i === index ? { ...v, ...change } : v)));
	return (
		<FormLayout columns={2} minColumnWidth={160}>
			<SelectField
				label={`${label} ${channel.fixture_attribute} function`}
				ariaLabel={`${label} ${channel.fixture_attribute} function`}
				value={native.function_id}
				options={functions.map((f) => ({
					value: f.id,
					label: `${f.name} · ${f.dmx_from}–${f.dmx_to}`,
				}))}
				onChange={(id) => {
					const selected = functions.find((f) => f.id === id);
					if (selected)
						setNative({
							function_id: id,
							raw: selected.dmx_from,
						});
				}}
			/>
			<NumberField
				label={`${label} ${channel.fixture_attribute} raw`}
				min={fn?.dmx_from ?? 0}
				max={fn?.dmx_to ?? 0xffff_ffff}
				value={native.raw}
				onChange={(event) => setNative({ raw: Number(event.target.value) })}
			/>
		</FormLayout>
	);
}
