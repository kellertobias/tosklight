import type { Page } from "@playwright/test";
import type { ApiDriver } from "./bench/core/api";
import {
	acceptedReport,
	closeDialog,
	colorFamily,
	colorOf,
	colorValues,
	createDirectShow,
	detent,
	directEditOf,
	directOf,
	dmx,
	encoder,
	loudFeedback,
	nativeEdit,
	nativePages,
	openColorDialog,
	openDirectColor,
	pageColorTo,
	programmerRevision,
	programSemantic,
	recordColorActions,
	SEMANTIC_BLUE,
	SEMANTIC_GATE,
	SEMANTIC_GREEN,
	SEMANTIC_RED,
	select,
	semanticPublished,
	showColor,
	usePresentation,
	valuesAction,
	whiteBlendEdit,
} from "./bench/color/directColorRig";
import { expect, test } from "./bench/core/fixtures";
import { requireSemanticContract } from "./bench/core/semanticContract";

/**
 * docs/testing/37-direct-color-pages.md (TL-554), DIRECT-COLOR-002 to 006. DIRECT-COLOR-001
 * lives in tests/112-color-intent.spec.ts; this spec uses the same rig (two verified ROOT PAR 6
 * heads A1/A2 and a verified Lustr B, numbers 101-103) through tests/bench/color/directColorRig.ts,
 * plus a verified Martin ELP CL (E, 104) whose emitters have no measured appearance.
 *
 * The desk uses the Advanced Color presentation, so pages 1 and 2 are the semantic pages and
 * the Direct (native) controls start on page 3, as the scenario numbers them. On the six-encoder
 * desk, page 3 shows the first six native controls of the reference head.
 */

test.use({ viewport: { width: 1600, height: 1000 } });

type Channel = { channel_id: string; raw: number };

const byChannel = (channels: readonly Channel[]) =>
	channels
		.map((channel) => ({ channel_id: channel.channel_id, raw: channel.raw }))
		.sort((left, right) => left.channel_id.localeCompare(right.channel_id));

test.describe("docs/testing/37-direct-color-pages.md", () => {
	test("DIRECT-COLOR-002 @ui › the full Color dialog names the reference head, lists the overflow and follows a new reference inertly", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		test.setTimeout(90_000);
		const show = await createDirectShow(api, page, desk, { overflow: true });
		const [x1, x2] = [show.ids[201], show.ids[202]];
		const selection = [x1, x2];
		requireSemanticContract(await semanticPublished(api, selection), SEMANTIC_GATE);
		await usePresentation(api, "advanced");
		await select(api, show, selection);
		await programSemantic(api, selection, SEMANTIC_RED);
		await bench.tick(25);

		// Eleven native controls: eight on pages 3/4, three beyond them (the overflow).
		const pages = await nativePages(api, selection);
		expect(pages.pages.flatMap((entry) => entry.controls).filter(Boolean)).toHaveLength(8);
		expect(pages.overflow.map((control) => control.label)).toEqual(["White", "Color Scene", "Color Wheel 1"]);

		const sent = recordColorActions(page);
		await desk.open(api.baseUrl);
		await pageColorTo(page, 3);
		await expect(encoder(page, 1)).toHaveAccessibleName(/^Enc 1 · Color Temperature · 201\b/);
		const revision = await programmerRevision(api);

		// Step 1-2: Expand; the reference head is clearly identified and every overflow control is
		// a touch encoder, with the wheel and macro functions listed as choices.
		const dialog = await openDirectColor(page);
		const direct = dialog.getByTestId("color-direct");
		await expect(direct.getByTestId("color-direct-reference")).toHaveText("Reference: 201 · Overflow X1 · Main");
		const overflow = direct.getByTestId("color-direct-overflow");
		await expect(overflow.getByRole("group")).toHaveCount(3);
		expect(await overflow.getByRole("group").evaluateAll((groups) => groups.map((group) => group.getAttribute("aria-label")))).toEqual([
			"Native 1 · White",
			"Native 2 · Color Scene",
			"Native 3 · Color Wheel 1",
		]);
		await expect(overflow.getByRole("list", { name: "Color Scene functions" }).getByRole("listitem")).toHaveText(["Color Scene"]);
		const wheel = overflow.getByRole("list", { name: "Color Wheel 1 functions" }).getByRole("listitem");
		await expect(wheel).toHaveCount(13);
		await expect(wheel.first()).toHaveText("Open");
		await expect(wheel.nth(1)).toHaveText("Deep Red");

		// Step 3: choosing another reference head is inert; the section and pages 3/4 follow it.
		const candidates = direct.getByRole("group", { name: "Reference head" });
		const second = candidates.getByRole("button", { name: "202 · Overflow X2 · Main", exact: true });
		await second.click();
		await expect(second).toHaveAttribute("aria-pressed", "true");
		await expect(direct.getByTestId("color-direct-reference")).toHaveText("Reference: 202 · Overflow X2 · Main");
		await page.waitForTimeout(300);
		expect(await programmerRevision(api), "choosing a reference sends nothing to the Programmer").toBe(revision);
		expect(sent).toEqual([]);

		// Step 4: an overflow control sends the same Direct edit as an encoder of pages 3/4.
		const chosen = await nativePages(api, selection, x2);
		const reference = chosen.reference;
		const shown = chosen.values?.controls ?? [];
		const white = pages.overflow[0];
		const red = pages.pages[0].controls[1];
		if (!reference || !red) throw new Error("the reference head has no Red control");
		await detent(overflow.getByRole("group", { name: "Native 1 · White" }), -1);
		await expect.poll(() => sent.length).toBe(1);
		await closeDialog(dialog);
		await expect(colorFamily(page)).toHaveAccessibleName("Color 3 of 4");
		await expect(encoder(page, 2)).toHaveAccessibleName(/^Enc 2 · Red · 202\b/);
		await detent(encoder(page, 2), 1);
		await expect.poll(() => sent.length).toBe(2);
		const [fromOverflow, fromPage] = sent.map(directEditOf);
		const expected = (control: typeof red, value: number) => ({
			type: "apply_intent",
			fixture_ids: selection,
			group_id: null,
			attribute: "color",
			operation: {
				type: "component_edits",
				edits: [
					{
						kind: "native",
						binding: { channel_id: control.channel_id, function_id: control.functions[0].function_id },
						operation: { kind: "relative", value },
					},
				],
			},
			timing: fromPage.timing,
			native_reference: { fixture_id: x2, head_id: reference.head_id },
		});
		expect(fromOverflow).toEqual(expected(white, -1));
		expect(fromPage).toEqual(expected(red, 1));
		const raw = (channel: string) => shown.find((entry) => entry.channel_id === channel)?.raw ?? 0;
		const edited = async () =>
			(await colorValues(api)).map((value) => {
				const channels = value.value.value.recipe?.channels ?? [];
				const of = (id: string) => channels.find((entry) => entry.channel_id === id)?.raw;
				return [value.fixture_id, value.value.value.kind, of(white.channel_id), of(red.channel_id)];
			});
		await expect
			.poll(edited)
			.toEqual(
				selection.map((id) => [
					id,
					"direct",
					Math.max(0, raw(white.channel_id) - 1),
					Math.min(red.raw_max, raw(red.channel_id) + 1),
				]),
			);

		// Step 5 (TL-544 G4): touching a wheel choice selects that slot as one complete recipe
		// adoption; every other control is kept, and the choice is now the current one. A detent of
		// the wheel's encoder then moves exactly one choice, in profile order.
		const wheelControl = pages.overflow[2];
		const before = await colorOf(api, x1);
		const reopened = await openDirectColor(page);
		const choices = reopened
			.getByTestId("color-direct-overflow")
			.getByRole("list", { name: "Color Wheel 1 functions" });
		await choices.getByRole("button", { name: "Deep Red", exact: true }).click();
		await expect.poll(() => sent.length).toBe(3);
		const deepRed = wheelControl.functions[1];
		expect(directEditOf(sent[2]).operation.edits).toEqual([
			{
				kind: "native",
				binding: { channel_id: wheelControl.channel_id, function_id: deepRed.function_id },
				operation: { kind: "set", value: Math.min(deepRed.raw_from, deepRed.raw_to) },
			},
		]);
		await expect(choices.getByRole("button", { name: "Deep Red", exact: true })).toHaveAttribute("aria-pressed", "true");
		const after = await colorOf(api, x1);
		const others = (value: typeof after) =>
			byChannel((value?.recipe?.channels ?? []).filter((entry) => entry.channel_id !== wheelControl.channel_id));
		expect(others(after), "every other control is kept").toEqual(others(before));
		await detent(reopened.getByTestId("color-direct-overflow").getByRole("group", { name: "Native 3 · Color Wheel 1" }), 1);
		await expect.poll(() => sent.length).toBe(4);
		expect(directEditOf(sent[3]).operation.edits[0].binding.function_id).toBe(wheelControl.functions[2].function_id);
		await closeDialog(reopened);
	});

	test("DIRECT-COLOR-003 @ui › Native replay versus Best-effort match, and Native only for an unknown appearance, all passive", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		test.setTimeout(90_000);
		const show = await createDirectShow(api, page, desk);
		const [a1, a2, b, e] = [show.ids[101], show.ids[102], show.ids[103], show.ids[104]];
		const mixed = [a1, a2, b];
		requireSemanticContract(await semanticPublished(api, mixed), SEMANTIC_GATE);
		await usePresentation(api, "advanced");
		await select(api, show, mixed);
		await programSemantic(api, [...mixed, e], SEMANTIC_RED);
		await bench.tick(25);

		// DIRECT-COLOR-001 step 4: the first native edit, with A1 the reference head.
		const pages = await nativePages(api, mixed);
		const control = pages.pages[0].controls[0];
		if (!control || !pages.reference) throw new Error("A1 has no native controls");
		const first = await valuesAction(
			api,
			nativeEdit(mixed, control, -10, { reference: { fixture_id: a1, head_id: pages.reference.head_id } }),
		);
		expect(first.status, JSON.stringify(first)).toBe("changed");
		const report = await acceptedReport(api, bench, mixed);
		expect(directOf(report, a1)?.replay).toBe("exact");
		expect(directOf(report, a2)?.replay).toBe("exact");
		expect(directOf(report, b)).toMatchObject({ replay: "fallback", compatibility: "different_source" });

		// Step 1-2: the dialog reads Native replay / Best-effort match; UV has its own wording.
		await desk.open(api.baseUrl);
		const dialog = await openDirectColor(page);
		const focused = await focusedElement(page);
		const status = dialog.getByTestId("color-direct-status");
		const row = (name: string) => status.getByRole("row").filter({ has: page.getByRole("rowheader", { name: new RegExp(`· ${name}\\b`) }) });
		await expect(row("Par A1").getByRole("cell").first()).toHaveText("Native replay");
		await expect(row("Par A2").getByRole("cell").first()).toHaveText("Native replay");
		await expect(row("Lustr B").getByRole("cell").first()).toHaveText("Best-effort match");
		await expect(row("Lustr B").getByRole("cell").nth(1)).toContainText("different fixture type");
		for (const name of ["Par A1", "Par A2", "Lustr B"]) {
			await expect(row(name).getByRole("cell").first(), "UV is described in its own detail, never in the replay").not.toContainText("UV");
			await expect(row(name), "an exact native replay is never an exact colour").not.toContainText(/exact colou?r/i);
		}
		const a1Uv = directOf(report, a1)?.uv;
		if (a1Uv === "apply") await expect(row("Par A1").getByRole("cell").nth(1)).toContainText("UV applied");
		if (a1Uv === "park_off") await expect(row("Par A1").getByRole("cell").nth(1)).toContainText("UV parked off");
		// The approximation shows the measured match of the output, beside the Direct status.
		await expect(dialog.getByTestId("color-approximation")).toBeVisible();

		// Step 3: a Direct recipe whose appearance is unknown on B (E's uncalibrated layout):
		// Native only; B keeps its visible colour, nothing white is invented, UV parks off.
		const before = await dmx(bench, 1, 121, 8);
		const elp = await nativePages(api, [e, b]);
		const elpRed = elp.pages[0].controls[1];
		if (!elpRed || !elp.reference) throw new Error("E has no native Red");
		const unknown = await valuesAction(
			api,
			nativeEdit([e, b], elpRed, 5, { reference: { fixture_id: e, head_id: elp.reference.head_id } }),
		);
		expect(unknown.status, JSON.stringify(unknown)).toBe("changed");
		const after = await acceptedReport(api, bench, mixed);
		expect(directOf(after, b)).toMatchObject({ replay: "native_only", uv: "park_off" });
		expect(await dmx(bench, 1, 121, 8), "B keeps its visible output").toEqual(before);
		await expect(row("Lustr B").getByRole("cell").first()).toHaveText("Native only · appearance unknown");
		await expect(row("Lustr B").getByRole("cell").nth(1)).toContainText("UV parked off");

		// Step 4: passive throughout: no toast, alert, focus move or other modal.
		await expect(loudFeedback(page)).toHaveCount(0);
		await expect(page.getByRole("dialog")).toHaveCount(1);
		expect(await focusedElement(page), "nothing moved focus").toBe(focused);
	});

	test("DIRECT-COLOR-004 @ui › the first semantic edit of a Direct value starts from its appearance, else from an explicit start", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		test.setTimeout(90_000);
		const show = await createDirectShow(api, page, desk);
		const [a1, a2, e] = [show.ids[101], show.ids[102], show.ids[104]];
		requireSemanticContract(await semanticPublished(api, [a1]), SEMANTIC_GATE);
		await usePresentation(api, "advanced");
		await select(api, show, [a1]);
		await programSemantic(api, [a1, a2, e], SEMANTIC_RED);
		await bench.tick(25);

		// A1 holds a half-level red Direct recipe (Red 255 → 127): a known appearance.
		const pages = await nativePages(api, [a1, a2]);
		const red = pages.pages[0].controls[0];
		if (!red || !pages.reference) throw new Error("A1 has no native Red");
		const head = pages.reference.head_id;
		await valuesAction(api, nativeEdit([a1], red, -128, { reference: { fixture_id: a1, head_id: head } }));
		const recipe = (await colorOf(api, a1)) as unknown as {
			kind: string;
			portable: { visible: { xyz: { x: number; y: number; z: number } } };
		};
		expect(recipe.kind).toBe("direct");
		const visible = recipe.portable.visible.xyz;
		expect(visible.y, "a half-level recipe").toBeCloseTo(0.2126729 / 2, 2);
		await bench.tick(25);

		// Step 1-2: turn White Blend on page 1.
		await desk.open(api.baseUrl);
		await pageColorTo(page, 1);
		const whiteBlend = page.locator(".parameter-surfaces").getByRole("group", { name: /^Enc \d+ · White Blend/ });
		await detent(whiteBlend, 1);
		await expect.poll(async () => (await colorOf(api, a1))?.kind).toBe("semantic");
		const adopted = await colorOf(api, a1);
		for (const axis of ["x", "y", "z"] as const)
			expect(adopted?.intent.base_xyz[axis], "starts from the recipe's appearance: half level stays half level").toBeCloseTo(
				visible[axis],
				6,
			);
		expect(adopted?.intent.white_blend).toBeGreaterThan(0);
		let dialog = await openColorDialog(page);
		await expect(dialog.getByTestId("color-adoption-report")).toHaveText("Started from an approximation of the Direct colour.");
		await closeDialog(dialog);

		// Black stays black (A2: every native control at 0), through the integrator path.
		await valuesAction(api, nativeEdit([a2], red, -255, { reference: { fixture_id: a2, head_id: head } }));
		const black = await valuesAction(api, whiteBlendEdit([a2], 0.25));
		expect(black.color_adoption).toMatchObject({ fixtures: [{ fixture_id: a2, start: "approximate", uv_unknown: false }] });
		expect((await colorOf(api, a2))?.intent.base_xyz).toEqual({ x: 0, y: 0, z: 0 });

		// Step 3: A1 takes E's Direct recipe, whose appearance is unknown: the edit holds quietly.
		const elp = await nativePages(api, [e, a1]);
		const elpRed = elp.pages[0].controls[1];
		if (!elpRed || !elp.reference) throw new Error("E has no native Red");
		await valuesAction(api, nativeEdit([e, a1], elpRed, 5, { reference: { fixture_id: e, head_id: elp.reference.head_id } }));
		await expect.poll(async () => (await colorOf(api, a1))?.kind).toBe("direct");
		await bench.tick(25);
		const revision = await programmerRevision(api);
		await detent(whiteBlend, 1);
		await expect(whiteBlend).toHaveAccessibleName(/White Blend · Choose start/);
		expect(await programmerRevision(api), "nothing changes").toBe(revision);
		expect((await colorOf(api, a1))?.kind).toBe("direct");
		await expect(loudFeedback(page)).toHaveCount(0);
		dialog = await openColorDialog(page);
		const start = dialog.getByTestId("color-explicit-start");
		await expect(start).toContainText("The Direct colour's appearance is unknown. Choose a starting colour.");
		await expect(start.getByRole("button")).toHaveText(["Start from black", "Start from white"]);

		// Step 4: Start from black, turn again: the edit applies from black, UV starts off.
		await start.getByRole("button", { name: "Start from black", exact: true }).click();
		await expect(start).toContainText("Starting colour chosen. Turn the control again to apply it.");
		expect(await programmerRevision(api), "choosing a start sends nothing").toBe(revision);
		await closeDialog(dialog);
		await detent(whiteBlend, 1);
		await expect.poll(async () => (await colorOf(api, a1))?.kind).toBe("semantic");
		const explicit = await colorOf(api, a1);
		expect(explicit?.intent.base_xyz).toEqual({ x: 0, y: 0, z: 0 });
		expect(explicit?.intent.uv).toEqual({ amount: 0 });
		dialog = await openColorDialog(page);
		await expect(dialog.getByTestId("color-adoption-report")).toHaveText(
			"Started from your explicit colour. UV was unknown and starts off.",
		);
	});

	test("DIRECT-COLOR-005 @ui › hardware encode/2 and the software encoder send identical Direct edits; integrators use the first verified head and the latest frame", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		test.setTimeout(90_000);
		const show = await createDirectShow(api, page, desk);
		const [a1, a2, b] = [show.ids[101], show.ids[102], show.ids[103]];
		const mixed = [a1, a2, b];
		requireSemanticContract(await semanticPublished(api, mixed), SEMANTIC_GATE);
		await usePresentation(api, "advanced");
		await select(api, show, mixed);
		await programSemantic(api, mixed, SEMANTIC_RED);
		await bench.tick(25);
		const pages = await nativePages(api, mixed);
		const green = pages.pages[0].controls[1];
		if (!green || !pages.reference) throw new Error("A1 has no native Green");
		const reference = { fixture_id: a1, head_id: pages.reference.head_id };

		// Step 1-2: in the hardware-connected layout, page Color to 3 and send encode/2 up.
		const sent = recordColorActions(page);
		await desk.open(api.baseUrl);
		await showColor(page);
		const hardware = await bench.osc();
		const client = `direct-color-${crypto.randomUUID()}`;
		await hardware.subscribe(client, "desk");
		try {
			await expect.poll(() => hardwareConnected(api)).toBe(true);
			await pageColorTo(page, 3);
			await expect(page.locator(".hardware-encoder-display")).toHaveCount(6);
			await expect(page.getByLabel(/^Encoder 2: Green · 101\b/)).toBeVisible();
			const revision = await programmerRevision(api);
			await hardware.send("/light/desk/encode/2", ["up"]);
			await expect.poll(() => programmerRevision(api)).toBeGreaterThan(revision);
			await expect.poll(() => sent.length).toBe(1);
		} finally {
			await hardware.send("/light/unsubscribe", [client]).catch(() => undefined);
			await hardware.close();
		}

		// The same software encoder (software-only layout, page 3, Enc 2), one detent.
		await expect.poll(() => hardwareConnected(api)).toBe(false);
		await expect(colorFamily(page)).toHaveAccessibleName(/^Color 3 of \d+$/);
		await expect(encoder(page, 2)).toHaveAccessibleName(/^Enc 2 · Green · 101\b/);
		await detent(encoder(page, 2), 1);
		await expect.poll(() => sent.length).toBe(2);
		const [fromHardware, fromSoftware] = sent.map(directEditOf);
		expect(fromSoftware).toEqual(fromHardware);
		expect(fromHardware).toMatchObject({
			type: "apply_intent",
			fixture_ids: mixed,
			attribute: "color",
			native_reference: reference,
			operation: {
				type: "component_edits",
				edits: [
					{
						kind: "native",
						binding: { channel_id: green.channel_id, function_id: green.functions[0].function_id },
						operation: { kind: "relative", value: 1 },
					},
				],
			},
		});

		// Step 3: an integrator's Direct edit without a reference head uses the first verified head
		// of the ordered selection (A2 here, showing green while A1 shows red), and without a
		// displayed source the latest accepted frame.
		const ordered = [a2, a1, b];
		await programSemantic(api, [a1, b], SEMANTIC_RED);
		await programSemantic(api, [a2], SEMANTIC_GREEN);
		await bench.tick(25);
		const seen = await nativePages(api, ordered);
		expect(seen.reference?.fixture_id).toBe(a2);
		const shown = seen.values?.controls ?? [];
		expect(shown.find((entry) => entry.channel_id === green.channel_id)?.raw, "A2 shows green").toBe(255);
		const integrator = await valuesAction(api, nativeEdit(ordered, green, -1));
		expect(integrator.status, JSON.stringify(integrator)).toBe("changed");
		const nudged = (channels: readonly Channel[], delta: number) =>
			byChannel(channels.map((entry) => ({ ...entry, raw: entry.channel_id === green.channel_id ? entry.raw + delta : entry.raw })));
		const recipes = async () =>
			(await colorValues(api))
				.filter((value) => mixed.includes(value.fixture_id))
				.map((value) => [value.fixture_id, value.value.value.kind, byChannel(value.value.value.recipe?.channels ?? [])]);
		expect(await recipes()).toEqual(expect.arrayContaining(mixed.map((id) => [id, "direct", nudged(shown, -1)])));
		expect(await recipes()).toHaveLength(3);

		// The output moves on (A2 blue); the next integrator edit seeds from the latest frame.
		await programSemantic(api, [a2], SEMANTIC_BLUE);
		await bench.tick(25);
		const latest = (await nativePages(api, [a2])).values?.controls ?? [];
		expect(byChannel(latest), "the accepted frame moved on").not.toEqual(byChannel(shown));
		const moved = await valuesAction(api, nativeEdit([a2], green, 1));
		expect(moved.status, JSON.stringify(moved)).toBe("changed");
		expect(byChannel((await colorOf(api, a2))?.recipe?.channels ?? [])).toEqual(nudged(latest, 1));
	});

	test("DIRECT-COLOR-006 @ui › a native turn on idle heads starts from the shown profile defaults", async ({ api, bench, desk, page }) => {
		test.setTimeout(90_000);
		const show = await createDirectShow(api, page, desk);
		const [a1, a2] = [show.ids[101], show.ids[102]];
		requireSemanticContract(await semanticPublished(api, [a1, a2]), SEMANTIC_GATE);
		await usePresentation(api, "advanced");
		await select(api, show, [a1, a2]);
		await bench.tick(25);
		const sent = recordColorActions(page);
		await desk.open(api.baseUrl);
		await pageColorTo(page, 3);
		await expect(encoder(page, 1)).toHaveAccessibleName(/^Enc 1 · Red · 101\b/);
		// G5: an idle head shows its profile defaults, and the first detent adopts exactly those.
		const pages = await nativePages(api, [a1, a2]);
		const red = pages.pages[0].controls[0];
		const shown = pages.values?.controls ?? [];
		if (!red) throw new Error("A1 has no native Red");
		expect(shown.length, "the idle reference head shows its defaults").toBeGreaterThan(0);
		await detent(encoder(page, 1), 1);
		await expect.poll(() => sent.length).toBeGreaterThan(0);
		// One software detent moves 1/255 of the function's range (at least 1).
		const step = Math.max(1, Math.floor(Math.abs(red.functions[0].raw_to - red.functions[0].raw_from) / 255));
		// A default already at the top of the range stays there (the step is clamped).
		const top = Math.max(red.functions[0].raw_from, red.functions[0].raw_to);
		const expected = byChannel(
			shown.map((entry) => ({
				...entry,
				raw: entry.channel_id === red.channel_id ? Math.min(entry.raw + step, top) : entry.raw,
			})),
		);
		for (const fixture of [a1, a2])
			await expect.poll(async () => byChannel((await colorOf(api, fixture))?.recipe?.channels ?? [])).toEqual(expected);
		await expect(loudFeedback(page), "nothing is reported as an error").toHaveCount(0);
	});

	test("DIRECT-COLOR-006 @api › a detent on an output that moved on is held, the desk re-reads and the next detent applies", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const show = await createDirectShow(api, page, desk);
		const [a1, a2] = [show.ids[101], show.ids[102]];
		requireSemanticContract(await semanticPublished(api, [a1, a2]), SEMANTIC_GATE);
		await select(api, show, [a1]);
		await programSemantic(api, [a1], SEMANTIC_RED);
		await bench.tick(25);
		const pages = await nativePages(api, [a1]);
		const red = pages.pages[0].controls[0];
		if (!red || !pages.reference) throw new Error("A1 has no native Red");
		const reference = { fixture_id: a1, head_id: pages.reference.head_id };
		const read = async () =>
			(await api.request<{ lease: number }>("GET", `/api/v2/output/readouts?lane=normal&fixture_ids=${a1}`)).lease;
		const shownLease = await read();
		// The displayed output moves on, more often than the desk's lease ring holds frames.
		for (let frame = 0; frame < 12; frame += 1) {
			await programSemantic(api, [a2], frame % 2 ? SEMANTIC_RED : SEMANTIC_GREEN);
			await bench.tick(25);
			await read();
		}
		const revision = await programmerRevision(api);
		const held = await valuesAction(
			api,
			nativeEdit([a1], red, -1, { reference, undoGroup: "direct-006", displayedSource: { lane: "normal", lease: shownLease } }),
		);
		expect(held).toMatchObject({ status: "no_change", hold: "displayed_source_unavailable" });
		expect(await programmerRevision(api)).toBe(revision);
		const fresh = await read();
		expect(fresh).toBeGreaterThan(shownLease);
		const applied = await valuesAction(
			api,
			nativeEdit([a1], red, -1, { reference, undoGroup: "direct-006", displayedSource: { lane: "normal", lease: fresh } }),
		);
		expect(applied.status, JSON.stringify(applied)).toBe("changed");
		expect((await colorOf(api, a1))?.recipe?.channels.find((channel) => channel.channel_id === red.channel_id)?.raw).toBe(254);
	});
});

async function hardwareConnected(api: ApiDriver) {
	return (await api.request<{ hardware_connected: boolean }>("GET", "/api/v2/bootstrap", undefined, false)).hardware_connected;
}

/** A stable description of the focused element, to prove that nothing moved focus. */
function focusedElement(page: Page) {
	return page.evaluate(() => {
		const element = document.activeElement;
		return element ? `${element.tagName}.${element.className}[${element.getAttribute("aria-label") ?? ""}]` : "none";
	});
}
