import type { APIRequestContext, Locator, Page } from "@playwright/test";
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
		const envelope = editor.getByRole("button", { name: "Edit Cue 1 House out", exact: true });
		const viewport = editor.getByLabel("Timecode timeline viewport", { exact: true });
		await expect(envelope).toBeVisible();
		await zoomIn(page, editor);
		await expect(editor.locator(".timecode-cue-envelope")).toHaveCount(0);
		for (const [index, name] of ["House out", "Verse wash", "Chorus"].entries())
			await assertCueFrameAlignment(editor, index + 1, name, [44, 220, 396][index], false);
		const before = await geometry(editor, viewport, envelope);
		await envelope.tap();
		await expect(editor.getByRole("slider", { name: "Cue 1 In fade", exact: true })).toBeVisible();
		await expect(editor.locator(".timecode-editor-inspector")).toBeVisible();
		await assertCueFrameAlignment(editor, 1, "House out", 44, true);
		const inDelay = editor.getByRole("slider", { name: "Cue 1 In delay", exact: true });
		await expect(inDelay).toHaveAttribute("aria-valuenow", "88");
		const triggerX = await editor.getByRole("slider", { name: "Cue 1 Trigger", exact: true }).evaluate((node) => { const box = node.getBoundingClientRect(); return box.left + box.width / 2; });
		const delayX = await inDelay.evaluate((node) => { const box = node.getBoundingClientRect(); return box.left + box.width / 2; });
		const oneSecondWidth = 44 * Number(await editor.locator(".timecode-timeline-canvas").getAttribute("data-pixels-per-frame"));
		expect(Math.abs(delayX - triggerX - oneSecondWidth)).toBeLessThanOrEqual(1);
		const scale = Number(await editor.locator(".timecode-timeline-canvas").getAttribute("data-pixels-per-frame"));
		const names = await editor.locator(".timecode-cue-name").evaluateAll((nodes) => nodes.map((node) => {
			const box = node.getBoundingClientRect();
			let opacity = 1;
			for (let ancestor: Element | null = node; ancestor; ancestor = ancestor.parentElement) opacity *= Number(getComputedStyle(ancestor).opacity);
			const style = getComputedStyle(node);
			return { left: box.left, width: box.width, opacity, text: node.textContent,
				boxShadow: style.boxShadow, borderRadius: style.borderRadius,
				textAlign: style.textAlign, background: style.backgroundColor };

		}));
		expect(names).toHaveLength(3);
		for (const [index, span] of [176, 176, 220].entries()) {
			expect(Math.abs(names[index].width - span * scale)).toBeLessThanOrEqual(1);
			expect(names[index].opacity).toBeGreaterThanOrEqual(0.85);
			expect(names[index].boxShadow).toMatch(/2px 0px(?: 0px){2} inset/);
			expect(names[index].borderRadius).toBe("0px");
			expect(names[index].textAlign).toBe("left");
			expect(names[index].background).not.toMatch(/^(?:transparent|rgba\(0,\s*0,\s*0,\s*0\))$/);
			if (index > 0) expect(Math.abs(names[index].left - names[index - 1].left - names[index - 1].width)).toBeLessThanOrEqual(1);
		}
		expect(names.map((name) => name.text)).toEqual([expect.stringContaining("House out"), expect.stringContaining("Verse wash"), expect.stringContaining("Chorus")]);
		await page.screenshot({ path: testInfo.outputPath("timecode-cue-editor.png") });
		await page.locator(".timecode-window.timecode-editor").screenshot({ path: testInfo.outputPath("timecode-cue-editor-detail.png") });
		expect(await geometry(editor, viewport, envelope)).toEqual(before);
		await editor.getByRole("button", { name: "Edit Cue 2 Verse wash", exact: true }).tap({ position: { x: 32, y: 18 } });
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
		await editor.getByRole("button", { name: "Edit Cue 2 Verse wash", exact: true }).tap({ position: { x: 32, y: 18 } });
		const box = await viewport.boundingBox();
		if (!box) throw new Error("Missing timeline viewport");
		await page.mouse.move(box.x + box.width / 2, box.y + 25);
		await page.mouse.wheel(100_000, 0);
		await expect.poll(() => viewport.evaluate((node) => node.scrollLeft)).toBeGreaterThan(0);
		const rightEdge = await geometry(editor, viewport, envelope);
		await editor.getByRole("button", { name: "Close selection settings", exact: true }).tap();
		expect(await geometry(editor, viewport, envelope)).toEqual(rightEdge);
		await page.getByRole("button", { name: "Fit timeline", exact: true }).click();
		await expect.poll(() => viewport.evaluate((node) => node.scrollLeft)).toBe(0);
		await expect.poll(() => viewport.evaluate((node) => node.scrollWidth - node.clientWidth)).toBeLessThanOrEqual(1);
		await expect.poll(async () => (await api.showObject<CueList>(show.id, "cue_list", rig.list.id))?.body.cues.map((cue) => cue.trigger.type)).toEqual(["manual", "manual", "manual"]);
	});

	test("TIMECODE-001 @ui › the complete expanded lane fits above the desk controls with an 88px envelope and 44px names", async ({ api, show, desk, page, request }, testInfo) => {
		await page.setViewportSize({ width: 1280, height: 800 });
		const { rig } = await seedAudioEditor(api, show.id, request, ["Movement", "Chase", "Colour"]);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		const viewport = editor.getByLabel("Timecode timeline viewport", { exact: true });
		const lane = editor.locator(`.timecode-editor-lane[data-lane-id="${rig.definition.lanes[0].id}"]`);
		const cue = lane.getByRole("button", { name: "Edit Cue 1 House out", exact: true });
		await cue.scrollIntoViewIfNeeded();
		const before = await geometry(editor, viewport, cue);
		await cue.tap({ position: { x: 32, y: 18 } });
		await expect(lane.getByRole("slider", { name: "Cue 1 In fade", exact: true })).toBeVisible();
		expect(await geometry(editor, viewport, cue)).toEqual(before);
		for (const cueNumber of [1, 3]) {
			if (cueNumber === 3) await lane.getByRole("button", { name: "Edit Cue 3 Chorus", exact: true }).click({ position: { x: 32, y: 18 } });
			await expect.poll(() => lane.evaluate((node) => {
				const box = node.getBoundingClientRect();
				const viewport = node.closest(".timecode-timeline-scroll")!;
				const bounds = viewport.getBoundingClientRect();
				const audio = viewport.querySelector(".main-audio")?.getBoundingClientRect();
				return box.top >= Math.max(bounds.top + viewport.clientTop + 32, audio?.bottom ?? 0) - 1 && box.bottom <= bounds.top + viewport.clientTop + viewport.clientHeight + 1;
			})).toBe(true);
			const envelopeHeight = await lane.locator(".timecode-cue-envelope").first().evaluate((node) => node.getBoundingClientRect().height);
			const nameHeight = await cue.evaluate((node) => node.getBoundingClientRect().height);
			expect(envelopeHeight).toBe(88);
			expect(nameHeight).toBe(44);
			expect(envelopeHeight / nameHeight).toBe(2);
			expect(await geometry(editor, viewport, cue)).toEqual(before);
		}
		await expect(page.locator('[data-keypad-key="SHIFT"]:visible').first()).toBeVisible();
		await page.locator(".timecode-window.timecode-editor").screenshot({ path: testInfo.outputPath("timecode-expanded-small-viewport-detail.png") });
	});

	test("TIMECODE-001 @ui › tapping an unplaced manual Cue divider never creates a placement or sends a save", async ({ api, show, desk, page }) => {
		const rig = await seedEditor(api, show.id);
		const collection = await api.request<{ objects: Array<{ revision: number; definition: TimecodeDefinition }> }>("GET", "/api/v2/timecodes", undefined, true, undefined, { showId: show.id });
		const lanes = structuredClone(rig.definition.lanes);
		if (lanes[0].content.kind !== "cue_list") throw new Error("Missing Cuelist");
		lanes[0].content.clips[0].cue_starts = lanes[0].content.clips[0].cue_starts.filter((start) => start.cue_id !== rig.list.cues[1].id);
		await api.request("POST", "/api/v2/timecodes/actions", { request_id: crypto.randomUUID(), action: { type: "update", timecode_id: rig.definition.id, expected_revision: collection.objects.find((entry) => entry.definition.id === rig.definition.id)!.revision, patch: { lanes } } }, true, undefined, { showId: show.id });
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await editor.getByRole("button", { name: "Edit Cue 1 House out", exact: true }).click({ position: { x: 32, y: 18 } });
		const before = await readClip(api, show.id, rig.definition.id);
		let saves = 0;
		page.on("request", (request) => { if (request.method() === "POST" && request.url().endsWith("/api/v2/timecodes/actions")) saves += 1; });
		const divider = editor.getByRole("slider", { name: "Move Cue 2 trigger", exact: true });
		await divider.click();
		await divider.tap();
		await page.waitForTimeout(150);
		expect(saves).toBe(0);
		expect(await readClip(api, show.id, rig.definition.id)).toEqual(before);
		expect((await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body.cues[1].trigger.type).toBe("manual");
	});

	test("TIMECODE-001 @ui › both visible Cue divider grips move the real Trigger with mouse and touch and persist", async ({ api, show, desk, page }) => {
		const rig = await seedEditor(api, show.id);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		await editor.getByRole("button", { name: "Edit Cue 2 Verse wash", exact: true }).click({ position: { x: 32, y: 18 } });
		const divider = editor.getByRole("slider", { name: "Move Cue 2 trigger", exact: true });
		await expect(divider).toHaveAttribute("aria-valuenow", "220");
		const grips = await divider.locator(":scope > span").evaluate((node) => ["::before", "::after"].map((pseudo) => {
			const style = getComputedStyle(node, pseudo);
			return { width: style.width, height: style.height, background: style.backgroundColor, content: style.content };
		}));
		for (const grip of grips) {
			expect(grip).toMatchObject({ width: "2px", height: "18px", content: '""' });
			expect(grip.background).not.toBe("rgba(0, 0, 0, 0)");
		}
		await dragFrames(page, editor, divider, 22, undefined, -6);
		await expect.poll(async () => (await readClip(api, show.id, rig.definition.id)).cue_starts.find((start) => start.cue_id === rig.list.cues[1].id)?.offset_frame).toBe(198);
		await touchDragFrames(page, editor, divider, 22, false, 6);
		await expect.poll(async () => (await readClip(api, show.id, rig.definition.id)).cue_starts.find((start) => start.cue_id === rig.list.cues[1].id)?.offset_frame).toBe(220);
		await assertCueFrameAlignment(editor, 2, "Verse wash", 264, true);
		await page.reload();
		await openEditor(page);
		await editor.getByRole("button", { name: "Edit Cue 2 Verse wash", exact: true }).click({ position: { x: 32, y: 18 } });
		await expect(divider).toHaveAttribute("aria-valuenow", "264");
		expect((await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body.cues.map((cue) => cue.trigger.type)).toEqual(["manual", "manual", "manual"]);
	});

	test("TIMECODE-001 @ui › each timing point saves to the Cuelist and a refused pointer edit rolls back", async ({ api, show, desk, page }) => {
		const rig = await seedEditor(api, show.id);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		await editor.getByRole("button", { name: "Edit Cue 1 House out", exact: true }).click();
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
		await editor.getByRole("button", { name: "Edit Cue 1 House out", exact: true }).click();
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
		await editor.getByRole("button", { name: "Edit Cue 2 Verse wash", exact: true }).click();
		const trigger = editor.getByRole("slider", { name: "Cue 2 Trigger", exact: true });
		await expect(trigger).toBeVisible();
		await assertCueFrameAlignment(editor, 2, "Verse wash", 220, true);
		await touchDragFrames(page, editor, trigger, 44, true);
		await assertCueFrameAlignment(editor, 2, "Verse wash", 220, true);
		expect((await readClip(api, show.id, rig.definition.id)).cue_starts.find((start) => start.cue_id === rig.list.cues[1].id)?.offset_frame).toBe(176);
		await dragFrames(page, editor, trigger, 44, async () => {
			await assertCueFrameAlignment(editor, 2, "Verse wash", 264, true);
			expect((await readClip(api, show.id, rig.definition.id)).cue_starts.find((start) => start.cue_id === rig.list.cues[1].id)?.offset_frame).toBe(176);
		});
		await expect.poll(async () => (await readClip(api, show.id, rig.definition.id)).cue_starts.find((start) => start.cue_id === rig.list.cues[1].id)?.offset_frame).toBe(220);
		await assertCueFrameAlignment(editor, 2, "Verse wash", 264, true);
		await page.reload();
		await openEditor(page);
		await zoomIn(page, editor);
		await editor.getByRole("button", { name: "Edit Cue 2 Verse wash", exact: true }).tap({ position: { x: 32, y: 18 } });
		await assertCueFrameAlignment(editor, 2, "Verse wash", 264, true);
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
		await editor.getByRole("button", { name: "Edit Cue 2 Verse wash", exact: true }).click();
		await expect(editor.getByRole("slider", { name: "Cue 2 Trigger", exact: true })).toHaveAttribute("aria-valuenow", "45");
		expect((await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body.cues.map((cue) => cue.trigger.type)).toEqual(["manual", "manual", "manual"]);
	});
	test("TIMECODE-001 @ui › touch drags each timing point and cancelling a touch restores the unsaved preview", async ({ api, show, desk, page }) => {
		const rig = await seedEditor(api, show.id);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		await editor.getByRole("button", { name: "Edit Cue 1 House out", exact: true }).tap();
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
		await editor.getByRole("button", { name: "Edit Cue 1 House out", exact: true }).click();
		const firstTrigger = editor.getByRole("slider", { name: "Cue 1 Trigger", exact: true });
		await touchDragFrames(page, editor, firstTrigger, 44, true);
		await assertCueFrameAlignment(editor, 1, "House out", 44, true);
		expect((await readClip(api, show.id, rig.definition.id)).start_frame).toBe(44);
		await dragFrames(page, editor, firstTrigger, 44, async () => {
			await assertCueFrameAlignment(editor, 1, "House out", 88, true);
			await assertCueFrameAlignment(editor, 2, "Verse wash", 264, false);
			const saved = await readClip(api, show.id, rig.definition.id);
			expect(saved.start_frame).toBe(44);
			expect(saved.end_frame).toBe(616);
		});
		await expect.poll(async () => (await readClip(api, show.id, rig.definition.id)).start_frame).toBe(88);
		const moved = await readClip(api, show.id, rig.definition.id);
		expect(moved.end_frame).toBe(660);
		expect(moved.cue_starts).toEqual(rig.definition.lanes[0].content.kind === "cue_list" ? rig.definition.lanes[0].content.clips[0].cue_starts : []);
		await editor.getByRole("button", { name: "Edit Cue 3 Chorus", exact: true }).click();
		await expect(editor.locator(".timecode-cue-edit-rail.out.reference")).toContainText("Out · not executed");
		const details = editor.locator(".timecode-cue-editor-details");
		await expect(details).toContainText("It is not played at this clip’s end.");
		await details.getByLabel("Clip Out duration", { exact: true }).fill("00:00:01.00");
		await details.getByLabel("Clip Out duration", { exact: true }).press("Tab");
		await expect.poll(async () => (await readClip(api, show.id, rig.definition.id)).out_fade_frames).toBe(44);
		expect((await readClip(api, show.id, rig.definition.id)).end_frame).toBe(660);
		expect((await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body.cues).toEqual(originalCues);
	});

	test("TIMECODE-001 @ui › open Speed Groups retain context and Cue focus dims only other editing lanes", async ({ api, show, desk, page, request }, testInfo) => {
		const { rig, speedLanes } = await seedAudioEditor(api, show.id, request, ["Movement", "Chase", "Colour"]);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		const window = page.locator(".timecode-window.timecode-editor");
		await expect(editor.getByLabel("Selected lane and keyframe actions")).toHaveCount(0);
		const section = editor.locator(".timecode-speed-section");
		const movement = editor.locator(`[data-lane-id="${speedLanes[0].id}"].timecode-editor-lane`);
		const chase = editor.locator(`[data-lane-id="${speedLanes[1].id}"].timecode-editor-lane`);
		const color = (lane: Locator) => lane.evaluate((node) => getComputedStyle(node).backgroundColor);
		await movement.getByRole("button", { name: /^Movement/ }).tap();
		await expect(movement).toHaveClass(/lane-expanded/);
		expect(await color(movement)).not.toBe(await color(chase));
		await window.getByRole("button", { name: "Next Keyframe", exact: true }).tap();
		await expect(movement.locator(".timecode-timeline-item.selected")).toHaveCount(1);
		await editor.getByRole("button", { name: "Close speed group editor", exact: true }).tap();
		await expect(section).toHaveClass(/section-expanded/);
		await expect(movement).not.toHaveClass(/lane-expanded/);
		expect(await color(movement)).toBe(await color(chase));
		expect(await color(movement)).not.toBe("rgba(0, 0, 0, 0)");
		await window.screenshot({ path: testInfo.outputPath("timecode-speed-context-detail.png") });
		await editor.getByRole("button", { name: "Edit Cue 1 House out", exact: true }).click({ position: { x: 32, y: 18 } });
		const active = editor.locator(`[data-lane-id="${rig.definition.lanes[0].id}"].timecode-editor-lane`);
		const other = editor.locator(".lane-cue_list").nth(1);
		const opacity = (lane: Locator) => lane.evaluate((node) => Number(getComputedStyle(node).opacity));
		expect(await opacity(section)).toBeLessThan(0.5);
		expect(await opacity(other)).toBeLessThan(0.5);
		expect(await opacity(active)).toBe(1);
		expect(await opacity(editor.locator(".main-audio"))).toBe(1);
		const inspector = editor.locator(".timecode-editor-inspector");
		const sectionHeaders = await inspector.locator("summary").evaluateAll((nodes) => nodes.map((node) => {
			const box = node.getBoundingClientRect();
			return { name: node.textContent, left: box.left, width: box.width, height: box.height };
		}));
		expect(sectionHeaders.map((header) => header.name)).toEqual(["Cue In", "Cue Out", "Cuelist"]);
		for (const header of sectionHeaders) {
			expect(Math.abs(header.left - sectionHeaders[0].left)).toBeLessThanOrEqual(1);
			expect(Math.abs(header.width - sectionHeaders[0].width)).toBeLessThanOrEqual(1);
			expect(header.height).toBeGreaterThanOrEqual(44);
		}
		for (const name of ["Cue In", "Cue Out", "Cuelist"]) {
			const detail = inspector.locator("details").filter({ has: page.locator("summary").filter({ hasText: new RegExp(`^${name}$`) }) });
			const summary = inspector.locator("summary").filter({ hasText: new RegExp(`^${name}$`) });
			const wasOpen = await detail.getAttribute("open");
			await summary.tap();
			if (wasOpen === null) await expect(detail).toHaveAttribute("open", "");
			else await expect(detail).not.toHaveAttribute("open", "");
			await summary.click();
		}
		await inspector.locator("summary").filter({ hasText: /^Cue In$/ }).tap();
		await inspector.locator("summary").filter({ hasText: /^Cue Out$/ }).tap();
		const panel = (await inspector.boundingBox())!;
		await page.mouse.move(panel.x + panel.width / 2, panel.y + panel.height / 2);
		await page.mouse.wheel(0, -10000);
		await window.screenshot({ path: testInfo.outputPath("timecode-focus-sidebar-detail.png") });
	});

	test("TIMECODE-001 @ui › the last Cue end grip preserves earlier triggers and retains a refused autosave draft", async ({ api, show, desk, page }, testInfo) => {
		const rig = await seedEditor(api, show.id);
		const originalCues = structuredClone((await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body.cues);
		const originalClip = await readClip(api, show.id, rig.definition.id);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await editor.getByRole("button", { name: "Edit Cue 3 Chorus", exact: true }).click({ position: { x: 32, y: 18 } });
		const end = editor.getByRole("slider", { name: "Extend last Cue", exact: true });
		const divider = editor.getByRole("slider", { name: "Move Cue 3 trigger", exact: true });
		const centerY = (handle: Locator) => handle.evaluate((node) => { const box = node.getBoundingClientRect(); return box.top + box.height / 2; });
		expect(Math.abs(await centerY(end) - await centerY(divider))).toBeLessThanOrEqual(1);
		const nameBand = editor.getByRole("button", { name: "Edit Cue 3 Chorus", exact: true });
		const nameBox = (await nameBand.boundingBox())!;
		expect(await centerY(end)).toBeGreaterThanOrEqual(nameBox.y);
		expect(await centerY(end)).toBeLessThanOrEqual(nameBox.y + nameBox.height);
		await dragFrames(page, editor, end, 44);
		await expect.poll(async () => (await readClip(api, show.id, rig.definition.id)).end_frame).toBe(660);
		await touchDragFrames(page, editor, end, 44);
		await expect.poll(async () => (await readClip(api, show.id, rig.definition.id)).end_frame).toBe(704);
		expect((await readClip(api, show.id, rig.definition.id)).cue_starts).toEqual(originalClip.cue_starts);
		expect((await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body.cues).toEqual(originalCues);
		await page.route("**/api/v2/timecodes/actions", (route) => route.request().method() === "POST" ? route.fulfill({ status: 503, contentType: "application/json", body: JSON.stringify({ error: { code: "save_refused", message: "Refused end extension" } }) }) : route.continue());
		await dragFrames(page, editor, end, 4);
		await expect(page.getByText(/Refused end extension|503/).first()).toBeVisible();
		await expect(page.getByText("Not saved", { exact: true })).toBeVisible();
		await expect(page.getByRole("button", { name: "Retry autosave", exact: true })).toBeVisible();
		await expect(end).toHaveAttribute("aria-valuenow", "708");
		expect((await readClip(api, show.id, rig.definition.id)).end_frame).toBe(704);
		await page.unroute("**/api/v2/timecodes/actions");
		await page.reload();
		await openEditor(page);
		await editor.getByRole("button", { name: "Edit Cue 3 Chorus", exact: true }).click({ position: { x: 32, y: 18 } });
		await expect(end).toHaveAttribute("aria-valuenow", "704");
		const collection = await api.request<{ objects: Array<{ revision: number; definition: TimecodeDefinition }> }>("GET", "/api/v2/timecodes", undefined, true, undefined, { showId: show.id });
		await api.request("POST", "/api/v2/timecodes/actions", { request_id: crypto.randomUUID(), action: { type: "update", timecode_id: rig.definition.id, expected_revision: collection.objects.find((entry) => entry.definition.id === rig.definition.id)!.revision, patch: { duration_frame: 704 } } }, true, undefined, { showId: show.id });
		await page.reload();
		await openEditor(page);
		await editor.getByRole("button", { name: "Edit Cue 3 Chorus", exact: true }).click({ position: { x: 32, y: 18 } });
		const viewport = editor.getByLabel("Timecode timeline viewport", { exact: true });
		const scrollBox = (await viewport.boundingBox())!;
		await page.mouse.move(scrollBox.x + scrollBox.width / 2, scrollBox.y + scrollBox.height / 2);
		await page.mouse.wheel(100000, 0);
		await expect.poll(() => end.evaluate((node) => {
			const target = node.getBoundingClientRect();
			const scroll = node.closest(".timecode-timeline-scroll")!;
			const box = scroll.getBoundingClientRect();
			return target.width === 44 && target.left >= box.left && target.right <= box.left + scroll.clientLeft + scroll.clientWidth + 1;
		})).toBe(true);
		await touchDragFrames(page, editor, end, -4, true);
		expect((await readClip(api, show.id, rig.definition.id)).end_frame).toBe(704);
		await page.locator(".timecode-window.timecode-editor").screenshot({ path: testInfo.outputPath("timecode-end-grabber-detail.png") });
		await page.getByRole("button", { name: "Delete clip", exact: true }).click();
		await expect(editor.locator(".timecode-cue-block")).toHaveCount(0);
	});

	test("TIMECODE-001 @ui › a long final Cue Out reference remains reachable after its independent horizontal scroll", async ({ api, show, desk, page }) => {
		const rig = await seedEditor(api, show.id);
		const record = (await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!;
		rig.list.cues[2].out_fade_millis = 30_000;
		await api.seedShowObject(show.id, "cue_list", rig.list.id, rig.list, record.revision);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		await editor.getByRole("button", { name: "Edit Cue 3 Chorus", exact: true }).click({ position: { x: 32, y: 18 } });
		const reference = editor.locator(".timecode-cue-edit-rail.out.reference");
		const readCue = async () => (await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body.cues[2];
		for (const [label, field] of [["Out fade", "out_fade_millis"], ["Out delay", "out_delay_millis"]] as const) {
			const handle = editor.getByRole("slider", { name: `Cue 3 ${label}`, exact: true });
			await handle.scrollIntoViewIfNeeded();
			await expect.poll(() => handle.evaluate((node) => {
				const box = node.getBoundingClientRect();
				return node.contains(document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2));
			})).toBe(true);
			if (label === "Out fade") await expect.poll(() => reference.evaluate((node) => node.scrollLeft)).toBeGreaterThan(0);
			const before = (await readCue())[field];
			await touchDragFrames(page, editor, handle, 4);
			await expect.poll(async () => (await readCue())[field]).toBeGreaterThan(before ?? 0);
		}
		expect((await readClip(api, show.id, rig.definition.id)).end_frame).toBe(616);
		expect((await readCue()).trigger.type).toBe("manual");
	});

	test("TIMECODE-001 @ui › the main audio waveform and volume points stay below the ruler while other lanes scroll", async ({ api, show, desk, page, request }, testInfo) => {
		const { rig, speedLanes, audioLaneId, lanes } = await seedAudioEditor(api, show.id, request, ["Movement", "Chase", "Colour", "Stage movement", "Audience movement", "Stage chase", "Audience chase", "Colour pulse", "Colour sweep", "Accent movement", "Accent chase", "Accent colour"]);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await expandSpeedGroups(editor);
		const viewport = editor.getByLabel("Timecode timeline viewport", { exact: true });
		const ruler = editor.locator(".timecode-timeline-heading");
		const audio = editor.locator(".timecode-editor-lane.lane-audio_volume.main-audio");
		const waveform = audio.locator(".timecode-waveform");
		const label = audio.locator(".timecode-editor-lane-label");
		const volume = audio.locator(".timecode-timeline-item.item-volume");
		const ordinary = editor.locator(`.timecode-editor-lane[data-lane-id="${speedLanes[0].id}"]`);
		const speed = ordinary.locator(".timecode-timeline-item.item-speed");
		await expect(waveform.locator("path")).toHaveAttribute("d", /.+/);
		await expect(editor.locator(".timecode-editor-lane").first()).toHaveAttribute("data-lane-id", audioLaneId);
		await expect(label).toContainText("Show Opener.wav");
		await ordinary.locator(".timecode-lane-select").click();
		await zoomIn(page, editor);
		const point = (locator: Locator) => locator.evaluate((node) => {
			const box = node.getBoundingClientRect(); return { x: box.x, y: box.y, width: box.width, height: box.height };
		});
		const initial = { waveform: await point(waveform), label: await point(label), volume: await point(volume), ruler: await point(ruler), ordinary: await point(ordinary), speed: await point(speed) };
		expect(initial.waveform.y).toBeGreaterThanOrEqual(initial.ruler.y + initial.ruler.height);
		expect(initial.volume.x + initial.volume.width / 2).toBeCloseTo(initial.speed.x + initial.speed.width / 2, 1);
		const bounds = await viewport.boundingBox();
		if (!bounds) throw new Error("Missing timeline viewport");
		await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height - 30);
		await page.mouse.wheel(0, 360);
		await expect.poll(() => viewport.evaluate((node) => node.scrollTop)).toBeGreaterThan(100);
		for (const [locator, previous] of [[waveform, initial.waveform], [label, initial.label], [volume, initial.volume], [ruler, initial.ruler]] as const)
			expect((await point(locator)).y).toBe(previous.y);
		expect((await point(ordinary)).y).toBeLessThan(initial.ordinary.y - 100);
		const beforePan = { waveform: await point(waveform), volume: await point(volume), speed: await point(speed), scroll: await viewport.evaluate((node) => node.scrollLeft) };
		await page.mouse.wheel(320, 0);
		await expect.poll(() => viewport.evaluate((node) => node.scrollLeft)).toBeGreaterThan(beforePan.scroll);
		const delta = await viewport.evaluate((node) => node.scrollLeft) - beforePan.scroll;
		for (const [locator, previous] of [[waveform, beforePan.waveform], [volume, beforePan.volume], [speed, beforePan.speed]] as const) {
			const moved = await point(locator);
			expect(moved.x).toBeCloseTo(previous.x - delta, 1);
			expect(moved.width).toBe(previous.width);
		}
		const collection = await api.request<{ objects: Array<{ definition: TimecodeDefinition }> }>("GET", "/api/v2/timecodes", undefined, true, undefined, { showId: show.id });
		expect(collection.objects.find((entry) => entry.definition.id === rig.definition.id)!.definition.lanes.map((lane) => lane.id)).toEqual(lanes.map((lane) => lane.id));
		await page.screenshot({ path: testInfo.outputPath("timecode-sticky-main-audio.png") });
	});

	test("TIMECODE-001 @ui › grouped Speed navigation previews every group and enlarges only the selected lane", async ({ api, show, desk, page, request }, testInfo) => {
		const names = ["Movement", "Chase", "Colour"];
		const { rig, speedLanes, audioLaneId, lanes } = await seedAudioEditor(api, show.id, request, names);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		const viewport = editor.getByLabel("Timecode timeline viewport", { exact: true });
		const audio = editor.locator(".timecode-editor-lane.main-audio");
		const section = editor.locator(".timecode-speed-section");
		const toggle = section.getByRole("button", { name: "Speed Groups", exact: true });
		const summary = section.locator(".timecode-speed-summary");
		const cueLane = editor.locator(`.timecode-editor-lane[data-lane-id="${rig.definition.lanes[0].id}"]`);
		const speedLane = (index: number) => editor.locator(`.timecode-editor-lane[data-lane-id="${speedLanes[index].id}"]`);
		await expect(editor.locator(".timecode-editor-lane").first()).toHaveAttribute("data-lane-id", audioLaneId);
		await expect(toggle).toHaveAttribute("aria-expanded", "true");
		await expect(toggle).toContainText("Collapse");
		await toggle.tap();
		await expect(toggle).toHaveAttribute("aria-expanded", "false");
		await expect(toggle).toContainText("Expand");
		await expect(section.getByRole("button", { name: "Expand Speed Groups", exact: true })).toBeVisible();
		await expect(summary).toHaveAttribute("aria-description", names.join(", "));
		await expect(summary).toBeVisible();
		for (const [index, name] of names.entries()) {
			const preview = summary.locator(`[data-lane-id="${speedLanes[index].id}"]`);
			await expect(preview).toBeVisible();
			await expect(preview).toHaveAttribute("data-lane-name", name);
		}
		const bottom = (locator: Locator) => locator.evaluate((node) => node.getBoundingClientRect().bottom);
		const top = (locator: Locator) => locator.evaluate((node) => node.getBoundingClientRect().top);
		expect(await top(section)).toBeGreaterThanOrEqual(await bottom(audio));
		expect(await top(cueLane)).toBeGreaterThanOrEqual(await bottom(section));
		await expect(editor.getByRole("button", { name: "Cuelists", exact: true })).toHaveCount(0);
		await expect(editor.getByRole("button", { name: "Expand Cuelists", exact: true })).toHaveCount(0);
		await expect(editor.locator(".lane-cue_list")).toHaveCount(2);
		await expect(editor.locator(".lane-cue_list").nth(0).locator(".timecode-lane-select")).toContainText("Stage Looks");
		await expect(editor.locator(".lane-cue_list").nth(1).locator(".timecode-lane-select")).toContainText("Audience Looks");
		const speedRowHeight = await section.locator(".timecode-section-row.collapsed").evaluate((node) => node.getBoundingClientRect().height);
		expect(speedRowHeight).toBe(44);
		for (const row of await editor.locator(".lane-cue_list").all()) {
			expect(await row.evaluate((node) => node.getBoundingClientRect().height)).toBe(56);
			for (const cue of await row.locator(".timecode-cue-name").all()) expect(await cue.evaluate((node) => node.getBoundingClientRect().height)).toBe(44);
		}
		await expect(editor.locator(".timecode-cue-envelope")).toHaveCount(0);
		await page.screenshot({ path: testInfo.outputPath("timecode-grouped-overview.png") });
		await page.locator(".timecode-window.timecode-editor").screenshot({ path: testInfo.outputPath("timecode-grouped-overview-detail.png") });
		await toggle.tap();
		await expect(toggle).toHaveAttribute("aria-expanded", "true");
		await expect(summary).toBeHidden();
		const compactHeight = await speedLane(1).evaluate((node) => node.getBoundingClientRect().height);
		await toggle.tap();
		await expect(toggle).toHaveAttribute("aria-expanded", "false");
		// The combined miniature is one full-row touch target; choose a lane after expanding it.
		await section.getByRole("button", { name: "Expand Speed Groups", exact: true }).tap();
		await expect(toggle).toHaveAttribute("aria-expanded", "true");
		await expect(toggle).toContainText("Collapse");
		await speedLane(0).locator(".timecode-lane-select").tap();
		await expect(speedLane(0)).toHaveClass(/lane-expanded/);
		await expect(editor.locator(".lane-speed_group.lane-expanded")).toHaveCount(1);
		expect(await speedLane(0).evaluate((node) => node.getBoundingClientRect().height)).toBeGreaterThan(compactHeight);
		await speedLane(1).locator(".timecode-lane-select").tap();
		await expect(speedLane(1)).toHaveClass(/lane-expanded/);
		await expect(speedLane(0)).not.toHaveClass(/lane-expanded/);
		await expect(editor.locator(".lane-speed_group.lane-expanded")).toHaveCount(1);
		await zoomIn(page, editor);
		const point = speedLane(1).locator(".timecode-timeline-item.item-speed");
		const before = await geometry(editor, viewport, point);
		await point.click();
		await expect(editor.locator(".timecode-editor-inspector")).toBeVisible();
		expect(await geometry(editor, viewport, point)).toEqual(before);
		await page.screenshot({ path: testInfo.outputPath("timecode-speed-group-editor.png") });
		await page.locator(".timecode-window.timecode-editor").screenshot({ path: testInfo.outputPath("timecode-speed-group-editor-detail.png") });
		await editor.getByRole("button", { name: "Close selection settings", exact: true }).click();
		expect(await geometry(editor, viewport, point)).toEqual(before);
		await speedLane(1).getByRole("button", { name: "Close speed group editor", exact: true }).tap();
		await expect(editor.locator(".lane-speed_group.lane-expanded")).toHaveCount(0);
		await toggle.tap();
		await expect(toggle).toHaveAttribute("aria-expanded", "false");
		await expect(summary).toBeVisible();
		for (const [index, name] of names.entries()) await expect(summary.locator(`[data-lane-id="${speedLanes[index].id}"]`)).toHaveAttribute("data-lane-name", name);
		const collection = await api.request<{ objects: Array<{ definition: TimecodeDefinition }> }>("GET", "/api/v2/timecodes", undefined, true, undefined, { showId: show.id });
		expect(collection.objects.find((entry) => entry.definition.id === rig.definition.id)!.definition.lanes.map((lane) => lane.id)).toEqual(lanes.map((lane) => lane.id));
	});

	test("TIMECODE-001 @ui › coincident In end and Out start remain independently editable by mouse and touch", async ({ api, show, desk, page }) => {
		// Follow 0 with zero Out delay puts the handover exactly at the incoming fade's end.
		const rig = await seedEditor(api, show.id, true);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		await editor.getByRole("button", { name: "Edit Cue 1 House out", exact: true }).tap();
		const incoming = editor.getByRole("slider", { name: "Cue 1 In fade", exact: true });
		const outgoing = editor.getByRole("slider", { name: "Cue 1 Out delay", exact: true });
		const readCue = async () => (await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body.cues[0];
		await expect(incoming).toHaveAttribute("aria-valuenow", "132");
		await expect(outgoing).toHaveAttribute("aria-valuenow", "132");
		await dragFrames(page, editor, incoming, 4);
		await expect.poll(async () => (await readCue()).fade_millis).toBeGreaterThan(1000);
		expect((await readCue()).out_delay_millis).toBe(0);
		const inFadeAfterMouse = (await readCue()).fade_millis;
		await dragFrames(page, editor, outgoing, 4);
		await expect.poll(async () => (await readCue()).out_delay_millis).toBeGreaterThan(0);
		expect((await readCue()).fade_millis).toBe(inFadeAfterMouse);

		const inEndFrame = Number(await incoming.getAttribute("aria-valuenow"));
		const exactOutStart = editor.locator(".timecode-cue-editor-details").getByLabel("Out start", { exact: true });
		await exactOutStart.fill(frameText(inEndFrame));
		await exactOutStart.press("Tab");
		await expect.poll(async () => (await readCue()).out_delay_millis).toBe(0);
		await expect(outgoing).toHaveAttribute("aria-valuenow", String(inEndFrame));
		await touchDragFrames(page, editor, incoming, -4);
		await expect.poll(async () => (await readCue()).fade_millis).toBe(1000);
		expect((await readCue()).out_delay_millis).toBe(0);
		await expect(incoming).toHaveAttribute("aria-valuenow", "132");
		await expect(outgoing).toHaveAttribute("aria-valuenow", "132");
		await touchDragFrames(page, editor, outgoing, 4);
		await expect.poll(async () => (await readCue()).out_delay_millis).toBeGreaterThan(0);
		expect((await readCue()).fade_millis).toBe(1000);
	});

	test("TIMECODE-001 @ui › a four-frame clip keeps its true width and Trigger alignment when moved", async ({ api, show, desk, page }) => {
		// A real nonzero 1ms In fade rounds to zero frames, avoiding zero-In inheritance.
		const rig = await seedEditor(api, show.id, false, true);
		const lanes = structuredClone(rig.definition.lanes);
		if (lanes[0].content.kind !== "cue_list") throw new Error("Expected Cuelist lane");
		const clip = lanes[0].content.clips[0];
		clip.end_frame = 48;
		clip.end_cue_id = clip.start_cue_id;
		clip.cue_starts = [];
		const current = await api.request<{ objects: Array<{ revision: number; definition: TimecodeDefinition }> }>("GET", "/api/v2/timecodes", undefined, true, undefined, { showId: show.id });
		await api.request("POST", "/api/v2/timecodes/actions", { request_id: crypto.randomUUID(), action: { type: "update", timecode_id: rig.definition.id, expected_revision: current.objects.find((entry) => entry.definition.id === rig.definition.id)!.revision, patch: { lanes } } }, true, undefined, { showId: show.id });
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		const scale = Number(await editor.locator(".timecode-timeline-canvas").getAttribute("data-pixels-per-frame"));
		const outer = editor.locator(".timecode-timeline-item.item-clip");
		const cue = editor.getByRole("button", { name: "Edit Cue 1 House out", exact: true });
		for (const target of [outer, cue]) {
			const width = await target.evaluate((node) => node.getBoundingClientRect().width);
			expect(width).toBeLessThan(44);
			expect(Math.abs(width - 4 * scale)).toBeLessThanOrEqual(1);
		}
		await assertCueFrameAlignment(editor, 1, "House out", 44, false);
		await cue.click({ position: { x: 2, y: 22 } });
		await assertCueFrameAlignment(editor, 1, "House out", 44, true);
		await expect(editor.getByRole("slider", { name: "Cue 1 In fade", exact: true })).toHaveAttribute("aria-valuenow", "44");
		const trigger = editor.getByRole("slider", { name: "Cue 1 Trigger", exact: true });
		await touchDragFrames(page, editor, trigger, 4, true);
		await assertCueFrameAlignment(editor, 1, "House out", 44, true);
		expect((await readClip(api, show.id, rig.definition.id)).start_frame).toBe(44);
		await dragFrames(page, editor, trigger, 4);
		await expect.poll(async () => (await readClip(api, show.id, rig.definition.id)).start_frame).toBe(48);
		expect((await readClip(api, show.id, rig.definition.id)).end_frame).toBe(52);
		await assertCueFrameAlignment(editor, 1, "House out", 48, true);
		expect((await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body.cues[0]).toMatchObject({ fade_millis: 1, delay_millis: 0, out_delay_millis: 0, out_fade_millis: 0 });
	});

});

async function seedEditor(api: ApiDriver, showId: string, coincident = false, nearZero = false) {
	const list: CueList = {
		id: crypto.randomUUID(), name: "Stage Looks", mode: "sequence", priority: 0, looped: false,
		cues: ["House out", "Verse wash", "Chorus"].map((name, index) => ({
			id: crypto.randomUUID(), number: String(index + 1), name,
			fade_millis: nearZero && index === 0 ? 1 : 1000, delay_millis: nearZero && index === 0 ? 0 : 1000, out_delay_millis: (coincident || nearZero) && index === 0 ? 0 : 500, out_fade_millis: nearZero && index === 0 ? 0 : 1000,
			trigger: coincident && index === 1 ? { type: "follow", delay_millis: 0 } : { type: "manual" as const }, changes: [],
		})),
	};
	await api.seedShowObject(showId, "cue_list", list.id, list);
	const definition: TimecodeDefinition = {
		id: crypto.randomUUID(), number: 86, name: "Cue editor acceptance", duration_frame: 880,
		transport_offset_frame: 0, auto_start: false, audio: null, markers: [],
		lanes: [{ id: crypto.randomUUID(), name: "Stage Looks", content: { kind: "cue_list", cue_list_id: list.id, clips: [{
			id: crypto.randomUUID(), start_frame: 44, end_frame: 616, start_cue_id: list.cues[0].id!, end_cue_id: list.cues[2].id!,
			start_behavior: "state", end_behavior: "release", in_fade_frames: 0, out_fade_frames: 0,
			cue_starts: [...(coincident ? [] : [{ cue_id: list.cues[1].id!, offset_frame: 176 }]), { cue_id: list.cues[2].id!, offset_frame: 352 }],
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

async function dragFrames(page: Page, editor: Locator, handle: Locator, frames: number, onPreview?: () => Promise<void>, grabOffset = 0) {
	await handle.scrollIntoViewIfNeeded();
	const box = await handle.boundingBox();
	const scale = Number(await editor.locator(".timecode-timeline-canvas").getAttribute("data-pixels-per-frame"));
	if (!box || !(scale > 0)) throw new Error("Missing timing point or timeline scale");
	await page.mouse.move(box.x + box.width / 2 + grabOffset, box.y + box.height / 2);
	await page.mouse.down();
	await page.mouse.move(box.x + box.width / 2 + grabOffset + frames * scale, box.y + box.height / 2, { steps: 10 });
	await onPreview?.();
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

async function touchDragFrames(page: Page, editor: Locator, handle: Locator, frames: number, cancel = false, grabOffset = 0) {
	await handle.scrollIntoViewIfNeeded();
	const box = await handle.boundingBox();
	const scale = Number(await editor.locator(".timecode-timeline-canvas").getAttribute("data-pixels-per-frame"));
	if (!box || !(scale > 0)) throw new Error("Missing touch timing point");
	const initial = await handle.getAttribute("aria-valuenow");
	const cdp = await page.context().newCDPSession(page);
	const touch = (x: number) => [{ x, y: box.y + box.height / 2, id: 1, radiusX: 2, radiusY: 2, force: 1 }];
	const x = box.x + box.width / 2 + grabOffset;
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

function referenceWav() {
	const sampleRate = 8000;
	const frames = sampleRate * 20;
	const buffer = Buffer.alloc(44 + frames * 2);
	buffer.write("RIFF", 0); buffer.writeUInt32LE(buffer.length - 8, 4); buffer.write("WAVEfmt ", 8);
	buffer.writeUInt32LE(16, 16); buffer.writeUInt16LE(1, 20); buffer.writeUInt16LE(1, 22);
	buffer.writeUInt32LE(sampleRate, 24); buffer.writeUInt32LE(sampleRate * 2, 28); buffer.writeUInt16LE(2, 32); buffer.writeUInt16LE(16, 34);
	buffer.write("data", 36); buffer.writeUInt32LE(frames * 2, 40);
	for (let index = 0; index < frames; index += 1) {
		const envelope = 0.3 + 0.6 * Math.abs(Math.sin(index / sampleRate * Math.PI));
		buffer.writeInt16LE(Math.round(Math.sin(index / sampleRate * 2 * Math.PI * 440) * envelope * 24000), 44 + index * 2);
	}
	return buffer;
}

async function seedAudioEditor(api: ApiDriver, showId: string, request: APIRequestContext, names: string[]) {
	const rig = await seedEditor(api, showId);
	const imported = await request.post(`${api.baseUrl}/api/v2/timecodes/audio/import?name=Show%20Opener.wav`, {
		headers: { authorization: `Bearer ${api.session!.token}`, "x-tosk-show": showId, "content-type": "audio/wav" },
		data: referenceWav(),
	});
	expect(imported.ok()).toBe(true);
	const asset = await imported.json() as { asset_id: string; asset_revision: number };
	const audience: CueList = { ...rig.list, id: crypto.randomUUID(), name: "Audience Looks", cues: rig.list.cues.map((cue, index) => ({ ...cue, id: crypto.randomUUID(), name: ["Warm audience", "Audience sweep", "Audience pulse"][index] })) };
	await api.seedShowObject(showId, "cue_list", audience.id, audience);
	const audienceLane: TimecodeDefinition["lanes"][number] = { id: crypto.randomUUID(), name: "Audience Looks", content: { kind: "cue_list", cue_list_id: audience.id, clips: [{ id: crypto.randomUUID(), start_frame: 44, end_frame: 616, start_cue_id: audience.cues[0].id!, end_cue_id: audience.cues[2].id!, start_behavior: "state", end_behavior: "release", in_fade_frames: 0, out_fade_frames: 0, cue_starts: [{ cue_id: audience.cues[1].id!, offset_frame: 176 }, { cue_id: audience.cues[2].id!, offset_frame: 352 }] }] } };
	const speedLanes: TimecodeDefinition["lanes"] = names.map((name, index) => ({
		id: crypto.randomUUID(), name,
		content: { kind: "speed_group", group: String.fromCharCode(65 + index), keyframes: [{ id: crypto.randomUUID(), frame: 220, bpm: [96, 128, 72][index % 3], phase: 0 }] },
	}));
	const audioLaneId = crypto.randomUUID();
	const lanes: TimecodeDefinition["lanes"] = [...rig.definition.lanes, audienceLane, ...speedLanes, {
		id: audioLaneId, name: "Main audio", content: { kind: "audio_volume", keyframes: [
			{ id: crypto.randomUUID(), frame: 220, value: 0.8, fade_frames: 0, curve: "linear" },
		] },
	}];
	const current = await api.request<{ objects: Array<{ revision: number; definition: TimecodeDefinition }> }>("GET", "/api/v2/timecodes", undefined, true, undefined, { showId });
	await api.request("POST", "/api/v2/timecodes/actions", {
		request_id: crypto.randomUUID(), action: { type: "update", timecode_id: rig.definition.id,
			expected_revision: current.objects.find((entry) => entry.definition.id === rig.definition.id)!.revision,
			patch: { audio: { asset_id: asset.asset_id, asset_revision: asset.asset_revision, file_name: "Show Opener.wav" }, lanes },
		},
	}, true, undefined, { showId });
	return { rig, speedLanes, audioLaneId, lanes };
}

async function expandSpeedGroups(editor: Locator) {
	const toggle = editor.getByRole("button", { name: "Speed Groups", exact: true });
	if (await toggle.getAttribute("aria-expanded") === "false") await toggle.click();
}

function frameText(frame: number) {
	const totalSeconds = Math.floor(frame / 44);
	const pad = (value: number) => String(value).padStart(2, "0");
	return `${pad(Math.floor(totalSeconds / 3600))}:${pad(Math.floor(totalSeconds / 60) % 60)}:${pad(totalSeconds % 60)}.${pad(frame % 44)}`;
}

async function assertCueFrameAlignment(editor: Locator, number: number, name: string, frame: number, expanded: boolean) {
	const canvas = editor.locator(".timecode-timeline-canvas");
	const axis = await canvas.evaluate((node) => ({ left: node.getBoundingClientRect().left, origin: Number(node.getAttribute("data-time-origin-px")), scale: Number(node.getAttribute("data-pixels-per-frame")) }));
	expect(axis.origin).toBeGreaterThan(0);
	const expectedX = axis.left + axis.origin + frame * axis.scale;
	const block = editor.getByRole("button", { name: `Edit Cue ${number} ${name}`, exact: true });
	expect(Math.abs(await block.evaluate((node) => node.getBoundingClientRect().left) - expectedX)).toBeLessThanOrEqual(1);
	if (expanded) {
		const trigger = editor.getByRole("slider", { name: `Cue ${number} Trigger`, exact: true });
		await expect(trigger).toHaveAttribute("aria-valuenow", String(frame));
		const center = await trigger.evaluate((node) => { const box = node.getBoundingClientRect(); return box.left + box.width / 2; });
		expect(Math.abs(center - expectedX)).toBeLessThanOrEqual(1);
		await expect(editor.locator(".timecode-cue-editor-details").getByLabel("Trigger", { exact: true })).toHaveValue(frameText(frame));
	}
	if (frame % 44 === 0) {
		const rulerTick = editor.locator(".timecode-ruler").getByText(frameText(frame), { exact: true });
		await expect(rulerTick).toBeVisible();
		expect(Math.abs(await rulerTick.evaluate((node) => node.getBoundingClientRect().left) - expectedX)).toBeLessThanOrEqual(1);
	}
}
