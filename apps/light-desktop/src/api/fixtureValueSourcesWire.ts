import type { FixtureValueSourcesSnapshot } from "../features/dmxDiagnostics/fixtureValueSources";
import {
	arrayAt,
	booleanAt,
	numberAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { decodeAttributeValue } from "./programmerValuesWireProjection";

export function decodeFixtureValueSources(
	value: unknown,
): FixtureValueSourcesSnapshot {
	const row = recordAt(value, "valueSources");
	return {
		fixture_id: stringAt(row.fixture_id, "fixture_id"),
		generated_at: stringAt(row.generated_at, "generated_at"),
		grand_master: numberAt(row.grand_master, "grand_master"),
		blackout: booleanAt(row.blackout, "blackout"),
		highlighted: booleanAt(row.highlighted, "highlighted"),
		values: arrayAt(row.values, "values").map((value, index) => {
			const entry = recordAt(value, `values[${index}]`);
			return {
				attribute: stringAt(entry.attribute, "attribute"),
				value: decodeAttributeValue(entry.value, "value"),
				sources: arrayAt(entry.sources, "sources").map((source) =>
					stringAt(source, "source"),
				),
			};
		}),
		dynamics: arrayAt(row.dynamics, "dynamics").map((entry) =>
			stringAt(entry, "dynamic"),
		),
	};
}
