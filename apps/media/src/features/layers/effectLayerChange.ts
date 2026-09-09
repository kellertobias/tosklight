import type { UpdateLayer } from "../../shared/api/generated/media-wire";

const EFFECT_CONTROL =
	/^effect-(\d+)-(type|enabled|mix|tv-curvature|distortion|image-grain|compression-damage|block-size|tile-displacement|chroma-damage|glitching|blur-amount|feedback-amount|feedback-motion|feedback-direction|cycle-interval|beat-move-amount|beat-move-direction|beat-move-decay|kaleidoscope-repetitions|kaleidoscope-angle|rasterize-mode|rasterize-dot-size|beat-scan-width|beat-scan-edge|beat-scan-falloff|beat-scan-duration|beat-scale-amount|beat-turn-enabled|beat-turn-rotation|beat-scale-decay|beat-grid-density|beat-grid-height|beat-grid-duration|beat-grid-origin|beat-grid-hue|beat-grid-brightness|beat-form-enlargement|beat-form-lifetime|beat-form-density|beat-form-variation|drawn-strength|drawn-line-detail)$/;

/** Converts one legacy native-effect editor control without touching other layer fields. */
export function effectLayerChange(
	id: string,
	value: string | number,
): UpdateLayer | undefined {
	const effect = EFFECT_CONTROL.exec(id);
	if (!effect) return undefined;
	const effectSlot = Number(effect[1]);
	const number = Number(value);
	switch (effect[2]) {
		case "type": return { effectSlot, effectType: String(value) };
		case "enabled": return { effectSlot, effectEnabled: value === "true" };
		case "mix": return { effectSlot, effectMix: number / 100 };
		case "tv-curvature": return { effectSlot, tvCurvature: number / 100 };
		case "distortion": return { effectSlot, effectDistortion: number / 100 };
		case "image-grain": return { effectSlot, imageGrain: number / 100 };
		case "compression-damage": return { effectSlot, compressionDamage: number / 100 };
		case "block-size": return { effectSlot, blockSize: number / 100 };
		case "tile-displacement": return { effectSlot, tileDisplacement: number / 100 };
		case "chroma-damage": return { effectSlot, chromaDamage: number / 100 };
		case "glitching": return { effectSlot, effectGlitching: number / 100 };
		case "blur-amount": return { effectSlot, blurAmount: number / 100 };
		case "feedback-amount": return { effectSlot, feedbackAmount: number / 100 };
		case "feedback-motion": return { effectSlot, feedbackMotion: number / 100 };
		case "feedback-direction": return { effectSlot, feedbackDirection: String(value) };
		case "cycle-interval": return { effectSlot, cycleInterval: String(value) };
		case "beat-move-amount": return { effectSlot, beatMoveAmount: number / 100 };
		case "beat-move-direction": return { effectSlot, beatMoveDirection: String(value) };
		case "beat-move-decay": return { effectSlot, beatMoveDecay: number };
		case "kaleidoscope-repetitions": return { effectSlot, kaleidoscopeRepetitions: number };
		case "kaleidoscope-angle": return { effectSlot, kaleidoscopeAngle: number };
		case "rasterize-mode": return { effectSlot, rasterizeMode: String(value) };
		case "rasterize-dot-size": return { effectSlot, rasterizeDotSize: number };
		case "beat-scan-width": return { effectSlot, beatScanWidth: number / 100 };
		case "beat-scan-edge": return { effectSlot, beatScanEdge: String(value) };
		case "beat-scan-falloff": return { effectSlot, beatScanFalloff: number / 100 };
		case "beat-scan-duration": return { effectSlot, beatScanDuration: number };
		case "beat-scale-amount": return { effectSlot, beatScaleAmount: number / 100 };
		case "beat-turn-enabled": return { effectSlot, beatTurnEnabled: value === "true" };
		case "beat-turn-rotation": return { effectSlot, beatTurnRotation: number };
		case "beat-scale-decay": return { effectSlot, beatScaleDecay: number };
		case "beat-grid-density": return { effectSlot, beatGridDensity: number };
		case "beat-grid-height": return { effectSlot, beatGridHeight: number / 100 };
		case "beat-grid-duration": return { effectSlot, beatGridDuration: number };
		case "beat-grid-origin": return { effectSlot, beatGridOrigin: String(value) };
		case "beat-grid-hue": return { effectSlot, beatGridHue: number };
		case "beat-grid-brightness": return { effectSlot, beatGridBrightness: number / 100 };
		case "beat-form-enlargement": return { effectSlot, beatFormEnlargement: number / 100 };
		case "beat-form-lifetime": return { effectSlot, beatFormLifetime: number };
		case "beat-form-density": return { effectSlot, beatFormDensity: number };
		case "beat-form-variation": return { effectSlot, beatFormVariation: number / 100 };
		case "drawn-strength": return { effectSlot, drawnStrength: number / 100 };
		case "drawn-line-detail": return { effectSlot, drawnLineDetail: number / 100 };
		default: return undefined;
	}
}
