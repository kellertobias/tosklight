import {
	closeColorModal,
	createColorIntentShow,
	openFullColorModal,
	SEMANTIC_COLOR_GATE,
	semanticPagesPublished,
} from "./bench/color/semanticColorScenario";
import { expect, test } from "./bench/core/fixtures";
import { requireSemanticContract } from "./bench/core/semanticContract";
import { recordPreset } from "./bench/dynamics/intentFrameScenario";

/**
 * TL-647: the semantic Color Special Dialog programs a live Group selection. Groups keep their
 * live membership, so the dialog's edit is the Group's own Color value (`group_values`), and
 * Record captures it into a universal Color preset (docs/help 03-groups-and-presets.md and
 * 05-color-intent.md). A hue touched on open white keeps it white; the Saturation that follows
 * gives it the hue the dialog shows.
 */

const WIDE = { width: 1496, height: 761 };
const GROUP = "5";

interface SemanticColorValue {
	kind: "color_program";
	value: { kind: "semantic"; intent: { recipe: { rgb: [number, number, number] } } };
}

interface GroupValue {
	group_id: string;
	attribute: string;
	value: SemanticColorValue;
}

/** Hue (degrees) and saturation (0..1) of a requested recipe, as the dialog derives them. */
function hueSaturation([red, green, blue]: readonly number[]) {
	const max = Math.max(red, green, blue);
	const chroma = max - Math.min(red, green, blue);
	if (chroma === 0) return { hue: 0, saturation: 0 };
	const sector =
		max === red
			? ((green - blue) / chroma + 6) % 6
			: max === green
				? (blue - red) / chroma + 2
				: (red - green) / chroma + 4;
	return { hue: sector * 60, saturation: chroma / max };
}

test.describe("TL-647 Color Special Dialog on a live Group selection", () => {
	test("@ui › hue then Saturation on a live Group programs the Group's colour with the shown hue, and Record stores it", async ({
		api,
		desk,
		page,
	}) => {
		test.setTimeout(90_000);
		await page.setViewportSize(WIDE);
		const show = await createColorIntentShow(
			api,
			page,
			desk,
			"TL-647 live group",
			[1, 2, 3].map((number) => ({
				number,
				name: `RGB ${number}`,
				manufacturer: "Generic",
				profile: "RGB LED",
				mode: "RGB virtual dimmer",
				address: `1.${(number - 1) * 10 + 1}`,
			})),
		);
		const members = [show.ids[1], show.ids[2], show.ids[3]];
		requireSemanticContract(await semanticPagesPublished(api, members), SEMANTIC_COLOR_GATE);
		for (const command of ["FIXTURE 1 THRU 3", `RECORD GROUP ${GROUP}`])
			expect(await api.executeCommandLineRaw(command), command).toMatchObject({ outcome: "accepted" });
		// `GROUP 5` on the desk's one shared command line selects the Group's live membership.
		expect(await api.executeCommandLineRaw(`GROUP ${GROUP}`)).toMatchObject({ outcome: "accepted" });
		const [programmer] = await api.request<
			Array<{ selection_expression: unknown; values: unknown[] }>
		>("GET", "/api/v2/programmers");
		expect(programmer.selection_expression).toMatchObject({ type: "live_group", group_id: GROUP });
		expect(programmer.values).toEqual([]);

		await desk.open(api.baseUrl);
		const layer = await openFullColorModal(page);
		const ring = layer.getByRole("slider", { name: "Hue" });
		const box = await ring.boundingBox();
		if (!box) throw new Error("The hue ring is not visible");
		await page.mouse.click(box.x + box.width * 0.78, box.y + box.height * 0.3);
		const touchedHue = Number(await ring.getAttribute("aria-valuenow"));
		expect(touchedHue, "the touch picks a hue well away from red").toBeGreaterThan(20);
		expect(touchedHue).toBeLessThan(340);
		const saturation = layer.getByRole("slider", { name: "Saturation", exact: true });
		await saturation.focus();
		await page.keyboard.press("End");
		await expect(saturation).toHaveAttribute("aria-valuenow", "100");
		await expect(ring).toHaveAttribute("aria-valuenow", String(touchedHue));

		// The edit is the Group's own Color value, not values of its members.
		const groupColor = async () => {
			const snapshot = await api.request<{
				projection: { fixture_values: unknown[]; group_values: GroupValue[] };
			}>("GET", "/api/v2/programmer/values/snapshot");
			expect(snapshot.projection.fixture_values).toEqual([]);
			const value = snapshot.projection.group_values.find(
				(entry) => entry.group_id === GROUP && entry.attribute === "color",
			);
			return value ? hueSaturation(value.value.value.intent.recipe.rgb) : null;
		};
		await expect.poll(async () => (await groupColor())?.saturation ?? 0).toBeCloseTo(1, 3);
		const programmed = await groupColor();
		expect(
			Math.abs((programmed?.hue ?? -360) - touchedHue),
			`programmed ${JSON.stringify(programmed)}, shown hue ${touchedHue}`,
		).toBeLessThan(1);
		await closeColorModal(layer);

		// Record stores that colour as one universal Color preset.
		const recorded = await recordPreset(api, { showId: show.id, ids: show.ids }, "Color", 7);
		expect(recorded.status).toBe("changed");
		const preset = await api.showObject<{
			universal_values?: { color?: SemanticColorValue };
		}>(show.id, "preset", "2.7");
		const stored = preset?.body.universal_values?.color;
		if (!stored)
			throw new Error(`Color 7 holds no universal colour: ${JSON.stringify(preset?.body)}`);
		const storedColor = hueSaturation(stored.value.intent.recipe.rgb);
		expect(storedColor.saturation).toBeCloseTo(1, 3);
		expect(Math.abs(storedColor.hue - touchedHue)).toBeLessThan(1);
	});
});
