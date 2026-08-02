import {
	Button,
	ColorPickerField,
	ModalRegistration,
	ModalTitleBar,
	Select,
	TextField,
} from "@tosklight/ui";
import { useState } from "react";
import type {
	GelAssignment,
	InstalledFixtureAppearance,
	InstalledLightSource,
	MultiPatchInstance,
	PatchedFixture,
} from "../../../api/types";
import type {
	PatchGelAssignment,
	PatchInstalledFixtureAppearance,
} from "../../../features/patch/contracts";
import { type PatchController, usePatchController } from "./controller";
import { fixtureDisplayId } from "./fixtureIds";

const SOURCE_OPTIONS: ReadonlyArray<{
	value: InstalledLightSource["type"];
	label: string;
}> = [
	{ value: "profile_default", label: "Profile default" },
	{ value: "tungsten", label: "Tungsten" },
	{ value: "halogen", label: "Halogen" },
	{ value: "discharge", label: "Discharge" },
	{ value: "led", label: "LED" },
	{ value: "fluorescent", label: "Fluorescent" },
	{ value: "arc", label: "Arc" },
	{ value: "other", label: "Other" },
];

const DEFAULT_APPEARANCE: InstalledFixtureAppearance = {
	light_source: { type: "profile_default" },
	color_temperature_kelvin: null,
	gel: { type: "open_white" },
	shaper_angles_degrees: [0, 0, 0, 0],
};

type AppearanceDraft = {
	sourceType: InstalledLightSource["type"];
	otherLabel: string;
	colorTemperature: string;
	gelType: GelAssignment["type"];
	customName: string;
	customColor: string;
	customNote: string;
};

export function LightSourceCell({
	fixture,
	instance,
}: {
	fixture: PatchedFixture;
	instance?: MultiPatchInstance;
}) {
	const controller = usePatchController();
	const appearance = instance
		? instance.installed_appearance
		: fixture.installed_appearance;
	const current = appearance ?? DEFAULT_APPEARANCE;
	const available = hasGeometryEmitter(fixture);
	if (!available)
		return (
			<td className="patch-stacked-cell">
				<span className="patch-stacked-line">Unavailable</span>
				<span className="patch-stacked-line patch-secondary">
					No geometry emitter
				</span>
			</td>
		);

	const source = sourceSummary(fixture, current);
	const gel = gelSummary(current.gel);
	const target = instance?.name || fixtureDisplayId(fixture);
	return (
		<td className="patch-stacked-cell">
			<Button
				className="patch-value patch-stacked-editor"
				aria-label={`Light source ${target}: ${source}; ${gel}`}
				onClick={() =>
					beginLightSourceEdit(controller, fixture, instance?.id ?? null)
				}
			>
				<span className="patch-stacked-line" title={source}>
					{source}
				</span>
				<span className="patch-stacked-line patch-secondary" title={gel}>
					{gel}
				</span>
			</Button>
		</td>
	);
}

export function LightSourceAppearanceDialog() {
	const controller = usePatchController();
	const target = controller.ui.appearanceEdit;
	if (!target) return null;
	const fixture = controller.data.all.find(
		(candidate) => candidate.fixture_id === target.fixtureId,
	);
	const instance = target.multipatchInstanceId
		? fixture?.multipatch?.find(
				(candidate) => candidate.id === target.multipatchInstanceId,
			)
		: undefined;
	if (!fixture || (target.multipatchInstanceId && !instance)) return null;
	const identity = instance?.name || fixtureDisplayId(fixture);
	const appearance =
		(instance ? instance.installed_appearance : fixture.installed_appearance) ??
		DEFAULT_APPEARANCE;
	return (
		<AppearanceEditor
			key={`${fixture.fixture_id}:${target.multipatchInstanceId ?? "primary"}`}
			controller={controller}
			fixture={fixture}
			instance={instance}
			identity={identity}
			appearance={appearance}
		/>
	);
}

function AppearanceEditor({
	controller,
	fixture,
	instance,
	identity,
	appearance,
}: {
	controller: PatchController;
	fixture: PatchedFixture;
	instance?: MultiPatchInstance;
	identity: string | number;
	appearance: InstalledFixtureAppearance;
}) {
	const [draft, setDraft] = useState(() => appearanceDraft(appearance));
	const [submitError, setSubmitError] = useState("");
	const [busy, setBusy] = useState(false);
	const result = normalizeAppearanceDraft(
		draft,
		appearance,
		profileCct(fixture),
	);
	const baseline = toPatchAppearance(appearance);
	const changed =
		result.appearance !== undefined &&
		JSON.stringify(result.appearance) !== JSON.stringify(baseline);
	const close = () => closeLightSourceEdit(controller);
	const apply = async () => {
		if (!result.appearance || !changed || busy) return;
		setBusy(true);
		setSubmitError("");
		const applied = await controller.patch.updateFixtureIntent(
			fixture.fixture_id,
			instance?.id ?? null,
			{
				type: "set_installed_appearance",
				appearance: result.appearance,
			},
		);
		setBusy(false);
		if (applied) close();
		else setSubmitError("The installed appearance could not be applied.");
	};
	const assignedBuiltIn =
		appearance.gel.type === "built_in" ? appearance.gel : null;

	return (
		<ModalRegistration onClose={close}>
			<div className="stacked-modal-layer">
				<section
					className="nested-modal patch-edit-modal light-source-editor"
					role="dialog"
					aria-modal="true"
					aria-label={`Set light source ${identity}`}
				>
					<ModalTitleBar
						title={`Set light source ${identity}`}
						actions={
							<Button
								className="primary"
								disabled={!changed || Boolean(result.error) || busy}
								onClick={() => void apply()}
							>
								Apply
							</Button>
						}
						closeLabel="Close light source editor"
						onClose={close}
					/>
					<div className="light-source-editor-grid">
						{/* biome-ignore lint/a11y/noLabelWithoutControl: Select renders its native control inside this label. */}
						<label>
							Light source
							<Select
								autoFocus
								aria-label="Installed light source"
								value={draft.sourceType}
								onChange={(event) =>
									setDraft({
										...draft,
										sourceType: event.target
											.value as InstalledLightSource["type"],
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
								onChange={(event) =>
									setDraft({ ...draft, otherLabel: event.target.value })
								}
							/>
						)}
						<TextField
							label="Color temperature (K)"
							description={profileTemperatureDescription(fixture)}
							inputMode="numeric"
							value={draft.colorTemperature}
							onChange={(event) =>
								setDraft({ ...draft, colorTemperature: event.target.value })
							}
						/>
						{/* biome-ignore lint/a11y/noLabelWithoutControl: Select renders its native control inside this label. */}
						<label>
							Gel / filter
							<Select
								aria-label="Installed gel or filter"
								value={draft.gelType}
								onChange={(event) =>
									setDraft({
										...draft,
										gelType: event.target.value as GelAssignment["type"],
									})
								}
							>
								<option value="open_white">Open white</option>
								{assignedBuiltIn && (
									<option value="built_in">
										Assigned catalog gel · {gelSummary(assignedBuiltIn)}
									</option>
								)}
								<option value="custom">Custom color</option>
								{!assignedBuiltIn && (
									<option disabled value="built_in">
										Catalog gels unavailable pending review
									</option>
								)}
							</Select>
						</label>
						{draft.gelType === "custom" && (
							<>
								<TextField
									label="Custom gel name"
									value={draft.customName}
									onChange={(event) =>
										setDraft({ ...draft, customName: event.target.value })
									}
								/>
								<ColorPickerField
									label="Custom gel color"
									value={draft.customColor}
									onChange={(value) =>
										setDraft({ ...draft, customColor: value })
									}
								/>
								<TextField
									label="Custom gel note (optional)"
									value={draft.customNote}
									onChange={(event) =>
										setDraft({ ...draft, customNote: event.target.value })
									}
								/>
							</>
						)}
					</div>
					<p className="patch-secondary" data-catalog-status="unavailable">
						Catalog gel search and CSV import remain unavailable until the
						manufacturer-neutral catalog review is complete.
					</p>
					{(result.error || submitError) && (
						<p className="patch-status" role="alert">
							{result.error || submitError}
						</p>
					)}
				</section>
			</div>
		</ModalRegistration>
	);
}

function beginLightSourceEdit(
	controller: PatchController,
	fixture: PatchedFixture,
	multipatchInstanceId: string | null,
) {
	if (!controller.appState.patchSetArmed || !hasGeometryEmitter(fixture))
		return;
	controller.ui.setSelectedFixture(fixture.fixture_id);
	controller.patch.selectPatchInstance({
		fixtureId: fixture.fixture_id,
		multipatchInstanceId,
	});
	controller.ui.setAppearanceEdit({
		fixtureId: fixture.fixture_id,
		multipatchInstanceId,
	});
}

function closeLightSourceEdit(controller: PatchController) {
	controller.ui.setAppearanceEdit(null);
	controller.dispatch({ type: "SET_PATCH_ARMED", value: false });
}

export function hasGeometryEmitter(fixture: PatchedFixture) {
	const snapshot = fixture.definition.profile_snapshot;
	const modeId = fixture.definition.mode_id;
	return Boolean(
		snapshot &&
			modeId &&
			snapshot.modes.find((mode) => mode.id === modeId)?.geometry.emitters
				.length,
	);
}

function appearanceDraft(
	appearance: InstalledFixtureAppearance,
): AppearanceDraft {
	return {
		sourceType: appearance.light_source.type,
		otherLabel:
			appearance.light_source.type === "other"
				? appearance.light_source.label
				: "",
		colorTemperature:
			appearance.color_temperature_kelvin == null
				? ""
				: String(appearance.color_temperature_kelvin),
		gelType: appearance.gel.type,
		customName:
			appearance.gel.type === "custom" ? appearance.gel.name : "Custom",
		customColor:
			appearance.gel.type === "custom" ? appearance.gel.color_srgb : "#FFFFFF",
		customNote:
			appearance.gel.type === "custom" ? (appearance.gel.note ?? "") : "",
	};
}

function normalizeAppearanceDraft(
	draft: AppearanceDraft,
	baseline: InstalledFixtureAppearance,
	inheritedCct: number | null,
): { appearance?: PatchInstalledFixtureAppearance; error?: string } {
	let colorTemperatureKelvin: number | null = null;
	const temperature = draft.colorTemperature.trim();
	if (temperature) {
		if (!/^\d+$/.test(temperature))
			return { error: "Color temperature must be a whole number of kelvin." };
		colorTemperatureKelvin = Number(temperature);
		if (
			!Number.isSafeInteger(colorTemperatureKelvin) ||
			colorTemperatureKelvin < 1_000 ||
			colorTemperatureKelvin > 25_000
		)
			return { error: "Color temperature must be from 1,000 K to 25,000 K." };
	}

	let lightSource: PatchInstalledFixtureAppearance["lightSource"];
	if (draft.sourceType === "other") {
		const label = draft.otherLabel.trim();
		const error = boundedTextError(label, "Other light-source name", 256);
		if (error) return { error };
		lightSource = { type: "other", label };
	} else lightSource = { type: draft.sourceType };
	if (
		draft.sourceType !== "profile_default" &&
		colorTemperatureKelvin === null &&
		inheritedCct === null
	)
		return {
			error:
				"An explicit light source requires a color temperature because this profile has no default CCT.",
		};

	let gel: PatchGelAssignment;
	if (draft.gelType === "open_white") gel = { type: "open_white" };
	else if (draft.gelType === "built_in") {
		if (baseline.gel.type !== "built_in")
			return { error: "The selected catalog gel is unavailable on this desk." };
		gel = {
			type: "built_in",
			catalogId: baseline.gel.catalog_id,
			entryId: baseline.gel.entry_id,
			embeddedFallback: {
				number: baseline.gel.embedded_fallback.number,
				name: baseline.gel.embedded_fallback.name,
				displaySrgb: baseline.gel.embedded_fallback.display_srgb,
				visualizerSrgb: baseline.gel.embedded_fallback.visualizer_srgb,
			},
		};
	} else {
		const name = draft.customName.trim();
		const nameError = boundedTextError(name, "Custom gel name", 256);
		if (nameError) return { error: nameError };
		const colorSrgb = draft.customColor.trim().toUpperCase();
		if (!/^#[0-9A-F]{6}$/.test(colorSrgb))
			return { error: "Custom gel color must be a #RRGGBB value." };
		const note = draft.customNote.trim();
		if (utf8Length(note) > 1_024)
			return { error: "Custom gel note must contain at most 1,024 bytes." };
		gel = { type: "custom", name, colorSrgb, note: note || null };
	}

	return {
		appearance: {
			lightSource,
			colorTemperatureKelvin,
			gel,
			shaperAnglesDegrees: [...baseline.shaper_angles_degrees] as [
				number,
				number,
				number,
				number,
			],
		},
	};
}

function toPatchAppearance(
	appearance: InstalledFixtureAppearance,
): PatchInstalledFixtureAppearance {
	const lightSource =
		appearance.light_source.type === "other"
			? { type: "other" as const, label: appearance.light_source.label }
			: { type: appearance.light_source.type };
	let gel: PatchGelAssignment;
	if (appearance.gel.type === "open_white") gel = { type: "open_white" };
	else if (appearance.gel.type === "custom")
		gel = {
			type: "custom",
			name: appearance.gel.name,
			colorSrgb: appearance.gel.color_srgb,
			note: appearance.gel.note,
		};
	else
		gel = {
			type: "built_in",
			catalogId: appearance.gel.catalog_id,
			entryId: appearance.gel.entry_id,
			embeddedFallback: {
				number: appearance.gel.embedded_fallback.number,
				name: appearance.gel.embedded_fallback.name,
				displaySrgb: appearance.gel.embedded_fallback.display_srgb,
				visualizerSrgb: appearance.gel.embedded_fallback.visualizer_srgb,
			},
		};
	return {
		lightSource,
		colorTemperatureKelvin: appearance.color_temperature_kelvin,
		gel,
		shaperAnglesDegrees: [...appearance.shaper_angles_degrees] as [
			number,
			number,
			number,
			number,
		],
	};
}

function sourceSummary(
	fixture: PatchedFixture,
	appearance: InstalledFixtureAppearance,
) {
	const option = SOURCE_OPTIONS.find(
		(candidate) => candidate.value === appearance.light_source.type,
	);
	const source =
		appearance.light_source.type === "other"
			? `Other (${appearance.light_source.label})`
			: (option?.label ?? "Profile default");
	const effectiveCct =
		appearance.color_temperature_kelvin ?? profileCct(fixture);
	return effectiveCct === null
		? source
		: `${source} · ${effectiveCct.toLocaleString("en-US")} K`;
}

function gelSummary(gel: GelAssignment) {
	if (gel.type === "open_white") return "Open white";
	if (gel.type === "custom") return gel.name;
	return `${gel.embedded_fallback.number} · ${gel.embedded_fallback.name}`;
}

function profileCct(fixture: PatchedFixture) {
	return (
		fixture.definition.profile_snapshot?.physical.color_temperature_kelvin ??
		null
	);
}

function profileTemperatureDescription(fixture: PatchedFixture) {
	const cct = profileCct(fixture);
	return cct === null
		? "No profile CCT is available. Explicit sources require a value."
		: `Leave empty to inherit ${cct.toLocaleString("en-US")} K from the embedded profile revision.`;
}

function boundedTextError(value: string, label: string, limit: number) {
	if (!value) return `${label} is required.`;
	if (utf8Length(value) > limit)
		return `${label} must contain at most ${limit} bytes.`;
	return null;
}

function utf8Length(value: string) {
	return new TextEncoder().encode(value).length;
}
