import type { EffectParameterView } from "./generated/media-wire";

/**
 * What each effect parameter accepts, as this server reports it.
 *
 * The server is authoritative — these rows mirror `media_domain::effect_parameter_bounds` so an
 * optimistic control and the stub server describe a parameter the same way the real API does. A
 * control that renders a range the server does not accept looks to an operator like a control
 * that does nothing.
 */
const UNIT = { minimum: 0, maximum: 1, step: 0.01 } as const;

const BOUNDS: Record<
	string,
	{ minimum: number; maximum: number; step: number }
> = {
	"feedback-direction": { minimum: 0, maximum: 5, step: 1 },
	"cycle-interval": { minimum: 0, maximum: 2, step: 1 },
	"beat-move-direction": { minimum: 0, maximum: 3, step: 1 },
	"beat-move-decay": { minimum: 0.05, maximum: 5, step: 0.05 },
	"kaleidoscope-repetitions": { minimum: 1, maximum: 16, step: 1 },
	"kaleidoscope-angle": { minimum: -180, maximum: 180, step: 1 },
	"rasterize-mode": { minimum: 0, maximum: 1, step: 1 },
	"rasterize-dot-size": { minimum: 2, maximum: 32, step: 1 },
	"beat-scan-width": { minimum: 0.01, maximum: 0.25, step: 0.01 },
	"beat-scan-edge": { minimum: 0, maximum: 1, step: 1 },
	"beat-scan-duration": { minimum: 0.2, maximum: 3, step: 0.05 },
	"beat-turn-enabled": { minimum: 0, maximum: 1, step: 1 },
	"beat-turn-rotation": { minimum: -30, maximum: 30, step: 1 },
	"beat-scale-decay": { minimum: 0.05, maximum: 5, step: 0.05 },
	"beat-grid-density": { minimum: 6, maximum: 64, step: 1 },
	"beat-grid-duration": { minimum: 0.2, maximum: 4, step: 0.05 },
	"beat-grid-origin": { minimum: 0, maximum: 4, step: 1 },
	"beat-grid-hue": { minimum: 0, maximum: 360, step: 1 },
	"beat-grid-brightness": { minimum: 0.1, maximum: 2, step: 0.05 },
	"beat-form-enlargement": { minimum: 1, maximum: 4, step: 0.05 },
	"beat-form-lifetime": { minimum: 0.1, maximum: 5, step: 0.05 },
	"beat-form-density": { minimum: 1, maximum: 4, step: 1 },
};

export function effectParameterBounds(id: string) {
	return BOUNDS[id] ?? UNIT;
}

/** Completes parameters written as identity, label, and value with what they accept. */
export function withEffectParameterBounds(
	parameters: Array<{
		id: string;
		label: string;
		value: number;
		defaultValue: number;
	}>,
): EffectParameterView[] {
	return parameters.map((parameter) => ({
		...parameter,
		...effectParameterBounds(parameter.id),
	}));
}
