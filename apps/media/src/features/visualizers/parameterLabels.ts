// What one visualizer calls a shared parameter, and how its audio-or-beat choice reads.
//
// The parameter block is shared by every kind, so a few kinds give a parameter a name of their own:
// the equalizer's Amount is its Bloom, Fractal Morph's two colours are where its gradient starts
// and ends. The Visualizers page and the Media pane both label controls from here.

const EQUALIZER_BARS = 0;
const STARFIELD = 22;
const MATRIX_DIGITAL_RAIN = 42;
const FRACTAL_MORPH = 51;

/** The published parameter name of the audio-or-beat choice. */
export const ON_BEAT = "on-beat";

export function visualizerParameterLabel(
	typeId: number,
	parameter: string,
	fallback: string,
): string {
	if (typeId === EQUALIZER_BARS && parameter === "amount") return "Bloom";
	if (typeId === MATRIX_DIGITAL_RAIN && parameter === "burst")
		return "Streaks per beat";
	if (typeId === FRACTAL_MORPH && parameter === "primary") return "Start colour";
	if (typeId === FRACTAL_MORPH && parameter === "secondary")
		return "End colour";
	if (parameter === ON_BEAT)
		return typeId === STARFIELD ? "Spawn stars" : "React to";
	return fallback;
}

/** The two answers to the audio-or-beat choice, keyed by the stored switch. */
export function onBeatOptions(
	typeId: number,
): { value: "false" | "true"; label: string }[] {
	return typeId === STARFIELD
		? [
				{ value: "false", label: "Continuously" },
				{ value: "true", label: "On beat" },
			]
		: [
				{ value: "false", label: "Audio" },
				{ value: "true", label: "Beat" },
			];
}
