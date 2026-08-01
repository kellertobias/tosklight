import { useMemo } from "react";
import type { FixtureDefinition, FixtureProfile } from "../../../api/types";
import {
	fixtureDefinitionKey,
	mergeFixtureDefinitions,
} from "../fixtureProfileModel";
import {
	compareFixtureManufacturers,
	groupFixtureFamilies,
} from "../patchUtils";

export type FixtureLibraryFamily = ReturnType<
	typeof groupFixtureFamilies
>[number];

interface FixtureLibraryModelOptions {
	fixtureProfiles: FixtureProfile[];
	legacyDefinitions: FixtureDefinition[];
	manufacturer: string;
	query: string;
	selectedFamilyKey: string;
	selectedModeKey: string;
	typeFilter: string;
}

export function useFixtureLibraryModel({
	fixtureProfiles,
	legacyDefinitions,
	manufacturer,
	query,
	selectedFamilyKey,
	selectedModeKey,
	typeFilter,
}: FixtureLibraryModelOptions) {
	const availableDefinitions = useMemo(
		() => mergeFixtureDefinitions(fixtureProfiles, legacyDefinitions),
		[fixtureProfiles, legacyDefinitions],
	);
	const fixtureTypes = useMemo(
		() =>
			[
				...new Set(
					availableDefinitions.map((item) => item.device_type || "other"),
				),
			].sort(),
		[availableDefinitions],
	);
	const searchMatchedDefinitions = useMemo(() => {
		const needle = normalizeFixtureSearch(query);
		return availableDefinitions.filter(
			(item) =>
				(!typeFilter || item.device_type === typeFilter) &&
				(!needle ||
					normalizeFixtureSearch(
						`${item.manufacturer} ${item.name} ${item.model} ${item.mode} ${item.device_type}`,
					).includes(needle)),
		);
	}, [availableDefinitions, query, typeFilter]);
	const manufacturers = useMemo(
		() =>
			[...new Set(searchMatchedDefinitions.map((item) => item.manufacturer))]
				.filter(Boolean)
				.sort(compareFixtureManufacturers),
		[searchMatchedDefinitions],
	);
	const libraryFamilies = useMemo(() => {
		const resolvedManufacturer = manufacturers.includes(manufacturer)
			? manufacturer
			: "";
		return groupFixtureFamilies(
			searchMatchedDefinitions.filter(
				(item) =>
					!resolvedManufacturer || item.manufacturer === resolvedManufacturer,
			),
		);
	}, [manufacturer, manufacturers, searchMatchedDefinitions]);
	const selectedFamily =
		libraryFamilies.find((family) => family.key === selectedFamilyKey) ??
		libraryFamilies[0] ??
		null;
	const selectedMode =
		selectedFamily?.modes.find(
			(mode) => fixtureDefinitionKey(mode) === selectedModeKey,
		) ??
		selectedFamily?.modes[0] ??
		null;

	return {
		fixtureTypes,
		libraryFamilies,
		manufacturers,
		selectedFamily,
		selectedMode,
	};
}

export function normalizeFixtureSearch(value: string) {
	return value
		.toLowerCase()
		.replace(/\bfour\b/gu, "4")
		.replace(/[^a-z0-9]+/gu, "");
}
