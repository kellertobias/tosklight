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
	const manufacturers = useMemo(
		() =>
			[...new Set(availableDefinitions.map((item) => item.manufacturer))]
				.filter(Boolean)
				.sort(compareFixtureManufacturers),
		[availableDefinitions],
	);
	const libraryFamilies = useMemo(() => {
		const needle = query.toLowerCase().trim();
		return groupFixtureFamilies(
			availableDefinitions.filter(
				(item) =>
					(!manufacturer || item.manufacturer === manufacturer) &&
					(!typeFilter || item.device_type === typeFilter) &&
					(!needle ||
						`${item.manufacturer} ${item.name} ${item.model} ${item.mode} ${item.device_type}`
							.toLowerCase()
							.includes(needle)),
			),
		);
	}, [availableDefinitions, query, manufacturer, typeFilter]);
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
