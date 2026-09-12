import type {
	FixtureHead,
	FixtureMode,
	FixtureProfile,
} from "../../wire";
import { blankGeometry } from "./geometry";
import { uuid } from "./utilities";

export function blankHead(index = 0): FixtureHead {
	return {
		id: uuid(),
		name: index === 0 ? "Main" : `Head ${index + 1}`,
		master_shared: index === 0,
	};
}

export function blankMode(name = "Default"): FixtureMode {
	const head = blankHead();
	return {
		id: uuid(),
		name,
		notes: "",
		splits: [{ number: 1, footprint: 1 }],
		heads: [head],
		channels: [],
		color_systems: [],
		control_actions: [],
		emitter_heads: [],
		geometry: { nodes: [], emitters: [] },
	};
}

export function blankFixtureProfile(): FixtureProfile {
	// The lantern has one part and one emitter; the mode drives that emitter with its own head.
	const mode = blankMode();
	const geometry = blankGeometry([]);
	const beam = geometry.emitters[0];
	if (beam) {
		mode.emitter_heads = [
			{ emitter_id: beam.id, head_id: mode.heads[0].id },
		];
	}
	return {
		schema_version: 2,
		id: uuid(),
		revision: 0,
		manufacturer: "",
		name: "",
		short_name: "",
		fixture_type: "other",
		patch_policy: "dmx",
		notes: "",
		photograph_asset: null,
		stage_icon_asset: null,
		model_asset: null,
		model_units: "auto",
		physical: {
			width_millimetres: null,
			height_millimetres: null,
			depth_millimetres: null,
			weight_kilograms: null,
			power_watts: null,
			connectors: "",
			light_source: "",
			color_rendering_index: null,
			lens: "",
		},
		optics: {},
		geometry,
		modes: [mode],
		hazardous: false,
		direct_control_protocols: [],
		signal_loss_policy: { type: "hold_last" },
		reserved_source: null,
	};
}

export function cloneProfile(profile: FixtureProfile): FixtureProfile {
	return structuredClone(profile);
}
