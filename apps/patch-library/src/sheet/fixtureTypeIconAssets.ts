/// <reference types="vite/client" />

const fixtureTypeIconModules = import.meta.glob<string>(
	"../../../../assets/icons/fixture-type/*.svg",
	{
		eager: true,
		query: "?url",
		import: "default",
	},
);

const fixtureTypeIcons = new Map<string, string>();
for (const [path, url] of Object.entries(fixtureTypeIconModules)) {
	if (path.endsWith(".expanded.svg")) continue;
	const name = /\/([^/]+)\.svg$/u.exec(path)?.[1];
	if (name) fixtureTypeIcons.set(name, url);
}

function icon(name: string) {
	return fixtureTypeIcons.get(name) ?? fixtureTypeIcons.get("parcan") ?? "";
}

export function fixtureTypeIconAsset(type: string) {
	const value = type.toLowerCase();
	// A 3D Point is a reference object, not a lantern, so it is matched before anything that
	// could read its name as a beam or a fixture family.
	if (/point|anchor/.test(value)) return icon("position-point");
	if (/fog|haze/.test(value)) return icon("hazer");
	if (/fan|blower/.test(value)) return icon("blower");
	if (/media|video|projector/.test(value)) return icon("projector");
	if (/pixel|wall|matrix/.test(value)) return icon("led-wall");
	if (/strip|batten/.test(value)) return icon("strip-light");
	if (/strobe/.test(value)) return icon("strobe");
	if (/laser/.test(value)) return icon("laser");
	if (/effect|flame|spark|pyro/.test(value)) return icon("effect");
	if (/fresnel/.test(value)) return icon("fresnel-barn-doors");
	if (/\bacl\b/.test(value)) return icon("acl-set");
	if (/blinder/.test(value)) return icon("blinder");
	if (/wash/.test(value)) return icon("led-wash-moving-light-lenses");
	if (/moving|mover|beam/.test(value)) return icon("profile-moving-light");
	if (/profile|spot/.test(value)) return icon("profile-dimmer-lamp");
	if (/dimmer|relay|par/.test(value)) return icon("parcan");
	return icon("parcan");
}
