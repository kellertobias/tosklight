import type {
	GelCatalog,
	GelCatalogImportPreview,
	GelCatalogImportTarget,
} from "../../../api/client/fixtures";
import type {
	GelAssignment,
	InstalledFixtureAppearance,
	InstalledLightSource,
	PatchedFixture,
} from "../../../api/types";
import type {
	PatchGelAssignment,
	PatchInstalledFixtureAppearance,
} from "../../../features/patch/contracts";

export const SOURCE_OPTIONS: ReadonlyArray<{
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

export const DEFAULT_APPEARANCE: InstalledFixtureAppearance = {
	light_source: { type: "profile_default" },
	color_temperature_kelvin: null,
	gel: { type: "open_white" },
	shaper_angles_degrees: [0, 0, 0, 0],
};

export type AppearanceDraft = {
	sourceType: InstalledLightSource["type"];
	otherLabel: string;
	colorTemperature: string;
	gelType: GelAssignment["type"];
	customName: string;
	customColor: string;
	customNote: string;
	catalogGel: Extract<GelAssignment, { type: "built_in" }> | null;
};

export function appearanceDraft(
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
		catalogGel: appearance.gel.type === "built_in" ? appearance.gel : null,
	};
}

export function normalizeAppearanceDraft(
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

	const gelResult = normalizeGel(draft);
	if ("error" in gelResult) return gelResult;
	return {
		appearance: {
			lightSource,
			colorTemperatureKelvin,
			gel: gelResult.gel,
			shaperAnglesDegrees: [...baseline.shaper_angles_degrees] as [
				number,
				number,
				number,
				number,
			],
		},
	};
}

function normalizeGel(
	draft: AppearanceDraft,
): { gel: PatchGelAssignment } | { error: string } {
	if (draft.gelType === "open_white") return { gel: { type: "open_white" } };
	if (draft.gelType === "built_in") {
		if (!draft.catalogGel)
			return { error: "Choose a gel from an installed catalog." };
		return {
			gel: {
				type: "built_in",
				catalogId: draft.catalogGel.catalog_id,
				entryId: draft.catalogGel.entry_id,
				embeddedFallback: {
					number: draft.catalogGel.embedded_fallback.number,
					name: draft.catalogGel.embedded_fallback.name,
					displaySrgb: draft.catalogGel.embedded_fallback.display_srgb,
					visualizerSrgb: draft.catalogGel.embedded_fallback.visualizer_srgb,
				},
			},
		};
	}
	const name = draft.customName.trim();
	const nameError = boundedTextError(name, "Custom gel name", 256);
	if (nameError) return { error: nameError };
	const colorSrgb = draft.customColor.trim().toUpperCase();
	if (!/^#[0-9A-F]{6}$/.test(colorSrgb))
		return { error: "Custom gel color must be a #RRGGBB value." };
	const note = draft.customNote.trim();
	if (utf8Length(note) > 1_024)
		return { error: "Custom gel note must contain at most 1,024 bytes." };
	return {
		gel: { type: "custom", name, colorSrgb, note: note || null },
	};
}

export function toPatchAppearance(
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

export function sourceSummary(
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

export function gelSummary(gel: GelAssignment) {
	if (gel.type === "open_white") return "Open white";
	if (gel.type === "custom") return gel.name;
	return `${gel.embedded_fallback.number} · ${gel.embedded_fallback.name}`;
}

export function profileCct(fixture: PatchedFixture) {
	return (
		fixture.definition.profile_snapshot?.physical.color_temperature_kelvin ??
		null
	);
}

export function profileTemperatureDescription(fixture: PatchedFixture) {
	const cct = profileCct(fixture);
	return cct === null
		? "No profile CCT is available. Explicit sources require a value."
		: `Leave empty to inherit ${cct.toLocaleString("en-US")} K from the embedded profile revision.`;
}

export function gelImportTarget(
	selectedId: string,
	newCatalogId: string,
	catalogs: GelCatalog[],
): GelCatalogImportTarget | null {
	if (selectedId === "new") return { type: "create", catalog_id: newCatalogId };
	const catalog = catalogs.find((candidate) => candidate.id === selectedId);
	return catalog
		? {
				type: "update",
				catalog_id: catalog.id,
				expected_revision: catalog.revision,
			}
		: null;
}

export function gelImportConflict(
	conflict: GelCatalogImportPreview["conflicts"][number],
) {
	if (conflict.type === "catalog_identity_already_exists")
		return "A catalog with this identity already exists.";
	if (conflict.type === "catalog_missing")
		return "The catalog no longer exists.";
	return `Catalog revision changed from ${conflict.expected} to ${conflict.current}. Preview again.`;
}

export function mergeGelCatalogs(known: GelCatalog[], received: GelCatalog[]) {
	const byId = new Map(known.map((catalog) => [catalog.id, catalog]));
	for (const catalog of received) byId.set(catalog.id, catalog);
	return [...byId.values()].sort(
		(left, right) =>
			left.name.localeCompare(right.name, undefined, { sensitivity: "base" }) ||
			left.id.localeCompare(right.id),
	);
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
