import type {
	FixtureChannel,
	FixtureMode,
	GeometryEmitter,
	Vector3Value,
} from "../fixtureProfile";
import type { StageProceduralResourceCache } from "./resources";

export type { FixtureChannel, FixtureMode, GeometryEmitter, Vector3Value };

/**
 * What the Stage draws a fixture profile's geometry from, shared by the desk's Stage and every
 * fixture-profile editor's live preview so the two cannot disagree about what a profile looks like.
 *
 * The types here are the parts of a patched fixture and an output snapshot that drawing reads.
 * They are deliberately narrower than the desk's own records: the Architect previews a profile
 * that is not patched anywhere and has no output to show.
 */

export type AttributeValue =
	| { kind: "normalized"; value: number }
	| { kind: "spread"; value: number[] }
	| { kind: "discrete"; value: string }
	| { kind: "color_xyz"; value: { x: number; y: number; z: number } }
	| { kind: "raw_dmx"; value: number }
	| { kind: "raw_dmx_exact"; value: number };

/**
 * Kept so a saved layout that carries one decodes without complaint; what is in a beam is decided
 * by the view and the render quality.
 */
export type StageRenderQuality =
	| "none"
	| "lines_only"
	| "lines_and_beams"
	| "beams"
	| "improved_beams";

export type InstalledLightSource =
	| { type: "profile_default" }
	| { type: "tungsten" }
	| { type: "halogen" }
	| { type: "discharge" }
	| { type: "led" }
	| { type: "fluorescent" }
	| { type: "arc" }
	| { type: "other"; label: string };

export interface GelDefinitionSnapshot {
	number: string;
	name: string;
	display_srgb: string;
	visualizer_srgb: string;
}

export type GelAssignment =
	| { type: "open_white" }
	| {
			type: "built_in";
			catalog_id: string;
			entry_id: string;
			embedded_fallback: GelDefinitionSnapshot;
	  }
	| {
			type: "custom";
			name: string;
			color_srgb: string;
			note: string | null;
	  };

export interface InstalledFixtureAppearance {
	light_source: InstalledLightSource;
	color_temperature_kelvin: number | null;
	luminous_output_lumens: number | null;
	gel: GelAssignment;
	shaper_angles_degrees: [number, number, number, number];
}

/** The parts of a patched fixture that drawing its profile geometry reads. */
export interface StageProfileFixture {
	fixture_id: string;
	logical_heads: ReadonlyArray<{ fixture_id: string; head_index: number }>;
	definition: {
		physical: {
			width_millimetres?: number | null;
			height_millimetres?: number | null;
			depth_millimetres?: number | null;
		};
		profile_snapshot?: {
			optics?: { color_temperature_kelvin?: number | null } | null;
		} | null;
	};
	installed_appearance?: InstalledFixtureAppearance;
	shaper_angle?: number | null;
}

/** The parts of the output snapshot that decide how bright a beam is drawn. */
export interface StageOutputSnapshot {
	blackout: boolean;
	grand_master: number;
}

export type FixtureAttributeValues = Map<string, AttributeValue>;
export type FixtureValuesById = Map<string, FixtureAttributeValues>;

export type StageShaperState = {
	supported: [boolean, boolean, boolean, boolean];
	insertions: [number, number, number, number];
	anglesDegrees: [number, number, number, number];
	moduleRotationDegrees: number;
};

export type { StageProceduralResourceCache };
