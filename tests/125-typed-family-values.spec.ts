import type { Page } from "@playwright/test";
import type { ApiDriver } from "./bench/core/api";
import type { DeskDriver } from "./bench/core/desk";
import { expect, test } from "./bench/core/fixtures";
import type { LightBench } from "./bench/core/lightBench";
import { requireSemanticContract } from "./bench/core/semanticContract";
import { BrowserPatch } from "./bench/show-setup/patchScenario";

/**
 * docs/testing/38-typed-family-values.md (TL-544 G3): typed Color, Position and Focus/Zoom values
 * from the command line, the desk keypad and OSC. Each value is the encoders' semantic family
 * edit, so the Programmer stores Color Intent, Angles and degrees, and DMX follows the fixtures'
 * own models.
 *
 * Rig: three Generic RGB LED (RGB virtual dimmer) at 1.1, 1.11 and 1.21, and one Cameo AURO SPOT
 * Z300 (20-Channel) at 1.31, whose profile carries a Position physical graph and a beam Zoom.
 */

const GATE =
	"semantic programming contract is not enabled on this runtime (a contract-0 server; run npm run test:e2e-semantic)";

const RGB = { manufacturer: "Generic", profile: "RGB LED", mode: "RGB virtual dimmer" } as const;
const SPOT = { manufacturer: "Cameo", profile: "AURO SPOT Z300", mode: "20-Channel" } as const;
const RGB_ADDRESSES = [1, 11, 21] as const;

type Scalar = { kind: "value"; value: number } | { kind: "spread"; value: number[] };
type FamilyValue = {
	fixture_id: string;
	attribute: string;
	value: { kind: string; value: Record<string, unknown> };
};

interface Rig {
	ids: Record<number, string>;
}

async function arrange(
	{ api, page, desk }: { api: ApiDriver; page: Page; desk: DeskDriver },
	label: string,
): Promise<Rig> {
	await api.request("PUT", "/api/v2/configuration", { programmer_fade_millis: 0 });
	const show = await api.createShow<{ id: string }>({ name: `TYPED-FAMILY ${label} ${crypto.randomUUID()}` });
	await api.openShow(show.id, { transition: "hold_current" });
	const patch = new BrowserPatch(api, page, desk);
	for (const [index, address] of RGB_ADDRESSES.entries())
		await patch.via.api.add({ number: index + 1, name: `RGB ${index + 1}`, ...RGB, address: `1.${address}` });
	await patch.via.api.add({ number: 4, name: "Spot", ...SPOT, address: "1.31" });
	const ids = Object.fromEntries(
		(await api.patch()).fixtures.map((fixture) => [fixture.fixture_number, fixture.fixture_id]),
	);
	const pages = await api
		.request<{ semantic: boolean }>(
			"GET",
			`/api/v2/programming/family-encoder-pages?fixture_ids=${Object.values(ids).join(",")}`,
		)
		.catch(() => null);
	requireSemanticContract(Boolean(pages?.semantic), GATE);
	return { ids };
}

async function familyValues(api: ApiDriver): Promise<FamilyValue[]> {
	const snapshot = await api.request<{ projection: { fixture_values?: FamilyValue[] } }>(
		"GET",
		"/api/v2/programmer/values/snapshot",
	);
	return snapshot.projection.fixture_values ?? [];
}

function scalar(value: unknown): number | null {
	const typed = value as Scalar | undefined;
	return typed?.kind === "value" ? typed.value : null;
}

/** The programmed Angles of one fixture, or null while it holds none. */
async function angles(api: ApiDriver, fixtureId: string) {
	const entry = (await familyValues(api)).find(
		(value) => value.fixture_id === fixtureId && value.attribute === "position",
	);
	const intent = entry?.value.value as
		| { kind: string; pan_degrees?: unknown; tilt_degrees?: unknown }
		| undefined;
	if (intent?.kind !== "angles") return null;
	return { pan: scalar(intent.pan_degrees), tilt: scalar(intent.tilt_degrees) };
}

/** The programmed Zoom opening of one fixture in degrees, or null. */
async function zoom(api: ApiDriver, fixtureId: string) {
	const entry = (await familyValues(api)).find(
		(value) => value.fixture_id === fixtureId && value.attribute === "zoom",
	);
	return scalar((entry?.value.value as { opening_degrees?: unknown } | undefined)?.opening_degrees);
}

async function rgbDmx(bench: LightBench, fixture: 1 | 2 | 3) {
	const frame = await bench.tick(0);
	const slots = frame.universes.find((entry) => entry.universe === 1)?.slots ?? [];
	const start = RGB_ADDRESSES[fixture - 1] - 1;
	return { red: slots[start], green: slots[start + 1], blue: slots[start + 2] };
}

async function accepted(api: ApiDriver, command: string) {
	const response = await api.executeCommandLineRaw(command);
	expect(response.outcome, `${command}: ${response.error ?? ""}`).toBe("accepted");
}

test.describe("docs/testing/38-typed-family-values.md", () => {
	test("TYPED-FAMILY-001 @api › typed Color tuples spread over the selection and reach DMX", async ({
		api,
		bench,
		page,
		desk,
	}) => {
		await arrange({ api, page, desk }, "001");
		await accepted(api, "FIXTURE 1 THRU 3 AT 100");
		// The help's example: red via yellow to green over the ordered selection.
		await accepted(api, "FIXTURE 1 THRU 3 AT COLOR 100 DIV 0 DIV 0 THRU 0 DIV 100 DIV 0");
		await expect.poll(() => rgbDmx(bench, 1)).toEqual({ red: 255, green: 0, blue: 0 });
		await expect.poll(() => rgbDmx(bench, 3)).toEqual({ red: 0, green: 255, blue: 0 });
		const middle = await rgbDmx(bench, 2);
		expect(middle.red).toBeGreaterThan(0);
		expect(middle.green).toBeGreaterThan(0);
		expect(middle.blue).toBe(0);
		const colors = (await familyValues(api)).filter((value) => value.attribute === "color");
		expect(colors).toHaveLength(3);
		for (const value of colors) expect(value.value.kind).toBe("color_program");
		expect((await familyValues(api)).filter((value) => value.attribute.startsWith("color."))).toEqual([]);

		// Empty components stay; the keypad's doubled DIV (OFFSET) still separates two values.
		await accepted(api, "FIXTURE 1 AT COLOR OFFSET 100");
		await expect.poll(() => rgbDmx(bench, 1)).toEqual({ red: 255, green: 0, blue: 255 });

		// Invalid values are rejected and change nothing.
		const before = await familyValues(api);
		for (const command of [
			"FIXTURE 1 AT COLOR 150",
			"FIXTURE 1 AT COLOR 1 DIV 2 DIV 3 DIV 4 DIV 5",
			"FIXTURE 1 THRU 3 AT COLOR + 10 THRU 20",
		]) {
			const response = await api.executeCommandLineRaw(command);
			expect(response.outcome, command).toBe("rejected");
		}
		expect(await familyValues(api)).toEqual(before);
	});

	test("TYPED-FAMILY-002 @api › Position angles and Zoom degrees from the command line and the desk keypad", async ({
		api,
		page,
		desk,
	}) => {
		const { ids } = await arrange({ api, page, desk }, "002");
		await accepted(api, "FIXTURE 4 AT POSITION 45 DIV - - 30");
		await expect.poll(() => angles(api, ids[4])).toEqual({ pan: 45, tilt: -30 });
		await accepted(api, "FIXTURE 4 AT POSITION + 15");
		await expect.poll(() => angles(api, ids[4])).toEqual({ pan: 60, tilt: -30 });
		await accepted(api, "FIXTURE 4 AT FOCUS DIV 20");
		await expect.poll(() => zoom(api, ids[4])).toBe(20);

		// The desk keypad: 4 [AT] [^3] 90 [DIV] 10 [ENT] shows `F4 AT POSITION 90 DIV 10`.
		await api.sendCommandKey("ESC");
		for (const key of ["4", "AT"] as const) await api.sendCommandKey(key);
		await api.sendCommandKey("SHIFT", "press");
		await api.sendCommandKey("3");
		await api.sendCommandKey("SHIFT", "release");
		for (const key of ["9", "0", "DIV", "1", "0"] as const) await api.sendCommandKey(key);
		await expect
			.poll(async () => (await api.getCommandLine()).commandLine.text)
			.toBe("F4 AT POSITION 90 DIV 10");
		await api.sendCommandKey("ENT");
		await expect.poll(() => angles(api, ids[4])).toEqual({ pan: 90, tilt: 10 });

		// A Color value on a fixture without Color is not applicable: nothing changes, no error.
		await accepted(api, "FIXTURE 4 AT COLOR 100");
		expect((await familyValues(api)).some((value) => value.fixture_id === ids[4] && value.attribute === "color")).toBe(false);
	});

	test("TYPED-FAMILY-003 @api › OSC family writes and the OSC keypad share the command-line value", async ({
		api,
		bench,
		page,
		desk,
	}) => {
		const { ids } = await arrange({ api, page, desk }, "003");
		const hardware = await bench.osc();
		const clientId = `typed-family-${crypto.randomUUID()}`;
		await hardware.subscribe(clientId, "desk");
		try {
			await accepted(api, "FIXTURE 4");
			await hardware.send("/light/desk/programmer/family/pan", [-60.5]);
			await hardware.send("/light/desk/programmer/family/tilt", [20]);
			await expect.poll(() => angles(api, ids[4])).toEqual({ pan: -60.5, tilt: 20 });
			await hardware.send("/light/desk/programmer/family/zoom", [30]);
			await expect.poll(() => zoom(api, ids[4])).toBe(30);

			await accepted(api, "FIXTURE 1 THRU 3 AT 100");
			await hardware.send("/light/desk/programmer/family/red", [0, 100]);
			await hardware.send("/light/desk/programmer/family/green", [0, 0]);
			await hardware.send("/light/desk/programmer/family/blue", [100, 0]);
			await expect.poll(() => rgbDmx(bench, 1)).toEqual({ red: 0, green: 0, blue: 255 });
			await expect.poll(() => rgbDmx(bench, 3)).toEqual({ red: 255, green: 0, blue: 0 });

			// A rejected write reports on the desk's feedback path and changes nothing.
			const before = await familyValues(api);
			const mark = hardware.mark();
			await hardware.send("/light/desk/programmer/family/red", [140]);
			const error = await hardware.expectAfter(mark, "/light/desk/feedback/programmer/error");
			expect(error.arguments[0]).toBe("/light/desk/programmer/family/red");
			expect(await familyValues(api)).toEqual(before);

			// The OSC keypad builds the same command: 4 [AT] [^7] [DIV] 12 [ENT].
			for (const action of ["escape", "digit-4", "at"]) {
				await hardware.send(`/light/desk/programmer/${action}`, [true]);
				await hardware.send(`/light/desk/programmer/${action}`, [false]);
			}
			await hardware.send("/light/desk/programmer/shift", [true]);
			await hardware.send("/light/desk/programmer/digit-7", [true]);
			await hardware.send("/light/desk/programmer/digit-7", [false]);
			await hardware.send("/light/desk/programmer/shift", [false]);
			for (const action of ["div", "digit-1", "digit-2"]) {
				await hardware.send(`/light/desk/programmer/${action}`, [true]);
				await hardware.send(`/light/desk/programmer/${action}`, [false]);
			}
			await expect
				.poll(async () => (await api.getCommandLine()).commandLine.text)
				.toBe("F4 AT FOCUS DIV 12");
			await hardware.send("/light/desk/programmer/enter", [true]);
			await hardware.send("/light/desk/programmer/enter", [false]);
			await expect.poll(() => zoom(api, ids[4])).toBe(12);
		} finally {
			await hardware.send("/light/unsubscribe", [clientId]).catch(() => undefined);
			await hardware.close();
		}
	});
});
