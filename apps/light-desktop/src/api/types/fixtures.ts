export interface PatchedFixture {
	fixture_id: string;
	fixture_number?: number | null;
	virtual_fixture_number?: number | null;
	name?: string;
	universe: number | null;
	address: number | null;
	layer_id?: string;
	direct_control?: {
		protocol: "citp";
		ip_address: string;
		port: number;
	} | null;
	internal_bindings?: {
		library?: string | null;
		output?: string | null;
	};
	definition: FixtureDefinition;
	logical_heads: Array<{
		/** Stable profile identity; absent only on legacy v1 projections. */
		profile_head_id?: string | null;
		fixture_id: string;
		head_index: number;
	}>;
	location?: { x: number; y: number; z: number };
	rotation?: { x: number; y: number; z: number };
	multipatch?: MultiPatchInstance[];
	group_masters_enabled?: boolean;
	grand_master_enabled?: boolean;
	invert_pan?: boolean;
	invert_tilt?: boolean;
	/** Degrees the mounting bracket is set to, positive nose-down. */
	bracket_angle?: number;
	/** Degrees a fitted shaper or barn-door module is turned to; absent when none is fitted. */
	shaper_angle?: number | null;
	installed_appearance?: InstalledFixtureAppearance;
	/** Schema-v2 fixtures patch each independently addressable split separately. */
	split_patches?: SplitPatch[];
	/** Exact raw values captured with the embedded profile snapshot. */
	highlight_overrides?: Record<string, number>;
	move_in_black_enabled?: boolean;
	move_in_black_delay_millis?: number;
	freeze_targets?: FixtureFreezeTarget[];
}

export type FixtureFreezeFamily = "intensity" | "color" | "position" | "beam";

export interface FixtureFreezeTarget {
	fixture_id: string;
	full: boolean;
	families: FixtureFreezeFamily[];
}

export interface MultiPatchInstance {
	id: string;
	name: string;
	universe: number | null;
	address: number | null;
	location: { x: number; y: number; z: number };
	rotation: { x: number; y: number; z: number };
	split_patches?: SplitPatch[];
	invert_pan?: boolean;
	invert_tilt?: boolean;
	bracket_angle?: number;
	shaper_angle?: number | null;
	installed_appearance?: InstalledFixtureAppearance;
}

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

export interface SplitPatch {
	split: number;
	universe: number | null;
	address: number | null;
}

export * from "@tosklight/patch/fixture-profile";
import type {
	FixtureProfile,
	XyzValue,
} from "@tosklight/patch/fixture-profile";

export interface FixtureDefinition {
	schema_version: number;
	id: string;
	revision: number;
	manufacturer: string;
	device_type: string;
	name: string;
	model: string;
	mode: string;
	footprint: number;
	heads: Array<{
		index: number;
		name: string;
		shared: boolean;
		parameters: Array<{
			/** Stable manufacturer/source identity retained separately from canonical programming. */
			source_attribute?: string;
			attribute: string;
			components: Array<{
				offset: number;
				byte_order: "msb_first" | "lsb_first";
			}>;
			default: number;
			virtual_dimmer: boolean;
			metadata?: {
				physical_min: number;
				physical_max: number;
				unit: string | null;
				invert: boolean;
				wrap: boolean;
				curve: string;
				position_movement_representation?: "speed" | "time" | "speed_or_time";
				position_axis_representation?: "absolute" | "endless";
			};
			capabilities: Array<{
				name: string;
				dmx_from: number;
				dmx_to: number;
				preset_family?: string | null;
			}>;
		}>;
	}>;
	color_calibration: {
		emitters: Array<{ name: string; xyz: XyzValue; limit: number }>;
		correction_matrix: number[][];
	} | null;
	physical: {
		pan_range_degrees?: number | null;
		tilt_range_degrees?: number | null;
		width_millimetres?: number | null;
		height_millimetres?: number | null;
		depth_millimetres?: number | null;
		weight_kilograms?: number | null;
		power_watts?: number | null;
	};
	model_asset?: string | null;
	icon_asset?: string | null;
	hazardous: boolean;
	direct_control_protocols: Array<"citp">;
	signal_loss_policy: { type: string; duration_millis?: number };
	safe_values: Record<string, unknown>;
	profile_id?: string | null;
	mode_id?: string | null;
	profile_snapshot?: FixtureProfile | null;
}

export interface MediaServerFixture {
	fixture_id: string;
	fixture_number?: number | null;
	name: string;
	kind?: "media_server" | "audio_player";
	endpoint: { protocol: "citp"; ip_address: string; port: number } | null;
	native_action?: string | null;
	layers: Array<{
		fixture_id: string;
		head_index: number;
		/** Attributes this logical head owns; controls outside it are shown disabled. */
		attributes?: string[];
	}>;
	/** Attributes of the shared master head, addressed through the parent fixture. */
	master_attributes?: string[];
	status: {
		online: boolean;
		last_success: string | null;
		last_error: string | null;
	};
	/** Live Internal Audio Player state; present only for `kind: "audio_player"`. */
	audio?: {
		folder: number;
		file: number;
		volume_percent: number;
		transport: "play" | "pause" | "stop";
		repeat: boolean;
		source: string | null;
		/**
		 * Everything the bound library holds. A CITP server advertises its library over the
		 * protocol; an Internal Audio Player has no such conversation, so the pane browses this.
		 */
		library?: Array<{ folder: number; file: number; name: string }>;
	} | null;
}

export interface OutputRoute {
	target?: { kind: "network" } | { kind: "usb_endpoint"; endpoint_id: string };
	protocol: "art_net" | "sacn";
	logical_universe: number;
	destination_universe: number;
	delivery_mode: "broadcast" | "multicast" | "unicast";
	destination: string | null;
	enabled: boolean;
	minimum_slots: number;
}

export interface OutputRouteRangeIntent {
	logical_start: number;
	logical_end: number;
	destination_start: number;
	destination_end: number;
	route: Omit<OutputRoute, "logical_universe" | "destination_universe">;
}

export interface PatchSnapshot {
	revision: number;
	fixtures: PatchedFixture[];
	routes: OutputRoute[];
}

export interface PatchLayer {
	id: string;
	name: string;
	order: number;
}
