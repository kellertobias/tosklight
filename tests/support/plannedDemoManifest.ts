export const PLANNED_DEMO_CONTROL_FIXTURES = 253;
export const PLANNED_DEMO_PHYSICAL_INSTANCES = 286;

export type DemoLocation = "stage" | "audience" | "aux";
export type DemoFamily = "profile" | "wash" | "led";

export interface DemoFixtureManifestEntry {
	number: number;
	name: string;
	profile: {
		manufacturer: string;
		name: string;
		mode: string;
		archive: string;
	};
	family?: DemoFamily;
	location?: DemoLocation;
	roles: readonly string[];
	multipatches: number;
	patch?: {
		universe: number;
		address: number;
		multipatchAddresses?: readonly number[];
	};
}

const PROFILES = {
	movingProfile: profile(
		"ROBE",
		"Robin DLS Profile",
		"Mode 3",
		"robe--robin-dls-profile.toskfixture",
	),
	wash: profile(
		"JB-Lighting",
		"JBLED A7",
		"Standard RGB 16 Bit (S16)",
		"jb-lighting--jbled-a7.toskfixture",
	),
	led: profile(
		"Generic",
		"RGBW LED",
		"DRGBW 8-bit dimmer first",
		"generic--rgbw-led.toskfixture",
	),
	audienceBeam: profile(
		"ROBE",
		"Robin LEDBeam 150",
		"Mode 1 – Standard 16-bit",
		"robe--robin-ledbeam-150.toskfixture",
	),
	staticProfile: profile(
		"Generic",
		"Dimmer Profile",
		"8-bit",
		"generic--dimmer-profile.toskfixture",
	),
	fresnel: profile(
		"Generic",
		"Dimmer Fresnel",
		"8-bit",
		"generic--dimmer-fresnel.toskfixture",
	),
	par: profile(
		"Generic",
		"Dimmer PAR Can",
		"8-bit",
		"generic--dimmer-par-can.toskfixture",
	),
	sunstrip: profile(
		"Showtec",
		"Sunstrip LED RGB 42206",
		"30 Channel",
		"showtec--sunstrip-led-rgb-42206.toskfixture",
	),
	acl: profile("Generic", "ACL", "8-bit", "generic--acl.toskfixture"),
	blinder: profile(
		"Generic",
		"Blinder",
		"Two channel, four blind",
		"generic--blinder.toskfixture",
	),
	hazer: profile("Generic", "Hazer", "Fan, Fog", "generic--hazer.toskfixture"),
	mediaServer: profile(
		"ToskLight",
		"Media Server",
		"2 layers",
		"tosklight--media-server.toskfixture",
	),
	laser: profile(
		"ToskLight",
		"Visualizer Laser",
		"12 Channel",
		"tosklight--visualizer-laser.toskfixture",
	),
	sparkler: profile(
		"Generic",
		"Cold Spark Fountain",
		"Intensity, Height, Lifetime",
		"generic--cold-spark.toskfixture",
	),
	flameJet: profile(
		"Generic",
		"Flame Jet",
		"Intensity, Height, Colour",
		"generic--flame-jet.toskfixture",
	),
} as const;

export const PLANNED_DEMO_FIXTURES: readonly DemoFixtureManifestEntry[] = [
	...family(
		"profile",
		"stage",
		101,
		28,
		PROFILES.movingProfile,
		"Profile Stage",
	),
	...family(
		"profile",
		"audience",
		129,
		4,
		PROFILES.movingProfile,
		"Profile Audience",
	),
	...family(
		"profile",
		"audience",
		149,
		2,
		PROFILES.movingProfile,
		"Profile Audience",
		["Follow Spots"],
	),
	...family("profile", "aux", 151, 4, PROFILES.movingProfile, "Profile Aux"),
	...family("wash", "stage", 201, 26, PROFILES.wash, "Wash Stage"),
	...family("wash", "audience", 227, 8, PROFILES.wash, "Wash Audience"),
	...family("wash", "aux", 243, 4, PROFILES.wash, "Wash Aux"),
	...family("led", "stage", 301, 16, PROFILES.led, "LED Stage"),
	...family("led", "audience", 317, 100, PROFILES.led, "LED Audience"),
	...family("led", "aux", 417, 10, PROFILES.led, "LED Aux"),
	...namedRange(451, 8, "Beam Audience", PROFILES.audienceBeam, [
		"Audience Beams",
	]),
	patchedFixture(
		1,
		"Front Truss Left 1",
		PROFILES.fresnel,
		["Front Lights"],
		1,
	),
	patchedFixture(
		2,
		"Front Truss Left 2",
		PROFILES.fresnel,
		["Front Lights"],
		2,
	),
	patchedFixture(
		3,
		"Front Truss Left 3",
		PROFILES.fresnel,
		["Front Lights"],
		3,
	),
	patchedFixture(
		4,
		"Front Truss Right 1",
		PROFILES.fresnel,
		["Front Lights"],
		4,
	),
	patchedFixture(
		5,
		"Front Truss Right 2",
		PROFILES.fresnel,
		["Front Lights"],
		5,
	),
	patchedFixture(
		6,
		"Front Truss Right 3",
		PROFILES.fresnel,
		["Front Lights"],
		6,
	),
	patchedFixture(7, "Side Light Left", PROFILES.fresnel, ["Front Lights"], 7),
	patchedFixture(8, "Side Light Right", PROFILES.fresnel, ["Front Lights"], 8),
	patchedFixture(9, "Front Drums", PROFILES.fresnel, ["Front Lights"], 9, 1),
	patchedFixture(
		11,
		"Profile Stage Left",
		PROFILES.staticProfile,
		["Front Lights", "Front Profiles"],
		10,
	),
	patchedFixture(
		12,
		"Profile Stage Right",
		PROFILES.staticProfile,
		["Front Lights", "Front Profiles"],
		11,
	),
	patchedFixture(
		13,
		"Profile Stage Center",
		PROFILES.staticProfile,
		["Front Lights", "Front Profiles"],
		12,
		1,
	),
	...namedRange(501, 8, "Sunstrip", PROFILES.sunstrip, ["Sunstrips"]),
	...[
		["ACL Back Center", "ACL 1"],
		["ACL Back Outside", "ACL 2"],
		["ACL Midtruss", "ACL 3"],
		["ACL Side", "ACL 4"],
	].map(([name, role], index) =>
		patchedFixture(
			601 + index,
			name,
			PROFILES.acl,
			[role, "All ACLs"],
			13 + index,
			7,
		),
	),
	patchedFixture(701, "Fourblinder Left", PROFILES.blinder, ["Blinders"], 21),
	patchedFixture(702, "Fourblinder Right", PROFILES.blinder, ["Blinders"], 23),
	patchedFixture(801, "Hazer Stage Left", PROFILES.hazer, ["Hazers"], 509),
	patchedFixture(802, "Hazer Stage Right", PROFILES.hazer, ["Hazers"], 511),
	{
		...patchedFixture(
			901,
			"House Lights",
			PROFILES.par,
			["House Lights"],
			17,
			3,
		),
		patch: { universe: 1, address: 17, multipatchAddresses: [18, 19, 20] },
	},
	...namedRange(1001, 2, "Media Server", PROFILES.mediaServer, [
		"Media Servers",
	]),
	...[
		["Laser Left", 1101],
		["Laser Center", 1102],
		["Laser Right", 1103],
	].map(([name, number]) =>
		fixture(Number(number), String(name), PROFILES.laser, ["Lasers"]),
	),
	...namedRange(1201, 6, "Sparkler", PROFILES.sparkler, ["Sparklers"]),
	...namedRange(1301, 3, "Flame Jet", PROFILES.flameJet, ["Flame Jets"]),
];

export const PLANNED_DEMO_FIRST_LEVEL_GROUPS = [
	"Profile All",
	"Profile Stage",
	"Profile Audience",
	"Profile Aux",
	"Wash All",
	"Wash Stage",
	"Wash Audience",
	"Wash Aux",
	"LED All",
	"LED Stage",
	"LED Audience",
	"LED Aux",
] as const;

export function plannedDemoRoleNumbers(role: string): number[] {
	return PLANNED_DEMO_FIXTURES.filter((entry) =>
		entry.roles.includes(role),
	).map((entry) => entry.number);
}

export function plannedDemoFamilyNumbers(
	familyName: DemoFamily,
	location?: DemoLocation,
): number[] {
	return PLANNED_DEMO_FIXTURES.filter(
		(entry) =>
			entry.family === familyName &&
			(location == null || entry.location === location),
	).map((entry) => entry.number);
}

function family(
	familyName: DemoFamily,
	location: DemoLocation,
	first: number,
	quantity: number,
	fixtureProfile: DemoFixtureManifestEntry["profile"],
	locationRole: string,
	extraRoles: readonly string[] = [],
): DemoFixtureManifestEntry[] {
	const familyLabel = familyName === "led" ? "LED" : title(familyName);
	const placeLabel = location === "aux" ? "Aux" : title(location);
	return Array.from({ length: quantity }, (_, index) =>
		fixture(
			first + index,
			`${familyLabel} ${placeLabel} ${index + 1}`,
			fixtureProfile,
			[`${familyLabel} All`, locationRole, ...extraRoles],
			0,
			familyName,
			location,
		),
	);
}

function namedRange(
	first: number,
	quantity: number,
	label: string,
	fixtureProfile: DemoFixtureManifestEntry["profile"],
	roles: readonly string[],
): DemoFixtureManifestEntry[] {
	return Array.from({ length: quantity }, (_, index) =>
		fixture(first + index, `${label} ${index + 1}`, fixtureProfile, roles),
	);
}

function fixture(
	number: number,
	name: string,
	fixtureProfile: DemoFixtureManifestEntry["profile"],
	roles: readonly string[],
	multipatches = 0,
	familyName?: DemoFamily,
	location?: DemoLocation,
): DemoFixtureManifestEntry {
	return {
		number,
		name,
		profile: fixtureProfile,
		family: familyName,
		location,
		roles,
		multipatches,
	};
}

function patchedFixture(
	number: number,
	name: string,
	fixtureProfile: DemoFixtureManifestEntry["profile"],
	roles: readonly string[],
	address: number,
	multipatches = 0,
): DemoFixtureManifestEntry {
	return {
		...fixture(number, name, fixtureProfile, roles, multipatches),
		patch: { universe: 1, address },
	};
}

function profile(
	manufacturer: string,
	name: string,
	mode: string,
	archive: string,
) {
	return { manufacturer, name, mode, archive };
}

function title(value: string) {
	return value[0].toUpperCase() + value.slice(1);
}
