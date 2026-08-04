export type AttributeEncoderGroup =
	| "intensity"
	| "color"
	| "position"
	| "beam"
	| "shapers"
	| "focus"
	| "control"
	| "media";

export type AttributeValueType = "continuous" | "color" | "indexed" | "control";

export interface AttributeBounds {
	min: number;
	max: number;
}

export interface CustomAttributeDescriptor {
	id: string;
	label: string;
	value_type: AttributeValueType;
	display_unit: string | null;
	physical_unit: string | null;
	normalized_bounds: AttributeBounds | null;
	domain_bounds: AttributeBounds | null;
	cyclic: boolean;
	recordable: boolean;
	lifecycle: "active" | "retired";
}

export interface AttributePlacement {
	attribute: string;
	encoder_group: AttributeEncoderGroup;
	encoder_page: number;
	encoder_slot: number;
	push_turn_of?: string | null;
}

export interface AttributeActivationGroup {
	id: string;
	label: string;
	members: string[];
}

export interface AttributeConfiguration {
	version: number;
	custom_attributes: CustomAttributeDescriptor[];
	placements: AttributePlacement[];
	activation_groups: AttributeActivationGroup[];
}

export interface ConfiguredAttributeDescriptor {
	id: string;
	label: string;
	encoder_group: AttributeEncoderGroup;
	encoder_page: number;
	encoder_slot: number;
	value_type: AttributeValueType;
	display_unit: string | null;
	physical_unit: string | null;
	normalized_min: number | null;
	normalized_max: number | null;
	domain_min: number | null;
	domain_max: number | null;
	cyclic: boolean;
	recordable: boolean;
	built_in: boolean;
	retired: boolean;
	activation_group_id: string | null;
	push_turn_of?: string | null;
}

export interface AttributeConfigurationSnapshot {
	show_id: string | null;
	show_revision: number;
	object_revision: number;
	configuration: AttributeConfiguration;
	recommended_configuration: AttributeConfiguration;
	descriptors: ConfiguredAttributeDescriptor[];
	validation_error: string | null;
}

export interface AttributeConfigurationPatch {
	custom_attributes?: CustomAttributeDescriptor[] | null;
	placements?: AttributePlacement[] | null;
	activation_groups?: AttributeActivationGroup[] | null;
}
