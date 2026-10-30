import type { AttributeValue } from "../../api/types/playback";

/** Validated values and source labels shown by the Fixture Sheet inspector. */
export interface FixtureValueSourcesSnapshot {
	fixture_id: string;
	generated_at: string;
	grand_master: number;
	blackout: boolean;
	highlighted: boolean;
	values: { attribute: string; value: AttributeValue; sources: string[] }[];
	dynamics: string[];
}
