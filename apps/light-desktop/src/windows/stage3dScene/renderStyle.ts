import type { StageRenderQuality } from "../../types";

/**
 * What a render style actually draws for a beam.
 *
 * Asked through these two rather than by comparing the value at each site: the Stage decides in a
 * dozen places whether to draw a cone and whether to draw a direction line, and a style added
 * later must not silently mean "the same as whatever the comparison happened to exclude".
 *
 * The operator picks between three — no beams, lines, cones. The other two values are what saved
 * layouts already carry, and a show that chose one keeps drawing what it chose.
 */

/** The volumetric cone. */
export function drawsBeamVolume(quality: StageRenderQuality) {
	return quality !== "none" && quality !== "lines_only";
}

/**
 * The thin direction line.
 *
 * Absent from the volumetric styles on purpose: a line down the middle of a cone is not extra
 * information, it is a seam.
 */
export function drawsBeamLine(quality: StageRenderQuality) {
	return quality === "lines_only" || quality === "lines_and_beams";
}

/**
 * Whether anything at all leaves the lens.
 *
 * `none` still lights the lens and still moves the head — the fixture is doing something and the
 * operator can see that it is — it simply does not draw where the light goes.
 */
export function drawsAnyBeam(quality: StageRenderQuality) {
	return drawsBeamVolume(quality) || drawsBeamLine(quality);
}
