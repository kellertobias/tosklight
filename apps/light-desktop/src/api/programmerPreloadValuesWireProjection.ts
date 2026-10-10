import {
	arrayAt,
	exactRecordAt,
	integerAt,
	printableStringAt,
} from "./playbackWirePrimitives";
import type { ProgrammerPreloadValuesProjection } from "../features/programmerPreloadValues/contracts";
import {
	decodeProgrammerValuesProjection,
	programmerValuesUuidAt,
} from "./programmerValuesWireProjection";

/** The two value authorities deliberately share one strict value-shape decoder. */
export function decodeProgrammerPreloadValuesProjection(
	value: unknown,
	path: string,
): ProgrammerPreloadValuesProjection {
	const projection = exactRecordAt(value, path, [
		"revision",
		"fixture_values",
		"group_values",
		"dynamic_values",
		"dynamic_definitions",
		"group_release_values",
	]);
	const { group_release_values, ...normal } = projection;
	const decoded = decodeProgrammerValuesProjection(normal, path);
	const groupReleaseValues = (
		group_release_values === undefined
			? []
			: arrayAt(group_release_values, `${path}.group_release_values`)
	).map((value, index) => {
		const at = `${path}.group_release_values[${index}]`;
		const entry = exactRecordAt(value, at, [
			"group_id",
			"attribute",
			"programmer_order",
			"changed_at_millis",
		]);
		return {
			groupId: printableStringAt(entry.group_id, `${at}.group_id`, 128),
			attribute: printableStringAt(entry.attribute, `${at}.attribute`, 128),
			programmerOrder: integerAt(
				entry.programmer_order,
				`${at}.programmer_order`,
			),
			changedAtMillis: integerAt(
				entry.changed_at_millis,
				`${at}.changed_at_millis`,
			),
		};
	});
	return {
		...decoded,
		...(groupReleaseValues.length ? { groupReleaseValues } : {}),
	};
}

export const programmerPreloadValuesUuidAt = programmerValuesUuidAt;
