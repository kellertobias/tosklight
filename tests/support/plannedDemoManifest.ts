export const PLANNED_DEMO_CONTROL_FIXTURES = 262;
export const PLANNED_DEMO_PHYSICAL_INSTANCES = 301;

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
}

const PROFILES = {
  movingProfile: profile("ROBE", "Robin DLS Profile", "Mode 3", "robe--robin-dls-profile.toskfixture"),
  wash: profile("JB-Lighting", "JBLED A7", "Standard RGB 16 Bit (S16)", "jb-lighting--jbled-a7.toskfixture"),
  led: profile("Generic", "RGBW LED", "DRGBW 8-bit dimmer first", "generic--rgbw-led.toskfixture"),
  staticProfile: profile("Generic", "Dimmer Profile", "8-bit", "generic--dimmer-profile.toskfixture"),
  fresnel: profile("Generic", "Dimmer Fresnel", "8-bit", "generic--dimmer-fresnel.toskfixture"),
  sunstrip: profile("Showtec", "Sunstrip LED RGB 42206", "30 Channel", "showtec--sunstrip-led-rgb-42206.toskfixture"),
  acl: profile("Generic", "ACL", "8-bit", "generic--acl.toskfixture"),
  blinder: profile("Generic", "Blinder", "Two channel, four blind", "generic--blinder.toskfixture"),
  hazer: profile("Generic", "Hazer", "Fan, Fog", "generic--hazer.toskfixture"),
  dimmer: profile("Generic", "Dimmer", "8-bit", "generic--dimmer.toskfixture"),
} as const;

export const PLANNED_DEMO_FIXTURES: readonly DemoFixtureManifestEntry[] = [
  ...family("profile", "stage", 101, 28, PROFILES.movingProfile, "Profile Stage"),
  ...family("profile", "audience", 129, 20, PROFILES.movingProfile, "Profile Audience"),
  ...family("profile", "audience", 149, 2, PROFILES.movingProfile, "Profile Audience", ["Follow Spots"]),
  ...family("profile", "aux", 151, 4, PROFILES.movingProfile, "Profile Aux"),
  ...family("wash", "stage", 201, 26, PROFILES.wash, "Wash Stage"),
  ...family("wash", "audience", 227, 16, PROFILES.wash, "Wash Audience"),
  ...family("wash", "aux", 243, 4, PROFILES.wash, "Wash Aux"),
  ...family("led", "stage", 301, 16, PROFILES.led, "LED Stage"),
  ...family("led", "audience", 317, 100, PROFILES.led, "LED Audience"),
  ...family("led", "aux", 417, 16, PROFILES.led, "LED Aux"),
  ...namedRange(11, 5, "Static Profile", PROFILES.staticProfile, ["Front Lights"]),
  ...namedRange(1, 8, "Fresnel", PROFILES.fresnel, ["Front Lights"]),
  ...namedRange(501, 8, "Sunstrip", PROFILES.sunstrip, ["Sunstrips"]),
  ...[
    ["Back Centre ACL", "ACL 1"],
    ["Back Split ACL", "ACL 2"],
    ["Mid Split ACL", "ACL 3"],
    ["Front Split ACL", "ACL 4"],
  ].map(([name, role], index) => fixture(601 + index, name, PROFILES.acl, [role, "All ACLs"], 7)),
  ...namedRange(701, 2, "Blinder", PROFILES.blinder, ["Blinders"]),
  ...namedRange(801, 2, "Hazer", PROFILES.hazer, ["Hazers"]),
  fixture(901, "House Lights", PROFILES.dimmer, ["House Lights"], 11),
];

export const PLANNED_DEMO_FIRST_LEVEL_GROUPS = [
  "Profile All", "Profile Stage", "Profile Audience", "Profile Aux",
  "Wash All", "Wash Stage", "Wash Audience", "Wash Aux",
  "LED All", "LED Stage", "LED Audience", "LED Aux",
] as const;

export function plannedDemoRoleNumbers(role: string): number[] {
  return PLANNED_DEMO_FIXTURES
    .filter((entry) => entry.roles.includes(role))
    .map((entry) => entry.number);
}

export function plannedDemoFamilyNumbers(familyName: DemoFamily, location?: DemoLocation): number[] {
  return PLANNED_DEMO_FIXTURES
    .filter((entry) => entry.family === familyName && (location == null || entry.location === location))
    .map((entry) => entry.number);
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
    fixture(first + index, `${familyLabel} ${placeLabel} ${index + 1}`, fixtureProfile, [
      `${familyLabel} All`, locationRole, ...extraRoles,
    ], 0, familyName, location));
}

function namedRange(
  first: number,
  quantity: number,
  label: string,
  fixtureProfile: DemoFixtureManifestEntry["profile"],
  roles: readonly string[],
): DemoFixtureManifestEntry[] {
  return Array.from({ length: quantity }, (_, index) =>
    fixture(first + index, `${label} ${index + 1}`, fixtureProfile, roles));
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
  return { number, name, profile: fixtureProfile, family: familyName, location, roles, multipatches };
}

function profile(manufacturer: string, name: string, mode: string, archive: string) {
  return { manufacturer, name, mode, archive };
}

function title(value: string) {
  return value[0].toUpperCase() + value.slice(1);
}
