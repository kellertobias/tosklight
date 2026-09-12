// Canonical fixture-profile wire model, shared by ToskLight Control and ToskLight Architect.
// Mirrors crates/shared/fixture/src/profile/*.rs. Serialized field names are snake_case.

export interface FixtureProfile {
	schema_version: 2 | 3;
	id: string;
	revision: number;
	manufacturer: string;
	name: string;
	short_name: string;
	fixture_type: string;
	patch_policy?: "dmx" | "visual_only" | "internal";
	notes: string;
	photograph_asset: string | null;
	stage_icon_asset: string | null;
	model_asset: string | null;
	/**
	 * The generic body this fixture is drawn as, named from the body catalogue.
	 *
	 * Null keeps the guess made from the declared type and the mode's channels. A packaged
	 * `model_asset` wins over both.
	 */
	body_model?: string | null;
	/**
	 * The fixture's parts, axes and emitters.
	 *
	 * Geometry belongs to the lantern rather than to one of its personalities: a moving head has
	 * the same yoke whichever mode it is patched in. A mode says only which of its heads owns
	 * which emitter, in `FixtureMode.emitter_heads`.
	 *
	 * Empty on a profile whose modes still carry their own geometry.
	 */
	geometry?: GeometryGraph;
	model_units?: "auto" | "metres";
	projection_assets?: FixtureProjectionSet | null;
	physical: FixtureProfilePhysical;
	optics?: FixtureProfileOptics;
	crowd?: FixtureProfileCrowd | null;
	effect?: FixtureProfileEffect | null;
	modes: FixtureMode[];
	hazardous: boolean;
	direct_control_protocols: Array<"citp">;
	signal_loss_policy: { type: string; duration_millis?: number };
	reserved_source: string | null;
}

export interface FixtureProfileCrowd {
	default_width_metres: number;
	default_depth_metres: number;
	modes: Array<{
		mode_id: string;
		posture: "sitting" | "standing_still" | "dancing";
		density: "sparse" | "medium" | "dense";
	}>;
}

export interface FixtureProfileEffect {
	effect_script_asset?: string | null;
	result_version: number;
}
export type FixtureProjectionView = "top" | "left" | "right" | "front" | "back";
export type FixtureProjectionOrientation =
	| "x_right_z_down"
	| "z_right_y_up"
	| "z_left_y_up"
	| "x_right_y_up"
	| "x_left_y_up";
export type FixtureProjectionPose =
	| "authored_home"
	| "moving_down"
	| "moving_forward";

export interface FixtureProjectionAsset {
	view: FixtureProjectionView;
	artwork_asset: string;
	view_box_millimetres: [number, number, number, number];
	physical_width_millimetres: number;
	physical_height_millimetres: number;
	origin_millimetres: [number, number];
	orientation: FixtureProjectionOrientation;
	pose: FixtureProjectionPose;
}

export interface FixtureProjectionSet {
	source_model_sha256: string;
	generator: string;
	generator_version: string;
	pose_contract_version: number;
	views: FixtureProjectionAsset[];
}

/**
 * What this fixture's light looks like, as against how the fixture is built.
 *
 * Every figure is optional. What a profile leaves out is derived from its declared fixture type,
 * so a library that has never been told any of this still renders sensibly.
 */
export interface FixtureProfileOptics {
	/** Relative output, 1 being an ordinary fixture of its type. */
	output?: number | null;
	/** How hard the rim of the field is, 0 to 1. */
	sharpness?: number | null;
	/** How evenly the field is filled, 0 to 1. */
	uniformity?: number | null;
	/** The lit surface light leaves through. The same for every fixture of this type. */
	light_source?: FixtureProfileLightSource | null;
	/** Correlated colour temperature of the engine, in kelvin. */
	color_temperature_kelvin?: number | null;
	/** Total output in lumens, as the manufacturer measures it. */
	luminous_output_lumens?: number | null;
	/** Nominal beam angle in degrees; a zoom channel's own range overrides it while it moves. */
	beam_angle_degrees?: number | null;
}

export interface FixtureProfileLightSource {
	form: "round" | "oval" | "rectangular";
	width_millimetres: number;
	height_millimetres: number;
}

export interface FixtureProfilePhysical {
	width_millimetres: number | null;
	height_millimetres: number | null;
	depth_millimetres: number | null;
	weight_kilograms: number | null;
	power_watts: number | null;
	connectors?: string;
	light_source?: string;
	color_rendering_index?: number | null;
	lens?: string;
}

export interface FixtureMode {
	id: string;
	name: string;
	notes: string;
	splits: FixtureSplit[];
	heads: FixtureHead[];
	channels: FixtureChannel[];
	color_systems: HeadColorSystem[];
	/** Which of the fixture's emitters each of this mode's heads owns. */
	emitter_heads?: EmitterHeadBinding[];
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
export type CanonicalTransform = "identity" | "invert_normalized";

export interface FixtureChannel {
	id: string;
	head_id: string;
	split: number;
	fixture_attribute: string;
	attribute: string;
	canonical_transform: CanonicalTransform;
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
	angular_motion?: AngularMotion | null;
	behavior: ChannelFunctionBehavior;
}

export type AngularMotionKind = "absolute_position" | "angular_velocity";

export interface AngularMotion {
	kind: AngularMotionKind;
	max_speed_degrees_per_second?: number | null;
	acceleration_degrees_per_second_squared?: number | null;
	deceleration_degrees_per_second_squared?: number | null;
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
	| {
			type: "hue_saturation";
			hue_channel_id: string;
			saturation_channel_id: string;
			intensity_channel_id?: string | null;
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
	/**
	 * How fast this axis can actually travel, per second in its own physical unit — degrees for a
	 * rotation, the translation's unit for a translation. Omitted means it moves as told.
	 */
	max_speed_per_second?: number | null;
	acceleration_per_second_squared?: number | null;
	deceleration_per_second_squared?: number | null;
}

export interface GeometryEmitter {
	id: string;
	name: string;
	node_id: string;
	/**
	 * Which head owns this emitter — a mode's answer, not the fixture's. Retained on profiles
	 * written before geometry left the mode.
	 */
	head_id?: string | null;
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


/**
 * The desk's canonical attribute registry entry. A profile channel names one of these as its
 * canonical identity, so the editor needs the same list Control and Architect resolve against.
 */
export interface AttributeDescriptor {
	id: string;
	label: string;
	family:
		| "intensity"
		| "position"
		| "color"
		| "beam"
		| "shapers"
		| "focus"
		| "control"
		| "media"
		| "custom";
	value_type: "continuous" | "color" | "indexed" | "control";
	default_unit: string | null;
	display_unit?: string | null;
	physical_unit?: string | null;
	normalized_min?: number | null;
	normalized_max?: number | null;
	domain_min?: number | null;
	domain_max?: number | null;
	cyclic?: boolean;
	recordable?: boolean;
	encoder_group?:
		| "intensity"
		| "color"
		| "position"
		| "beam"
		| "shapers"
		| "focus"
		| "control"
		| "media";
	encoder_page?: number;
	encoder_slot?: number;
	built_in?: boolean;
	retired?: boolean;
	activation_group_id?: string | null;
	push_turn_of?: string | null;
}


/** One generic body an operator can draw a fixture as, as the desk offers it. */
export interface FixtureBodyModel {
	id: string;
	label: string;
	group: string;
}

/** Which logical head owns one of the fixture's emitters, in this mode. */
export interface EmitterHeadBinding {
	emitter_id: string;
	head_id: string;
}
