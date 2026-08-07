/// <reference types="vite/client" />

import type { PatchedFixture } from "../api/types";

/**
 * The body a fixture is drawn with when its profile carries no model of its own.
 *
 * Most shipped and imported profiles have no `model_asset` — of the ten profiles in the demo show,
 * none does — so this is what the Stage actually draws most of the time. It picks from the same
 * shipped set the native renderer picks from, by the same rule, so one fixture does not look like
 * two things depending on which renderer drew it.
 *
 * **The rule never looks at a manufacturer or a product name.** A fixture with pan, tilt and a gobo
 * wheel is a profile moving head whoever built it. The desk used to match on
 * `manufacturer + name + model` with patterns down to individual products, which meant renaming a
 * profile silently changed the picture and disagreed with the renderer about the same fixture.
 *
 * @see crates/viz/project/src/default_model.rs — the authority this mirrors, name for name.
 */

// Two directories, because the shipped set is not all lamps: a show laser is an AV fixture and
// lives with them. Both are globbed rather than listed so a renamed file is a build error here
// instead of a body that silently stops loading.
const modelUrls = {
	...import.meta.glob<string>("../../../../assets/models/lamps/*.glb", {
		eager: true,
		query: "?url",
		import: "default",
	}),
	...import.meta.glob<string>("../../../../assets/models/av/*.glb", {
		eager: true,
		query: "?url",
		import: "default",
	}),
};

/** The shipped bodies, by the name the catalogue and the renderer both use. */
const shipped = new Map<string, string>();
for (const [path, url] of Object.entries(modelUrls)) {
	const name = /\/([^/]+)\.glb$/u.exec(path)?.[1];
	if (name) shipped.set(name, url);
}

/** Every name the rule below can return, so a missing file is caught by a test rather than a hole. */
export const DEFAULT_MODEL_NAMES = [
	"fresnel-barn-doors",
	"profile-spot",
	"par-64-short-nose-black",
	"moving-head-profile",
	"moving-head-wash",
	"moving-head-led-wash-400",
	"led-par-x-in-1",
	"led-strobe",
	"blinder-4-cell",
	"scanner-mirror-spot",
	"led-strip-rgbcct-1000",
	"hazer",
	"show-laser",
] as const;

export type DefaultModelName = (typeof DEFAULT_MODEL_NAMES)[number];

/** Where a shipped body can be fetched from, or null when this build did not bundle it. */
export function defaultModelUrl(name: DefaultModelName): string | null {
	return shipped.get(name) ?? null;
}

/** What a fixture's channels say it can do, whatever it calls itself. */
export interface FixtureTraits {
	dimmer: boolean;
	pan: boolean;
	tilt: boolean;
	gobo: boolean;
	colourWheel: boolean;
	rgb: boolean;
	strobe: boolean;
	fog: boolean;
}

const moving = (traits: FixtureTraits) => traits.pan && traits.tilt;

/**
 * Fold every channel's canonical attribute into the traits.
 *
 * Attribute keys are canonical — `color.red`, `gobo.1`, `shutter.strobe` — so this matches on their
 * shape rather than on a list of spellings that would go stale.
 */
export function fixtureTraits(fixture: PatchedFixture): FixtureTraits {
	const traits: FixtureTraits = {
		dimmer: false,
		pan: false,
		tilt: false,
		gobo: false,
		colourWheel: false,
		rgb: false,
		strobe: false,
		fog: false,
	};
	for (const head of fixture.definition.heads ?? []) {
		for (const parameter of head.parameters ?? []) {
			const key = parameter.attribute.trim().toLowerCase();
			const base = key.split(".").at(-1) ?? key;
			if (key === "intensity" || key === "dimmer" || base === "dimmer")
				traits.dimmer = true;
			if (key === "pan") traits.pan = true;
			if (key === "tilt") traits.tilt = true;
			if (key === "fog" || key === "haze" || key === "smoke") traits.fog = true;
			if (key.startsWith("gobo")) traits.gobo = true;
			if (key.startsWith("color.wheel") || key === "color")
				traits.colourWheel = true;
			if (key.startsWith("color") && ["red", "green", "blue"].includes(base))
				traits.rgb = true;
			// Subtractive colour mixing is still colour mixing: a CMY head is not a gobo spot.
			if (key.startsWith("color") && ["cyan", "magenta", "yellow"].includes(base))
				traits.rgb = true;
			if (key === "strobe" || base === "strobe") traits.strobe = true;
		}
	}
	return traits;
}

/**
 * The declared type, when it says anything useful.
 *
 * Trusted over the channel set, because a profile that calls itself a blinder is a blinder even if
 * its author gave it an RGB mixer.
 */
function byDeclaredType(
	fixtureType: string,
	traits: FixtureTraits,
): DefaultModelName | null {
	const words = new Set(
		fixtureType.trim().toLowerCase().replace(/[_-]/gu, " ").split(/\s+/u),
	);
	const has = (needle: string) => words.has(needle);
	const isMoving = moving(traits) || has("moving");

	if (has("hazer") || has("haze") || has("fogger") || has("fog") || has("smoke"))
		return "hazer";
	// Before the pattern-position channels every show laser has can be mistaken for a yoke: a laser
	// is a box with a window in it whatever its chart calls pan and tilt.
	if (has("laser") || has("lasers")) return "show-laser";
	if (has("scanner") || has("mirror")) return "scanner-mirror-spot";
	if (has("blinder")) return "blinder-4-cell";
	if (has("strobe")) return "led-strobe";
	if (has("strip") || has("sunstrip") || has("pixel") || has("bar"))
		return "led-strip-rgbcct-1000";
	if (has("fresnel")) return "fresnel-barn-doors";
	if (has("par") || has("parcan") || has("acl"))
		return traits.rgb ? "led-par-x-in-1" : "par-64-short-nose-black";
	if (has("wash"))
		return isMoving ? "moving-head-led-wash-400" : "led-par-x-in-1";
	if (has("profile") || has("spot") || has("ellipsoidal") || has("beam"))
		return isMoving ? "moving-head-profile" : "profile-spot";
	return null;
}

/**
 * Failing a useful declared type, what the channels say.
 *
 * Read in order, because the tests are not mutually exclusive: a moving head with both a gobo wheel
 * and a colour mixer is a profile, and a fixture with a strobe channel and an RGB mixer is an LED
 * PAR that happens to strobe.
 */
function byAttributes(traits: FixtureTraits): DefaultModelName {
	if (traits.fog) return "hazer";
	if (moving(traits)) {
		if (traits.gobo || traits.colourWheel) return "moving-head-profile";
		if (traits.rgb) return "moving-head-led-wash-400";
		return "moving-head-wash";
	}
	if (traits.rgb) return "led-par-x-in-1";
	if (traits.strobe) return "led-strobe";
	// Nothing but a level. A bare dimmer channel is feeding a lantern nobody described, and a
	// Fresnel is the one that looks least wrong standing in for any of them.
	return "fresnel-barn-doors";
}

/** Which shipped body this fixture gets, by declared type first and channels second. */
export function chooseDefaultModelName(
	fixture: PatchedFixture,
): DefaultModelName {
	const traits = fixtureTraits(fixture);
	return (
		byDeclaredType(fixture.definition.device_type ?? "", traits) ??
		byAttributes(traits)
	);
}

/**
 * The model source for a fixture: its own if the package carries one, else the shipped body.
 *
 * Null only where this build bundled no such file, which leaves the procedural placeholder in
 * place rather than an empty rig.
 */
export function fixtureModelSource(fixture: PatchedFixture): string | null {
	return (
		fixture.definition.model_asset ??
		defaultModelUrl(chooseDefaultModelName(fixture))
	);
}
