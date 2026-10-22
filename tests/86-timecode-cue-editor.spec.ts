import type { Locator, Page } from "@playwright/test";
import type { CueList } from "../apps/light-desktop/src/api/types";
import type { TimecodeDefinition } from "../apps/light-desktop/src/api/types/timecode";
import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";

// Exercise the production editor against a private bench server. Pointer movement, touch taps,
// authoritative Cuelist writes, and reopening are intentionally kept in the same operator path.
test.describe("TIMECODE-001 › overlapping Cue editor", () => {
	test.use({ hasTouch: true, viewport: { width: 1600, height: 1000 } });

	test("TIMECODE-001 @ui › touch selects one Cue, Close collapses it, and the sidebar preserves exact timeline geometry", async ({ api, show, desk, page }, testInfo) => {
		const rig = await seedEditor(api, show.id);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		const envelope = editor.getByRole("button", { name: "Cue 1 First", exact: true });
		const viewport = editor.getByLabel("Timecode timeline viewport", { exact: true });
		await expect(envelope).toBeVisible();
		await zoomIn(page, editor);
		const before = await geometry(editor, viewport, envelope);
		await envelope.tap();
		await expect(editor.getByRole("slider", { name: "Cue 1 In fade", exact: true })).toBeVisible();
		await expect(editor.locator(".timecode-editor-inspector")).toBeVisible();
		await page.screenshot({ path: testInfo.outputPath("timecode-cue-editor.png") });
		expect(await geometry(editor, viewport, envelope)).toEqual(before);
		await editor.getByRole("button", { name: "Edit Cue 2 Second", exact: true }).tap();
		await expect(editor.getByRole("slider", { name: "Cue 2 In fade", exact: true })).toBeVisible();
		await expect(editor.getByRole("slider", { name: "Cue 1 In fade", exact: true })).toHaveCount(0);
		await editor.getByRole("button", { name: "Close cue editor", exact: true }).tap();
		await expect(editor.getByRole("slider", { name: "Cue 2 In fade", exact: true })).toHaveCount(0);
		const closeSettings = editor.getByRole("button", { name: "Close selection settings", exact: true });
		if (await closeSettings.isVisible()) await closeSettings.tap();
		await expect(editor.locator(".timecode-editor-inspector")).toHaveCount(0);
		expect(await geometry(editor, viewport, envelope)).toEqual(before);

		// Closing a sidebar widens the viewport. At the right edge this must not let the browser
		// clamp scrollLeft and move the left-hand time origin, even though the content is wider.
		await editor.getByRole("button", { name: "Edit Cue 2 Second", exact: true }).tap();
		const box = await viewport.boundingBox();
		if (!box) throw new Error("Missing timeline viewport");
		await page.mouse.move(box.x + box.width / 2, box.y + 25);
		await page.mouse.wheel(100_000, 0);
		await expect.poll(() => viewport.evaluate((node) => node.scrollLeft)).toBeGreaterThan(0);
		const rightEdge = await geometry(editor, viewport, envelope);
		await editor.getByRole("button", { name: "Close selection settings", exact: true }).tap();
		expect(await geometry(editor, viewport, envelope)).toEqual(rightEdge);
		await editor.getByRole("button", { name: "Fit timeline", exact: true }).click();
		await expect.poll(() => viewport.evaluate((node) => node.scrollLeft)).toBe(0);
		await expect.poll(() => viewport.evaluate((node) => node.scrollWidth - node.clientWidth)).toBeLessThanOrEqual(1);
		await expect.poll(async () => (await api.showObject<CueList>(show.id, "cue_list", rig.list.id))?.body.cues.map((cue) => cue.trigger.type)).toEqual(["manual", "manual", "manual"]);
	});

	test("TIMECODE-001 @ui › each timing point saves to the Cuelist and a refused pointer edit rolls back", async ({ api, show, desk, page }) => {
		const rig = await seedEditor(api, show.id);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		await editor.getByRole("button", { name: "Cue 1 First", exact: true }).click();
		const readCue = async () => (await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body.cues[0];
		for (const [label, field] of [
			["In delay", "delay_millis"],
			["In fade", "fade_millis"],
			["Out delay", "out_delay_millis"],
			["Out fade", "out_fade_millis"],
		] as const) {
			const previous = (await readCue())[field];
			await dragFrames(page, editor, editor.getByRole("slider", { name: `Cue 1 ${label}`, exact: true }), 4);
			await expect.poll(async () => (await readCue())[field]).not.toBe(previous);
		}
		await dragFrames(page, editor, editor.getByRole("slider", { name: "Cue 1 Out delay", exact: true }), -600);
		await expect.poll(async () => (await readCue()).out_delay_millis).toBe(0);
		await expect(editor.getByRole("slider", { name: "Cue 1 Out delay", exact: true })).toHaveAttribute("aria-valuenow", "220");
		const saved = await readCue();
		await page.reload();
		await openEditor(page);
		await zoomIn(page, editor);
		await editor.getByRole("button", { name: "Cue 1 First", exact: true }).click();
		const frameOf = (millis: number | null | undefined) => Math.round((millis ?? 0) * 44 / 1000);
		for (const [label, frame] of [
			["In delay", 44 + frameOf(saved.delay_millis)],
			["In fade", 44 + frameOf(saved.delay_millis) + frameOf(saved.fade_millis)],
			["Out delay", 220 + frameOf(saved.out_delay_millis)],
			["Out fade", 220 + frameOf(saved.out_delay_millis) + frameOf(saved.out_fade_millis)],
		] as const)
			await expect(editor.getByRole("slider", { name: `Cue 1 ${label}`, exact: true })).toHaveAttribute("aria-valuenow", String(frame));
		expect(await readCue()).toEqual(saved);

		const handle = editor.getByRole("slider", { name: "Cue 1 In fade", exact: true });
		const before = await handle.getAttribute("aria-valuenow");
		let refused = 0;
		await page.route("**/api/v2/playback-topology/actions", async (route) => {
			if (route.request().postDataJSON()?.action?.type !== "save_cue_list") return route.continue();
			refused += 1;
			await route.fulfill({ status: 503, contentType: "application/json", body: JSON.stringify({ kind: "unavailable", error: "Test desk refused Cue timing", current_revision: null, current_related_revision: null, retryable: false }) });
		});
		await dragFrames(page, editor, handle, 4);
		await expect.poll(() => refused).toBeGreaterThan(0);
		await expect(page.getByText(/Test desk refused Cue timing/).first()).toBeVisible();
		await expect(handle).toHaveAttribute("aria-valuenow", before!);
		expect(await readCue()).toEqual(saved);
	});

	test("TIMECODE-001 @ui › manual Trigger drags stop at neighbours and survive reopening without changing trigger type", async ({ api, show, desk, page }) => {
		const rig = await seedEditor(api, show.id);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		await editor.getByRole("button", { name: "Cue 2 Second", exact: true }).click();
		const trigger = editor.getByRole("slider", { name: "Cue 2 Trigger", exact: true });
		await expect(trigger).toBeVisible();
		await dragFrames(page, editor, trigger, 300);
		const clip = async () => {
			const collection = await api.request<{ objects: Array<{ definition: TimecodeDefinition }> }>("GET", "/api/v2/timecodes", undefined, true, undefined, { showId: show.id });
			const definition = collection.objects.find((object) => object.definition.id === rig.definition.id)!.definition;
			const lane = definition.lanes[0];
			if (lane.content.kind !== "cue_list") throw new Error("Missing Cuelist lane");
			return lane.content.clips[0];
		};
		await expect.poll(async () => (await clip()).cue_starts.find((start) => start.cue_id === rig.list.cues[1].id)?.offset_frame).toBe(351);
		await dragFrames(page, editor, trigger, -600);
		await expect.poll(async () => (await clip()).cue_starts.find((start) => start.cue_id === rig.list.cues[1].id)?.offset_frame).toBe(1);
		await page.reload();
		await openEditor(page);
		await editor.getByRole("button", { name: "Cue 2 Second", exact: true }).click();
		await expect(editor.getByRole("slider", { name: "Cue 2 Trigger", exact: true })).toHaveAttribute("aria-valuenow", "45");
		expect((await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body.cues.map((cue) => cue.trigger.type)).toEqual(["manual", "manual", "manual"]);
	});
	test("TIMECODE-001 @ui › touch drags each timing point and cancelling a touch restores the unsaved preview", async ({ api, show, desk, page }) => {
		const rig = await seedEditor(api, show.id);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		await editor.getByRole("button", { name: "Cue 1 First", exact: true }).tap();
		const readCue = async () => (await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body.cues[0];
		const initial = await readCue();
		const handle = editor.getByRole("slider", { name: "Cue 1 In fade", exact: true });
		const initialFrame = await handle.getAttribute("aria-valuenow");
		await touchDragFrames(page, editor, handle, 4, true);
		await expect(handle).toHaveAttribute("aria-valuenow", initialFrame!);
		expect(await readCue()).toEqual(initial);
		for (const [label, field] of [
			["In delay", "delay_millis"], ["In fade", "fade_millis"],
			["Out delay", "out_delay_millis"], ["Out fade", "out_fade_millis"],
		] as const) {
			const before = (await readCue())[field];
			await touchDragFrames(page, editor, editor.getByRole("slider", { name: `Cue 1 ${label}`, exact: true }), 4);
			await expect.poll(async () => (await readCue())[field]).not.toBe(before);
		}
	});

	test("TIMECODE-001 @ui › first Trigger moves the whole clip and the final Cue explains its separate Clip Out", async ({ api, show, desk, page }) => {
		const rig = await seedEditor(api, show.id);
		const originalCues = (await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body.cues;
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		await editor.getByRole("button", { name: "Cue 1 First", exact: true }).click();
		await dragFrames(page, editor, editor.getByRole("slider", { name: "Cue 1 Trigger", exact: true }), 44);
		await expect.poll(async () => (await readClip(api, show.id, rig.definition.id)).start_frame).toBe(88);
		const moved = await readClip(api, show.id, rig.definition.id);
		expect(moved.end_frame).toBe(660);
		expect(moved.cue_starts).toEqual(rig.definition.lanes[0].content.kind === "cue_list" ? rig.definition.lanes[0].content.clips[0].cue_starts : []);
		await editor.getByRole("button", { name: "Edit Cue 3 Last", exact: true }).click();
		await expect(editor.locator(".timecode-cue-edit-rail.out.reference")).toContainText("Cue Out · next handover reference");
		const details = editor.locator(".timecode-cue-editor-details");
		await expect(details).toContainText("It is not played at this clip’s end.");
		await details.getByLabel("Clip Out duration", { exact: true }).fill("00:00:01.00");
		await details.getByLabel("Clip Out duration", { exact: true }).press("Tab");
		await expect.poll(async () => (await readClip(api, show.id, rig.definition.id)).out_fade_frames).toBe(44);
		expect((await readClip(api, show.id, rig.definition.id)).end_frame).toBe(660);
		expect((await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body.cues).toEqual(originalCues);
	});

});

async function seedEditor(api: ApiDriver, showId: string) {
	const list: CueList = {
		id: crypto.randomUUID(), name: "Envelope acceptance", mode: "sequence", priority: 0, looped: false,
		cues: ["First", "Second", "Last"].map((name, index) => ({
			id: crypto.randomUUID(), number: String(index + 1), name,
			fade_millis: 1000, delay_millis: 1000, out_delay_millis: 500, out_fade_millis: 1000,
			trigger: { type: "manual" as const }, changes: [],
		})),
	};
	await api.seedShowObject(showId, "cue_list", list.id, list);
	const definition: TimecodeDefinition = {
		id: crypto.randomUUID(), number: 86, name: "Cue editor acceptance", duration_frame: 880,
		transport_offset_frame: 0, auto_start: false, audio: null, markers: [],
		lanes: [{ id: crypto.randomUUID(), name: "All stage Cues", content: { kind: "cue_list", cue_list_id: list.id, clips: [{
			id: crypto.randomUUID(), start_frame: 44, end_frame: 616, start_cue_id: list.cues[0].id!, end_cue_id: list.cues[2].id!,
			start_behavior: "state", end_behavior: "release", in_fade_frames: 0, out_fade_frames: 0,
			cue_starts: [{ cue_id: list.cues[1].id!, offset_frame: 176 }, { cue_id: list.cues[2].id!, offset_frame: 352 }],
		}] } }],
	};
	await api.request("POST", "/api/v2/timecodes/actions", { request_id: crypto.randomUUID(), action: { type: "create", definition } }, true, undefined, { showId });
	return { list, definition };
}

async function openEditor(page: Page) {
	await expect(page.locator(".connection-cover")).toBeHidden();
	const pool = page.getByRole("button", { name: "Timecode 86 Cue editor acceptance", exact: true });
	if (!(await pool.isVisible().catch(() => false))) {
		const toggle = page.getByRole("button", { name: "Desktops / Built-ins", exact: true });
		if ((await toggle.getAttribute("data-dock-mode")) !== "builtins") await toggle.click();
		const shift = page.locator('[data-keypad-key="SHIFT"]:visible').first();
		if (!(await shift.isVisible().catch(() => false))) await page.locator(".mode-toggle").click();
		await shift.click();
		await page.locator("[aria-label='Shift Built-ins']").getByRole("button", { name: "Timecode", exact: true }).click();
	}
	await pool.click({ button: "right" });
	await expect(page.getByLabel("Timecode timeline editor", { exact: true })).toBeVisible();
}

async function zoomIn(page: Page, editor: Locator) {
	const overview = editor.getByRole("scrollbar", { name: "Timeline overview" });
	const handle = overview.getByRole("separator", { name: "Resize timeline overview from end" });
	const box = await handle.boundingBox();
	const track = await overview.boundingBox();
	if (!box || !track) throw new Error("Missing overview controls");
	await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	await page.mouse.down();
	// Approximately 2x keeps the first two Cues and every first-Cue point reachable.
	await page.mouse.move(track.x + track.width / 2, box.y + box.height / 2, { steps: 8 });
	await page.mouse.up();
}

async function dragFrames(page: Page, editor: Locator, handle: Locator, frames: number) {
	await handle.scrollIntoViewIfNeeded();
	const box = await handle.boundingBox();
	const scale = Number(await editor.locator(".timecode-timeline-canvas").getAttribute("data-pixels-per-frame"));
	if (!box || !(scale > 0)) throw new Error("Missing timing point or timeline scale");
	await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	await page.mouse.down();
	await page.mouse.move(box.x + box.width / 2 + frames * scale, box.y + box.height / 2, { steps: 10 });
	await page.mouse.up();
}

async function geometry(editor: Locator, viewport: Locator, envelope: Locator) {
	return {
		scale: await editor.locator(".timecode-timeline-canvas").getAttribute("data-pixels-per-frame"),
		scroll: await viewport.evaluate((node) => node.scrollLeft),
		cue: await envelope.evaluate((node) => ({ left: node.getBoundingClientRect().left, width: node.getBoundingClientRect().width })),
	};
}

async function readClip(api: ApiDriver, showId: string, timecodeId: string) {
	const collection = await api.request<{ objects: Array<{ definition: TimecodeDefinition }> }>("GET", "/api/v2/timecodes", undefined, true, undefined, { showId });
	const lane = collection.objects.find((object) => object.definition.id === timecodeId)!.definition.lanes[0];
	if (lane.content.kind !== "cue_list") throw new Error("Missing Cuelist lane");
	return lane.content.clips[0];
}

async function touchDragFrames(page: Page, editor: Locator, handle: Locator, frames: number, cancel = false) {
	await handle.scrollIntoViewIfNeeded();
	const box = await handle.boundingBox();
	const scale = Number(await editor.locator(".timecode-timeline-canvas").getAttribute("data-pixels-per-frame"));
	if (!box || !(scale > 0)) throw new Error("Missing touch timing point");
	const initial = await handle.getAttribute("aria-valuenow");
	const cdp = await page.context().newCDPSession(page);
	const touch = (x: number) => [{ x, y: box.y + box.height / 2, id: 1, radiusX: 2, radiusY: 2, force: 1 }];
	const x = box.x + box.width / 2;
	try {
		await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: touch(x) });
		for (let step = 1; step <= 8; step += 1)
			await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: touch(x + frames * scale * step / 8) });
		if (cancel) await expect(handle).not.toHaveAttribute("aria-valuenow", initial!);
		await cdp.send("Input.dispatchTouchEvent", { type: cancel ? "touchCancel" : "touchEnd", touchPoints: [] });
	} finally {
		await cdp.detach();
	}
}
