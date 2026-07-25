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
	/** Schema-v2 fixtures patch each independently addressable split separately. */
	split_patches?: SplitPatch[];
	/** Exact raw values captured with the embedded profile snapshot. */
	highlight_overrides?: Record<string, number>;
	move_in_black_enabled?: boolean;
	move_in_black_delay_millis?: number;
}

export interface MultiPatchInstance {
	id: string;
	name: string;
	universe: number | null;
	address: number | null;
	location: { x: number; y: number; z: number };
	rotation: { x: number; y: number; z: number };
	split_patches?: SplitPatch[];
}

export interface SplitPatch {
	split: number;
	universe: number | null;
	address: number | null;
}

export interface FixtureProfile {
	schema_version: 2;
	id: string;
	revision: number;
	manufacturer: string;
	name: string;
	short_name: string;
	fixture_type: string;
	patch_policy?: "dmx" | "visual_only";
	notes: string;
	photograph_asset: string | null;
	stage_icon_asset: string | null;
	model_asset: string | null;
	model_units?: "auto" | "metres";
	physical: FixtureProfilePhysical;
	modes: FixtureMode[];
	hazardous: boolean;
	direct_control_protocols: Array<"citp">;
	signal_loss_policy: { type: string; duration_millis?: number };
	reserved_source: string | null;
}

export interface FixtureProfilePhysical {
	width_millimetres: number | null;
	height_millimetres: number | null;
	depth_millimetres: number | null;
	weight_kilograms: number | null;
	power_watts: number | null;
	connectors?: string;
	light_source?: string;
	color_temperature_kelvin?: number | null;
	color_rendering_index?: number | null;
	luminous_output_lumens?: number | null;
	lens?: string;
	beam_angle_degrees?: number | null;
}

export interface FixtureMode {
	id: string;
	name: string;
	notes: string;
	splits: FixtureSplit[];
	heads: FixtureHead[];
	channels: FixtureChannel[];
	color_systems: HeadColorSystem[];
	control_actions: ControlAction[];
	geometry: GeometryGraph;
}

export interface FixtureSplit {
	number: number;
	footprint: number;
}

export interface FixtureHead {
	id: string;
	name: string;
	master_shared: boolean;
}

export type ChannelResolution = "u8" | "u16" | "u24" | "u32";
export type ChannelBehavior = "controlled" | "static";

export interface FixtureChannel {
	id: string;
	head_id: string;
	split: number;
	attribute: string;
	resolution: ChannelResolution;
	secondary_slots: number[];
	default_raw: number;
	highlight_raw: number;
	physical_min: number | null;
	physical_max: number | null;
	unit: string | null;
	invert: boolean;
	snap: boolean;
	reacts_to_virtual_intensity: boolean;
	reacts_to_sequence_master: boolean;
	reacts_to_group_master: boolean;
	reacts_to_grand_master: boolean;
	behavior: ChannelBehavior;
	functions: ChannelFunction[];
}

export interface ChannelFunction {
	id: string;
	name: string;
	dmx_from: number;
	dmx_to: number;
	attribute: string;
	priority: number;
	behavior: ChannelFunctionBehavior;
}

export type ChannelFunctionBehavior =
	| {
			type: "continuous";
			physical_min: number;
			physical_max: number;
			unit: string | null;
	  }
	| { type: "fixed"; semantic_id: string; label: string; raw_value: number }
	| { type: "indexed"; semantic_id: string; label: string; raw_value: number }
	| { type: "control"; action_id: string };

export type ControlActionKind = "latched" | "momentary" | "timed_pulse";
export type ControlActionSemantic =
	| "custom"
	| "lamp_on"
	| "lamp_off"
	| "reset"
	| "fan_auto"
	| "fan_low"
	| "fan_high"
	| "fan_max";

export interface ControlAction {
	id: string;
	name: string;
	semantic: ControlActionSemantic;
	kind: ControlActionKind;
	duration_millis: number | null;
	assignments: ControlActionAssignment[];
}

export interface ControlActionAssignment {
	channel_id: string;
	active_raw: number;
	inactive_raw: number;
}

export interface HeadColorSystem {
	head_id: string;
	correction_matrix: [
		[number, number, number],
		[number, number, number],
		[number, number, number],
	];
	system: ColorSystem;
}

export type ColorSystem =
	| { type: "additive"; emitters: EmitterBinding[] }
	| {
			type: "subtractive";
			cyan_channel_id: string;
			magenta_channel_id: string;
			yellow_channel_id: string;
	  }
	| { type: "discrete_wheel"; channel_id: string; slots: ColorWheelSlot[] };

export interface EmitterBinding {
	channel_id: string;
	name: string;
	xyz: XyzValue;
	maximum_level: number;
	response_curve: number;
	visible: boolean;
}

export interface ColorWheelSlot {
	semantic_id: string;
	label: string;
	dmx_from: number;
	dmx_to: number;
	measured_xyz: XyzValue | null;
}

export interface XyzValue {
	x: number;
	y: number;
	z: number;
}

export interface GeometryGraph {
	nodes: GeometryNode[];
	emitters: GeometryEmitter[];
}

export interface Vector3Value {
	x: number;
	y: number;
	z: number;
}

export interface GeometryNode {
	id: string;
	name: string;
	parent_id: string | null;
	transform: {
		translation: Vector3Value;
		rotation_degrees: Vector3Value;
		scale: Vector3Value;
	};
	pivot: Vector3Value;
	glb_node: string | null;
	motion: GeometryMotion | null;
}

export interface GeometryMotion {
	attribute: string;
	kind: "rotation" | "translation";
	axis: Vector3Value;
	physical_min: number;
	physical_max: number;
}

export interface GeometryEmitter {
	id: string;
	name: string;
	node_id: string;
	head_id: string;
	origin: Vector3Value;
	orientation_degrees: Vector3Value;
	beam_angle_degrees: number;
	field_angle_degrees: number;
	feather: number;
	focus: number;
	directional: boolean;
	layout: EmitterLayout;
}

export type EmitterLayout =
	| { type: "point" }
	| { type: "matrix"; columns: number; rows: number; spacing: Vector3Value }
	| { type: "ring"; count: number; radius_millimetres: number }
	| { type: "strip"; count: number; spacing_millimetres: number }
	| { type: "explicit_pixels"; positions: Vector3Value[] };

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
	name: string;
	endpoint: { protocol: "citp"; ip_address: string; port: number };
	layers: Array<{ fixture_id: string; head_index: number }>;
	status: {
		online: boolean;
		last_success: string | null;
		last_error: string | null;
	};
}

export interface OutputRoute {
	protocol: "art_net" | "sacn";
	logical_universe: number;
	destination_universe: number;
	delivery_mode: "broadcast" | "multicast" | "unicast";
	destination: string | null;
	enabled: boolean;
	minimum_slots: number;
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
