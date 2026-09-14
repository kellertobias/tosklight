import type { PatchedFixture } from "../../../api/types";

/** The range a placed Venue object's model scale may take; the Rust patch refuses anything else. */
export const MODEL_SCALE_MIN = 0.01;
export const MODEL_SCALE_MAX = 100;

/**
 * Whether a fixture is a Venue object a model scale applies to: anything visual-only the picture
 * draws as a body. A Crowd Area is visual-only too, but it is an area sized by its footprint.
 */
export function hasModelScale(fixture: PatchedFixture) {
	const profile = fixture.definition.profile_snapshot;
	return profile?.patch_policy === "visual_only" && !profile.crowd;
}

/** The scale this one is drawn at; an object placed before anyone set one is at its built size. */
export function modelScaleOf(fixture: PatchedFixture) {
	const stored = fixture.model_scale;
	return typeof stored === "number" &&
		Number.isFinite(stored) &&
		stored >= MODEL_SCALE_MIN &&
		stored <= MODEL_SCALE_MAX
		? stored
		: 1;
}

export function formatModelScale(scale: number) {
	return `${Number(scale.toFixed(3))}×`;
}

/**
 * The stored scale after typing one, or why it is refused. An empty field or `1` returns the object
 * to its built size, which is stored as no scale at all. A value outside the range is refused with
 * the range rather than clamped, so a typed 1000 never quietly becomes 100.
 */
export function modelScaleChange(
	value: string,
): { model_scale: number | null } | { error: string } {
	const trimmed = value.trim().replace(/[×x]$/iu, "");
	if (!trimmed) return { model_scale: null };
	const parsed = Number(trimmed);
	if (
		!Number.isFinite(parsed) ||
		parsed < MODEL_SCALE_MIN ||
		parsed > MODEL_SCALE_MAX
	)
		return {
			error: `Enter a scale from ${MODEL_SCALE_MIN} to ${MODEL_SCALE_MAX}; 1 is the size it was built at.`,
		};
	return { model_scale: parsed === 1 ? null : parsed };
}
