import { recipeHsv } from "./RangeControls";
import { recipePreview, wheelColors, wheelCorrections, type Recipe } from "./mockupModel";

export const magentaRecipe: Recipe = { red: 100, green: 0, blue: 100, white: 0, amber: 0, uv: 0, temperature: 6500, tint: 0, wheel1: 0, wheel2: 0 };
export const warmWhiteRecipe: Recipe = { ...magentaRecipe, red: 100, green: 100, blue: 100, white: 100, temperature: 3200 };

// User-supplied seven-slot example, not a transcription of the library's AURO Z300.
// Palette colors are illustrative display values, not measured filter chromaticities.
export const auroExampleSlots = [
	{ name: "Open", hex: "#ffffff" },
	{ name: "Warm White", hex: recipePreview(warmWhiteRecipe).hex },
	{ name: "Red", hex: "#ff3025" },
	{ name: "Yellow", hex: "#ffe600" },
	{ name: "Light blue", hex: "#54cbff" },
	{ name: "Dark blue", hex: "#342bc7" },
	{ name: "Green", hex: "#17ce55" },
];

export interface ColorMatch {
	id: string; name: string; shortName: string; capability: string; hex: string;
	approximate: boolean; method: string; explanation: string; quality: string; uvSupported: boolean;
	warnings: string[];
}

/** Deterministic UI demonstration only. The production resolver requires physical data. */
export function mixedColorMatches(recipe: Recipe): ColorMatch[] {
	const requested = recipePreview(recipe).hex;
	const distance = (hex: string) => [1, 3, 5].reduce((sum, at) => sum + (Number.parseInt(hex.slice(at, at + 2), 16) - Number.parseInt(requested.slice(at, at + 2), 16)) ** 2, 0);
	const autoSlot = auroExampleSlots.reduce((best, candidate) => distance(candidate.hex) < distance(best.hex) ? candidate : best);
	const requestedSlot = [undefined, "Open", "Red", "Dark blue", "Amber"][recipe.wheel1];
	const forcedSlot = auroExampleSlots.find(slot => slot.name === requestedSlot);
	const slot = forcedSlot ?? autoSlot;
	const correctionWarnings = recipe.wheel2 > 1 ? [`Correction wheel ${wheelCorrections[recipe.wheel2]} unavailable; request retained.`] : [];
	const mixerWarnings = [...(recipe.wheel1 > 1 ? [`Wheel 1 ${wheelColors[recipe.wheel1]} unavailable; request retained.`] : []), ...correctionWarnings];
	const wheelWarnings = [...(requestedSlot && !forcedSlot ? [`No ${requestedSlot} slot; automatic fallback shown, request retained.`] : []), ...correctionWarnings];
	const white = recipe.white > 0;
	const warm = white && recipe.temperature < 5000;
	return [
		{ id: "a7", name: "JBLED A7", shortName: "JBLED A7", capability: "RGB", hex: requested, approximate: false,
			method: recipe.white === 100 ? "RGB mixed white" : "RGB mix",
			explanation: white ? "The white contribution is synthesized with red, green and blue. No separate white emitter." : "RGB produces the requested display color; magenta uses red and blue.",
			quality: "Nominal RGB preview; fixture output has not been measured.", uvSupported: false, warnings: mixerWarnings },
		{ id: "root", name: "Cameo ROOT PAR 6", shortName: "ROOT PAR 6", capability: "RGBWAUV", hex: requested, approximate: false,
			method: warm ? "RGB + white + amber" : white ? "RGB + white" : recipe.amber > 0 ? "RGB + amber" : "RGB mix",
			explanation: warm ? "White and amber can contribute to the warm-white component. Actual allocation depends on emitter calibration." : white ? "RGB and white contribute to the requested color and white target. Actual allocation depends on emitter calibration." : recipe.amber > 0 ? "The virtual amber contribution is retained in the color request. Actual emitter allocation depends on calibration." : "RGB produces the requested display color. No white, amber or UV contribution is needed for the magenta example.",
			quality: "Library emitters use nominal values; white CCT and emitter chromaticities are not manufacturer measurements.", uvSupported: true, warnings: mixerWarnings },
		{ id: "auro", name: "Cameo AURO SPOT", shortName: "AURO SPOT", capability: "7-slot wheel", hex: slot.hex, approximate: slot.hex !== requested,
			method: slot.name,
			explanation: forcedSlot ? `${slot.name} is explicitly selected. The requested color remains stored even when the forced filter differs.` : slot.hex !== requested ? `No continuous color mixing. ${slot.name} is the closest slot in this example palette; the requested color is retained.` : `${slot.name} is selected. A similar display swatch does not establish a measured color-temperature or spectral match.`,
			quality: "Illustrative seven-slot palette supplied for this example; no measured filter colors.", uvSupported: false, warnings: wheelWarnings },
	];
}

export function ColorMatches({ recipe, recipes = [recipe, recipe, recipe], ranged = false }: { recipe: Recipe; recipes?: Recipe[]; ranged?: boolean }) {
	const matches = recipes.map((r, index) => ({ ...mixedColorMatches(r)[index], recipe: r }));
	return <section className="fam-color-matches" aria-label="Selected fixture color matches" data-testid="color-matches">
		<div className="fam-match-request"><span className="fam-result-swatch" data-testid="requested-color" style={{ background: recipePreview(recipe).hex }} /><b>{ranged ? "Selection spread · first → last" : "Requested color"}</b><small>Illustrative output</small></div>
		<div className="fam-match-results">{matches.map(match => <div key={match.id} className="fam-color-match" data-testid={`color-match-${match.id}`}
			data-white={match.recipe.white} data-temperature={match.recipe.temperature} data-duv={match.recipe.tint} data-hue={recipeHsv(match.recipe).hue} data-saturation={recipeHsv(match.recipe).saturation}>
			<div className="fam-output-swatches">{ranged && <span className="fam-result-swatch" aria-label={`${match.name} requested color`} style={{ background: recipePreview(match.recipe).hex }} />}<span className="fam-result-swatch" aria-label={`${match.name} estimated output`} style={{ background: match.hex }} /></div>
			<div className="fam-match-copy"><b>{match.name}</b><small>{match.capability} · {match.method}</small>
				{match.warnings.map(warning => <small key={warning} className="fam-match-warning">{warning}</small>)}
				{match.recipe.uv > 0 && <small className={match.uvSupported ? "" : "fam-match-warning"}>UV {match.recipe.uv}% · {match.uvSupported ? "UV emitter available" : "unsupported; request retained"}</small>}
			</div>
			<span className={`fam-match-status${match.approximate || match.warnings.length ? " fam-match-warning" : ""}`}>{match.warnings.length ? "Wheel limit" : match.approximate ? "≈ Approx." : "Estimated"}</span>
		</div>)}</div>
	</section>;
}

/** The other stories use four identical fixtures and an illustrative wheel palette. */
export function ExampleColorMatches({ recipes, capability }: { recipes: Recipe[]; capability: string }) {
	const wheelHexes = ["#ffffff", "#ff3d28", "#3779ff", "#ffba43"];
	const names: Record<string, string> = { rgbw: "RGBW", rgbwauv: "RGBWAUV", rgbal: "RGBAL", cmy: "CMY + wheels", wheel: "Color wheel" };
	return <div className="fam-generic-matches">{recipes.map((recipe, index) => {
		const requested = recipePreview(recipe).hex;
		const distance = (hex: string) => [1, 3, 5].reduce((sum, at) => sum + (Number.parseInt(hex.slice(at, at + 2), 16) - Number.parseInt(requested.slice(at, at + 2), 16)) ** 2, 0);
		const nearest = wheelHexes.reduce((a, b) => distance(a) < distance(b) ? a : b);
		const output = capability === "wheel" ? (recipe.wheel1 > 0 ? wheelHexes[recipe.wheel1 - 1] : nearest) : capability === "cmy" && recipe.wheel1 > 1 ? wheelHexes[recipe.wheel1 - 1] : requested;
		const unsupportedUv = recipe.uv > 0 && capability !== "rgbwauv";
		return <div key={index} data-testid={`selection-color-${index}`} data-white={recipe.white} data-temperature={recipe.temperature} data-duv={recipe.tint} data-hue={recipeHsv(recipe).hue} data-saturation={recipeHsv(recipe).saturation}>
			<span className="fam-result-swatch" aria-label={`Fixture ${101 + index} estimated output`} style={{ background: output }} /><span>Front wash {index + 1} · {names[capability]}</span>
			<small className={output !== requested || unsupportedUv ? "fam-match-warning" : ""}>{output !== requested ? "≈ Approx." : "Estimated mix"}{unsupportedUv ? " · UV unavailable; retained" : ""}</small>
		</div>;
	})}</div>;
}
