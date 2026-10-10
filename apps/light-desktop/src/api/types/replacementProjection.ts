// Frontend domain model. API decoders validate wire input before constructing these values.
// The persisted address spelling remains unchanged; features do not depend on generated DTOs.
export interface ReplacementProfileContext {
	profile_id: string;
	profile_revision: number;
	mode_id: string;
}

export interface ReplacementHeadTarget {
	profile_head_id: string;
	fixture_id: string;
}

export interface ReplacementProgramProjection {
	source_owner: string;
	source_profile: ReplacementProfileContext;
	source_head_id: string;
	target_profile: ReplacementProfileContext;
	// Empty is explicit dormant intent, distinct from absent metadata.
	targets: ReplacementHeadTarget[];
}
