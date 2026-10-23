import type { APIRequestContext, Locator, Page } from "@playwright/test";
import type { CueList } from "../apps/light-desktop/src/api/types";
import type { TimecodeDefinition } from "../apps/light-desktop/src/api/types/timecode";
import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";

// Exercise the production editor against a private bench server. Pointer movement, touch taps,
// authoritative Cuelist writes, and reopening are intentionally kept in the same operator path.
test.describe("TIMECODE-001 › overlapping Cue editor", () => {
	test.use({ hasTouch: true, viewport: { width: 1600, height: 1000 } });

	test("TIMECODE-001 @ui › touch selects one Cue, Close collapses it, and the sidebar preserves exact timeline geometry", async ({
		api,
		show,
		desk,
		page,
	}, testInfo) => {
		const rig = await seedEditor(api, show.id);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		const envelope = editor.getByRole("button", {
			name: "Edit Cue 1 House out",
			exact: true,
		});
		const viewport = editor.getByLabel("Timecode timeline viewport", {
			exact: true,
		});
		await expect(envelope).toBeVisible();
		await zoomIn(page, editor);
		await expect(editor.locator(".timecode-cue-envelope")).toHaveCount(0);
		for (const [index, name] of ["House out", "Verse wash", "Chorus"].entries())
			await assertCueFrameAlignment(
				editor,
				index + 1,
				name,
				[44, 220, 396][index],
				false,
			);
		const before = await geometry(editor, viewport, envelope);
		const add = page
			.locator(".timecode-window.timecode-editor")
			.getByRole("button", { name: "Add", exact: true });
		const addLeft = async () => (await add.boundingBox())!.x;
		const initialAddLeft = await addLeft();
		await envelope.tap();
		await expect(
			editor.getByRole("slider", { name: "Cue 1 In fade", exact: true }),
		).toBeVisible();
		await expect(editor.locator(".timecode-editor-inspector")).toBeVisible();
		expect(await addLeft()).toBe(initialAddLeft);
		await assertCueFrameAlignment(editor, 1, "House out", 44, true);
		await assertHalfGrip(
			editor,
			editor.getByRole("slider", { name: "Move clip start", exact: true }),
			44,
			"start",
		);
		for (const [buttonName, gripName] of [
			["Edit Cue 1 House out", "Move clip start"],
			["Edit Cue 2 Verse wash", "Move Cue 2 trigger"],
		]) {
			const glyphLeft = await editor
				.getByRole("button", { name: buttonName, exact: true })
				.locator(".timecode-cue-block-title b")
				.evaluate((node) => {
					const range = document.createRange();
					range.selectNodeContents(node);
					return range.getBoundingClientRect().left;
				});
			const gripRight = await editor
				.getByRole("slider", { name: gripName, exact: true })
				.locator(":scope > span")
				.evaluate((node) => node.getBoundingClientRect().right);
			expect(glyphLeft).toBeGreaterThanOrEqual(gripRight);
		}

		const inDelay = editor.getByRole("slider", {
			name: "Cue 1 In delay",
			exact: true,
		});
		await expect(inDelay).toHaveAttribute("aria-valuenow", "88");
		const triggerX = await editor
			.locator('.timecode-cue-trigger-boundary[data-trigger-frame="44"]')
			.evaluate((node) => node.getBoundingClientRect().left);
		const delayX = await inDelay.evaluate((node) => {
			const box = node.getBoundingClientRect();
			return box.left + box.width / 2;
		});
		const oneSecondWidth =
			44 *
			Number(
				await editor
					.locator(".timecode-timeline-canvas")
					.getAttribute("data-pixels-per-frame"),
			);
		expect(Math.abs(delayX - triggerX - oneSecondWidth)).toBeLessThanOrEqual(1);
		const scale = Number(
			await editor
				.locator(".timecode-timeline-canvas")
				.getAttribute("data-pixels-per-frame"),
		);
		const names = await editor
			.locator(".timecode-cue-name")
			.evaluateAll((nodes) =>
				nodes.map((node) => {
					const box = node.getBoundingClientRect();
					let opacity = 1;
					for (
						let ancestor: Element | null = node;
						ancestor;
						ancestor = ancestor.parentElement
					)
						opacity *= Number(getComputedStyle(ancestor).opacity);
					const style = getComputedStyle(node);
					return {
						left: box.left,
						width: box.width,
						opacity,
						text: node.textContent,
						boxShadow: style.boxShadow,
						borderRadius: style.borderRadius,
						textAlign: style.textAlign,
						background: style.backgroundColor,
					};
				}),
			);
		expect(names).toHaveLength(3);
		for (const [index, span] of [176, 176, 220].entries()) {
			expect(Math.abs(names[index].width - span * scale)).toBeLessThanOrEqual(
				1,
			);
			expect(names[index].opacity).toBeGreaterThanOrEqual(0.85);
			expect(names[index].boxShadow).toMatch(/2px 0px(?: 0px){2} inset/);
			expect(names[index].borderRadius).toBe("0px");
			expect(names[index].textAlign).toBe("left");
			expect(names[index].background).not.toMatch(
				/^(?:transparent|rgba\(0,\s*0,\s*0,\s*0\))$/,
			);
			if (index > 0)
				expect(
					Math.abs(
						names[index].left - names[index - 1].left - names[index - 1].width,
					),
				).toBeLessThanOrEqual(1);
		}
		expect(names.map((name) => name.text)).toEqual([
			expect.stringContaining("House out"),
			expect.stringContaining("Verse wash"),
			expect.stringContaining("Chorus"),
		]);
		await page.screenshot({
			path: testInfo.outputPath("timecode-cue-editor.png"),
		});
		await page
			.locator(".timecode-window.timecode-editor")
			.screenshot({
				path: testInfo.outputPath("timecode-cue-editor-detail.png"),
			});
		expect(await geometry(editor, viewport, envelope)).toEqual(before);
		await editor
			.getByRole("button", { name: "Edit Cue 2 Verse wash", exact: true })
			.tap({ position: { x: 32, y: 18 } });
		await expect(
			editor.getByRole("slider", { name: "Cue 2 In fade", exact: true }),
		).toBeVisible();
		expect(await addLeft()).toBe(initialAddLeft);
		await expect(
			editor.getByRole("slider", { name: "Cue 1 In fade", exact: true }),
		).toHaveCount(0);
		await editor
			.getByRole("button", { name: "Close cue editor", exact: true })
			.tap();
		await expect(
			editor.getByRole("slider", { name: "Cue 2 In fade", exact: true }),
		).toHaveCount(0);
		const closeSettings = editor.getByRole("button", {
			name: "Close selection settings",
			exact: true,
		});
		if (await closeSettings.isVisible()) await closeSettings.tap();
		await expect(editor.locator(".timecode-editor-inspector")).toHaveCount(0);
		expect(await addLeft()).toBe(initialAddLeft);
		expect(await geometry(editor, viewport, envelope)).toEqual(before);

		await setSelectionSidebar(page, true);
		// Closing a sidebar widens the viewport. At the right edge this must not let the browser
		// clamp scrollLeft and move the left-hand time origin, even though the content is wider.
		await editor
			.getByRole("button", { name: "Edit Cue 2 Verse wash", exact: true })
			.tap({ position: { x: 32, y: 18 } });
		const box = await viewport.boundingBox();
		if (!box) throw new Error("Missing timeline viewport");
		await page.mouse.move(box.x + box.width / 2, box.y + 25);
		await page.mouse.wheel(100_000, 0);
		await expect
			.poll(() => viewport.evaluate((node) => node.scrollLeft))
			.toBeGreaterThan(0);
		const rightEdge = await geometry(editor, viewport, envelope);
		await editor
			.getByRole("button", { name: "Close selection settings", exact: true })
			.tap();
		expect(await geometry(editor, viewport, envelope)).toEqual(rightEdge);
		await fitUsingOverview(page, editor);
		await expect
			.poll(() => viewport.evaluate((node) => node.scrollLeft))
			.toBe(0);
		await expect
			.poll(() =>
				viewport.evaluate((node) => node.scrollWidth - node.clientWidth),
			)
			.toBeLessThanOrEqual(1);
		await expect
			.poll(async () =>
				(
					await api.showObject<CueList>(show.id, "cue_list", rig.list.id)
				)?.body.cues.map((cue) => cue.trigger.type),
			)
			.toEqual(["manual", "manual", "manual"]);
	});

	test("TIMECODE-001 @ui › the complete expanded lane fits above the desk controls with an 88px envelope and 44px names", async ({
		api,
		show,
		desk,
		page,
		request,
	}, testInfo) => {
		await page.setViewportSize({ width: 1280, height: 800 });
		const { rig } = await seedAudioEditor(api, show.id, request, [
			"Movement",
			"Chase",
			"Colour",
		]);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		const viewport = editor.getByLabel("Timecode timeline viewport", {
			exact: true,
		});
		const lane = editor.locator(
			`.timecode-editor-lane[data-lane-id="${rig.definition.lanes[0].id}"]`,
		);
		const cue = lane.getByRole("button", {
			name: "Edit Cue 1 House out",
			exact: true,
		});
		await cue.scrollIntoViewIfNeeded();
		const before = await geometry(editor, viewport, cue);
		await cue.tap({ position: { x: 32, y: 18 } });
		await expect(
			lane.getByRole("slider", { name: "Cue 1 In fade", exact: true }),
		).toBeVisible();
		expect(await geometry(editor, viewport, cue)).toEqual(before);
		for (const cueNumber of [1, 3]) {
			if (cueNumber === 3)
				await lane
					.getByRole("button", { name: "Edit Cue 3 Chorus", exact: true })
					.click({ position: { x: 32, y: 18 } });
			await expect
				.poll(() =>
					lane.evaluate((node) => {
						const box = node.getBoundingClientRect();
						const viewport = node.closest(".timecode-timeline-scroll")!;
						const bounds = viewport.getBoundingClientRect();
						const audio = viewport
							.querySelector(".main-audio")
							?.getBoundingClientRect();
						return (
							box.top >=
								Math.max(
									bounds.top + viewport.clientTop + 32,
									audio?.bottom ?? 0,
								) -
									1 &&
							box.bottom <=
								bounds.top + viewport.clientTop + viewport.clientHeight + 1
						);
					}),
				)
				.toBe(true);
			const envelopeHeight = await lane
				.locator(".timecode-cue-envelope")
				.first()
				.evaluate((node) => node.getBoundingClientRect().height);
			const nameHeight = await cue.evaluate(
				(node) => node.getBoundingClientRect().height,
			);
			expect(envelopeHeight).toBe(88);
			expect(nameHeight).toBe(44);
			expect(envelopeHeight / nameHeight).toBe(2);
			expect(await geometry(editor, viewport, cue)).toEqual(before);
		}
		await expect(
			page.locator('[data-keypad-key="SHIFT"]:visible').first(),
		).toBeVisible();
		await page
			.locator(".timecode-window.timecode-editor")
			.screenshot({
				path: testInfo.outputPath(
					"timecode-expanded-small-viewport-detail.png",
				),
			});
	});

	test("TIMECODE-001 @ui › tapping an unplaced manual Cue divider never creates a placement or sends a save", async ({
		api,
		show,
		desk,
		page,
	}) => {
		const rig = await seedEditor(api, show.id);
		const collection = await api.request<{
			objects: Array<{ revision: number; definition: TimecodeDefinition }>;
		}>("GET", "/api/v2/timecodes", undefined, true, undefined, {
			showId: show.id,
		});
		const lanes = structuredClone(rig.definition.lanes);
		if (lanes[0].content.kind !== "cue_list")
			throw new Error("Missing Cuelist");
		lanes[0].content.clips[0].cue_starts =
			lanes[0].content.clips[0].cue_starts.filter(
				(start) => start.cue_id !== rig.list.cues[1].id,
			);
		await api.request(
			"POST",
			"/api/v2/timecodes/actions",
			{
				request_id: crypto.randomUUID(),
				action: {
					type: "update",
					timecode_id: rig.definition.id,
					expected_revision: collection.objects.find(
						(entry) => entry.definition.id === rig.definition.id,
					)!.revision,
					patch: { lanes },
				},
			},
			true,
			undefined,
			{ showId: show.id },
		);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await editor
			.getByRole("button", { name: "Edit Cue 1 House out", exact: true })
			.click({ position: { x: 32, y: 18 } });
		const before = await readClip(api, show.id, rig.definition.id);
		let saves = 0;
		page.on("request", (request) => {
			if (
				request.method() === "POST" &&
				request.url().endsWith("/api/v2/timecodes/actions")
			)
				saves += 1;
		});
		const divider = editor.getByRole("slider", {
			name: "Move Cue 2 trigger",
			exact: true,
		});
		await divider.click();
		await divider.tap();
		await page.waitForTimeout(150);
		expect(saves).toBe(0);
		expect(await readClip(api, show.id, rig.definition.id)).toEqual(before);
		expect(
			(await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body
				.cues[1].trigger.type,
		).toBe("manual");
	});

	test("TIMECODE-001 @ui › interior Cue dividers move the real Trigger with mouse and touch and persist", async ({
		api,
		show,
		desk,
		page,
	}) => {
		const rig = await seedEditor(api, show.id);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		await editor
			.getByRole("button", { name: "Edit Cue 2 Verse wash", exact: true })
			.click({ position: { x: 32, y: 18 } });
		const divider = editor.getByRole("slider", {
			name: "Move Cue 2 trigger",
			exact: true,
		});
		await expect(divider).toHaveAttribute("aria-valuenow", "220");
		await assertGripBars(editor, divider, 220);
		const bars = await gripMetrics(divider);
		const leftBarOffset = bars.bars[0].center - (bars.left + bars.width / 2);
		const rightBarOffset = bars.bars[1].center - (bars.left + bars.width / 2);
		await dragFrames(page, editor, divider, 22, undefined, leftBarOffset);
		await expect
			.poll(
				async () =>
					(await readClip(api, show.id, rig.definition.id)).cue_starts.find(
						(start) => start.cue_id === rig.list.cues[1].id,
					)?.offset_frame,
			)
			.toBe(198);
		await touchDragFrames(page, editor, divider, 22, false, rightBarOffset);
		await expect
			.poll(
				async () =>
					(await readClip(api, show.id, rig.definition.id)).cue_starts.find(
						(start) => start.cue_id === rig.list.cues[1].id,
					)?.offset_frame,
			)
			.toBe(220);
		await assertCueFrameAlignment(editor, 2, "Verse wash", 264, true);
		await page.reload();
		await openEditor(page);
		await editor
			.getByRole("button", { name: "Edit Cue 2 Verse wash", exact: true })
			.click({ position: { x: 32, y: 18 } });
		await expect(divider).toHaveAttribute("aria-valuenow", "264");
		expect(
			(await api.showObject<CueList>(
				show.id,
				"cue_list",
				rig.list.id,
			))!.body.cues.map((cue) => cue.trigger.type),
		).toEqual(["manual", "manual", "manual"]);
	});

	test("TIMECODE-001 @ui › visible timing dots stay on the four actual slope vertices with delayed and zero-delay In", async ({
		api,
		show,
		desk,
		page,
	}, testInfo) => {
		const rig = await seedEditor(api, show.id);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		await editor
			.getByRole("button", { name: "Edit Cue 1 House out", exact: true })
			.tap();
		await assertVisibleCueCorners(editor, 1);
		await page
			.locator(".timecode-window.timecode-editor")
			.screenshot({
				path: testInfo.outputPath("timecode-true-corners-delayed.png"),
			});
		const start = editor
			.locator(".timecode-cue-editor-details")
			.getByLabel("In start", { exact: true });
		await start.fill(frameText(44));
		await start.press("Tab");
		await expect
			.poll(
				async () =>
					(await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!
						.body.cues[0].delay_millis,
			)
			.toBe(0);
		await assertVisibleCueCorners(editor, 1);
		await page
			.locator(".timecode-window.timecode-editor")
			.screenshot({
				path: testInfo.outputPath("timecode-true-corners-zero-delay.png"),
			});
		await touchDragFrames(
			page,
			editor,
			editor.getByRole("slider", { name: "Cue 1 In delay", exact: true }),
			4,
		);
		await assertVisibleCueCorners(editor, 1);
	});

	test("TIMECODE-001 @ui › each timing point saves to the Cuelist and a refused pointer edit rolls back", async ({
		api,
		show,
		desk,
		page,
	}) => {
		const rig = await seedEditor(api, show.id);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		await editor
			.getByRole("button", { name: "Edit Cue 1 House out", exact: true })
			.click();
		const readCue = async () =>
			(await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body
				.cues[0];
		for (const [label, field] of [
			["In delay", "delay_millis"],
			["In fade", "fade_millis"],
			["Out delay", "out_delay_millis"],
			["Out fade", "out_fade_millis"],
		] as const) {
			const previous = (await readCue())[field];
			await dragFrames(
				page,
				editor,
				editor.getByRole("slider", { name: `Cue 1 ${label}`, exact: true }),
				4,
			);
			await expect
				.poll(async () => (await readCue())[field])
				.not.toBe(previous);
		}
		await dragFrames(
			page,
			editor,
			editor.getByRole("slider", { name: "Cue 1 Out delay", exact: true }),
			-600,
		);
		await expect.poll(async () => (await readCue()).out_delay_millis).toBe(0);
		await expect(
			editor.getByRole("slider", { name: "Cue 1 Out delay", exact: true }),
		).toHaveAttribute("aria-valuenow", "220");
		const saved = await readCue();
		await page.reload();
		await openEditor(page);
		await zoomIn(page, editor);
		await editor
			.getByRole("button", { name: "Edit Cue 1 House out", exact: true })
			.click();
		const frameOf = (millis: number | null | undefined) =>
			Math.round(((millis ?? 0) * 44) / 1000);
		for (const [label, frame] of [
			["In delay", 44 + frameOf(saved.delay_millis)],
			[
				"In fade",
				44 + frameOf(saved.delay_millis) + frameOf(saved.fade_millis),
			],
			["Out delay", 220 + frameOf(saved.out_delay_millis)],
			[
				"Out fade",
				220 + frameOf(saved.out_delay_millis) + frameOf(saved.out_fade_millis),
			],
		] as const)
			await expect(
				editor.getByRole("slider", { name: `Cue 1 ${label}`, exact: true }),
			).toHaveAttribute("aria-valuenow", String(frame));
		expect(await readCue()).toEqual(saved);

		const handle = editor.getByRole("slider", {
			name: "Cue 1 In fade",
			exact: true,
		});
		const before = await handle.getAttribute("aria-valuenow");
		let refused = 0;
		await page.route("**/api/v2/playback-topology/actions", async (route) => {
			if (route.request().postDataJSON()?.action?.type !== "save_cue_list")
				return route.continue();
			refused += 1;
			await route.fulfill({
				status: 503,
				contentType: "application/json",
				body: JSON.stringify({
					kind: "unavailable",
					error: "Test desk refused Cue timing",
					current_revision: null,
					current_related_revision: null,
					retryable: false,
				}),
			});
		});
		await dragFrames(page, editor, handle, 4);
		await expect.poll(() => refused).toBeGreaterThan(0);
		await expect(
			page.getByText(/Test desk refused Cue timing/).first(),
		).toBeVisible();
		await expect(handle).toHaveAttribute("aria-valuenow", before!);
		expect(await readCue()).toEqual(saved);
	});

	test("TIMECODE-001 @ui › manual Trigger drags stop at neighbours and survive reopening without changing trigger type", async ({
		api,
		show,
		desk,
		page,
	}) => {
		const rig = await seedEditor(api, show.id);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		await editor
			.getByRole("button", { name: "Edit Cue 2 Verse wash", exact: true })
			.click();
		const trigger = editor.getByRole("slider", {
			name: "Move Cue 2 trigger",
			exact: true,
		});
		await expect(trigger).toBeVisible();
		await assertCueFrameAlignment(editor, 2, "Verse wash", 220, true);
		await touchDragFrames(page, editor, trigger, 44, true);
		await assertCueFrameAlignment(editor, 2, "Verse wash", 220, true);
		expect(
			(await readClip(api, show.id, rig.definition.id)).cue_starts.find(
				(start) => start.cue_id === rig.list.cues[1].id,
			)?.offset_frame,
		).toBe(176);
		await dragFrames(page, editor, trigger, 44, async () => {
			await assertCueFrameAlignment(editor, 2, "Verse wash", 264, true);
			expect(
				(await readClip(api, show.id, rig.definition.id)).cue_starts.find(
					(start) => start.cue_id === rig.list.cues[1].id,
				)?.offset_frame,
			).toBe(176);
		});
		await expect
			.poll(
				async () =>
					(await readClip(api, show.id, rig.definition.id)).cue_starts.find(
						(start) => start.cue_id === rig.list.cues[1].id,
					)?.offset_frame,
			)
			.toBe(220);
		await assertCueFrameAlignment(editor, 2, "Verse wash", 264, true);
		// Prove the saved editor position drives the authoritative playback boundary.
		const context = { showId: show.id, deskId: api.session!.desk.id };
		const transport = (action: { type: string; frame?: number }) =>
			api.request(
				"POST",
				`/api/v2/timecodes/${rig.definition.id}/transport`,
				{ timecode_id: rig.definition.id, action },
				true,
				undefined,
				context,
			);
		const currentCue = async () =>
			(
				await api.request<{
					cue_list_clips: Array<{
						clip_id: string;
						cue_id: string;
						cue_start_frame: number;
						state: string;
					}>;
				}>(
					"GET",
					`/api/v2/timecodes/${rig.definition.id}/runtime`,
					undefined,
					true,
					undefined,
					context,
				)
			).cue_list_clips.find(
				(clip) =>
					clip.clip_id ===
					(rig.definition.lanes[0].content.kind === "cue_list"
						? rig.definition.lanes[0].content.clips[0].id
						: ""),
			);
		await transport({ type: "go" });
		await transport({ type: "pause" });
		await transport({ type: "seek", frame: 263 });
		await expect
			.poll(currentCue)
			.toMatchObject({ cue_id: rig.list.cues[0].id, state: "active" });
		await transport({ type: "seek", frame: 264 });
		await expect
			.poll(currentCue)
			.toMatchObject({
				cue_id: rig.list.cues[1].id,
				cue_start_frame: 264,
				state: "active",
			});
		await transport({ type: "stop" });

		await page.reload();
		await openEditor(page);
		await zoomIn(page, editor);
		await editor
			.getByRole("button", { name: "Edit Cue 2 Verse wash", exact: true })
			.tap({ position: { x: 32, y: 18 } });
		await assertCueFrameAlignment(editor, 2, "Verse wash", 264, true);
		await dragFrames(page, editor, trigger, 300);
		const clip = async () => {
			const collection = await api.request<{
				objects: Array<{ definition: TimecodeDefinition }>;
			}>("GET", "/api/v2/timecodes", undefined, true, undefined, {
				showId: show.id,
			});
			const definition = collection.objects.find(
				(object) => object.definition.id === rig.definition.id,
			)!.definition;
			const lane = definition.lanes[0];
			if (lane.content.kind !== "cue_list")
				throw new Error("Missing Cuelist lane");
			return lane.content.clips[0];
		};
		await expect
			.poll(
				async () =>
					(await clip()).cue_starts.find(
						(start) => start.cue_id === rig.list.cues[1].id,
					)?.offset_frame,
			)
			.toBe(351);
		await dragFrames(page, editor, trigger, -600);
		await expect
			.poll(
				async () =>
					(await clip()).cue_starts.find(
						(start) => start.cue_id === rig.list.cues[1].id,
					)?.offset_frame,
			)
			.toBe(1);
		await page.reload();
		await openEditor(page);
		await editor
			.getByRole("button", { name: "Edit Cue 2 Verse wash", exact: true })
			.click();
		await expect(
			editor.getByRole("slider", { name: "Move Cue 2 trigger", exact: true }),
		).toHaveAttribute("aria-valuenow", "45");
		expect(
			(await api.showObject<CueList>(
				show.id,
				"cue_list",
				rig.list.id,
			))!.body.cues.map((cue) => cue.trigger.type),
		).toEqual(["manual", "manual", "manual"]);
	});
	test("TIMECODE-001 @ui › touch drags each timing point and cancelling a touch restores the unsaved preview", async ({
		api,
		show,
		desk,
		page,
	}) => {
		const rig = await seedEditor(api, show.id);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		await editor
			.getByRole("button", { name: "Edit Cue 1 House out", exact: true })
			.tap();
		const readCue = async () =>
			(await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body
				.cues[0];
		const initial = await readCue();
		const handle = editor.getByRole("slider", {
			name: "Cue 1 In fade",
			exact: true,
		});
		const initialFrame = await handle.getAttribute("aria-valuenow");
		await touchDragFrames(page, editor, handle, 4, true);
		await expect(handle).toHaveAttribute("aria-valuenow", initialFrame!);
		expect(await readCue()).toEqual(initial);
		for (const [label, field] of [
			["In delay", "delay_millis"],
			["In fade", "fade_millis"],
			["Out delay", "out_delay_millis"],
			["Out fade", "out_fade_millis"],
		] as const) {
			const before = (await readCue())[field];
			await touchDragFrames(
				page,
				editor,
				editor.getByRole("slider", { name: `Cue 1 ${label}`, exact: true }),
				4,
			);
			await expect.poll(async () => (await readCue())[field]).not.toBe(before);
		}
	});

	test("TIMECODE-001 @ui › first Trigger moves the whole clip and the final Cue explains its separate Clip Out", async ({
		api,
		show,
		desk,
		page,
	}) => {
		const rig = await seedEditor(api, show.id);
		const originalCues = (await api.showObject<CueList>(
			show.id,
			"cue_list",
			rig.list.id,
		))!.body.cues;
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		await editor
			.getByRole("button", { name: "Edit Cue 1 House out", exact: true })
			.click();
		const firstTrigger = editor.getByRole("slider", {
			name: "Move clip start",
			exact: true,
		});
		await expect(
			editor.getByRole("slider", { name: "Cue 1 Trigger", exact: true }),
		).toHaveCount(0);
		await touchDragFrames(page, editor, firstTrigger, 44, true);
		await assertCueFrameAlignment(editor, 1, "House out", 44, true);
		expect((await readClip(api, show.id, rig.definition.id)).start_frame).toBe(
			44,
		);
		await dragFrames(page, editor, firstTrigger, 44, async () => {
			await assertCueFrameAlignment(editor, 1, "House out", 88, true);
			await assertCueFrameAlignment(editor, 2, "Verse wash", 264, false);
			const saved = await readClip(api, show.id, rig.definition.id);
			expect(saved.start_frame).toBe(44);
			expect(saved.end_frame).toBe(616);
		});
		await expect
			.poll(
				async () =>
					(await readClip(api, show.id, rig.definition.id)).start_frame,
			)
			.toBe(88);
		const moved = await readClip(api, show.id, rig.definition.id);
		expect(moved.end_frame).toBe(660);
		expect(moved.cue_starts).toEqual(
			rig.definition.lanes[0].content.kind === "cue_list"
				? rig.definition.lanes[0].content.clips[0].cue_starts
				: [],
		);
		const triggerField = editor
			.locator(".timecode-cue-editor-details")
			.getByLabel("Trigger", { exact: true });
		await triggerField.fill(frameText(132));
		await triggerField.press("Tab");
		await expect
			.poll(
				async () =>
					(await readClip(api, show.id, rig.definition.id)).start_frame,
			)
			.toBe(132);
		const exactMoved = await readClip(api, show.id, rig.definition.id);
		expect(exactMoved.end_frame - exactMoved.start_frame).toBe(
			moved.end_frame - moved.start_frame,
		);
		expect(exactMoved.cue_starts).toEqual(moved.cue_starts);
		await assertCueFrameAlignment(editor, 1, "House out", 132, true);
		await assertCueFrameAlignment(editor, 2, "Verse wash", 308, false);
		await page.reload();
		await openEditor(page);
		await editor
			.getByRole("button", { name: "Edit Cue 1 House out", exact: true })
			.click({ position: { x: 32, y: 18 } });
		await expect(firstTrigger).toHaveAttribute("aria-valuenow", "132");

		await editor
			.getByRole("button", { name: "Edit Cue 3 Chorus", exact: true })
			.click();
		await expect(
			editor.locator(".timecode-cue-edit-rail.out.reference"),
		).toContainText("Out · not executed");
		const details = editor.locator(".timecode-cue-editor-details");
		await expect(details).toContainText("It is not played at this clip’s end.");
		await details
			.getByLabel("Clip Out duration", { exact: true })
			.fill("00:00:01.00");
		await details.getByLabel("Clip Out duration", { exact: true }).press("Tab");
		await expect
			.poll(
				async () =>
					(await readClip(api, show.id, rig.definition.id)).out_fade_frames,
			)
			.toBe(44);
		expect((await readClip(api, show.id, rig.definition.id)).end_frame).toBe(
			704,
		);
		expect(
			(await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body
				.cues,
		).toEqual(originalCues);
	});

	test("TIMECODE-001 @ui › open Speed Groups retain context and Cue focus dims only other editing lanes", async ({
		api,
		show,
		desk,
		page,
		request,
	}, testInfo) => {
		const { rig, speedLanes } = await seedAudioEditor(api, show.id, request, [
			"Movement",
			"Chase",
			"Colour",
		]);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		const window = page.locator(".timecode-window.timecode-editor");
		await expect(
			editor.getByLabel("Selected lane and keyframe actions"),
		).toHaveCount(0);
		const section = editor.locator(".timecode-speed-section");
		const movement = editor.locator(
			`[data-lane-id="${speedLanes[0].id}"].timecode-editor-lane`,
		);
		const chase = editor.locator(
			`[data-lane-id="${speedLanes[1].id}"].timecode-editor-lane`,
		);
		const color = (lane: Locator) =>
			lane.evaluate((node) => getComputedStyle(node).backgroundColor);
		await movement.getByRole("button", { name: /^Movement/ }).tap();
		await expect(movement).toHaveClass(/lane-expanded/);
		expect(await color(movement)).not.toBe(await color(chase));
		await window
			.getByRole("button", { name: "Next Keyframe", exact: true })
			.tap();
		await expect(
			movement.locator(".timecode-timeline-item.selected"),
		).toHaveCount(1);
		await editor
			.getByRole("button", { name: "Close speed group editor", exact: true })
			.tap();
		await expect(section).toHaveClass(/section-expanded/);
		await expect(movement).not.toHaveClass(/lane-expanded/);
		expect(await color(movement)).toBe(await color(chase));
		expect(await color(movement)).not.toBe("rgba(0, 0, 0, 0)");
		await window.screenshot({
			path: testInfo.outputPath("timecode-speed-context-detail.png"),
		});
		await editor
			.getByRole("button", { name: "Edit Cue 1 House out", exact: true })
			.click({ position: { x: 32, y: 18 } });
		const active = editor.locator(
			`[data-lane-id="${rig.definition.lanes[0].id}"].timecode-editor-lane`,
		);
		const other = editor.locator(".lane-cue_list").nth(1);
		const opacity = (lane: Locator) =>
			lane.evaluate((node) => Number(getComputedStyle(node).opacity));
		expect(await opacity(section)).toBe(1);
		expect(await opacity(other)).toBe(1);
		await expect
			.poll(() => opacity(other.locator(".timecode-timeline-item").first()))
			.toBeLessThan(0.5);
		await expect
			.poll(() =>
				opacity(
					movement.locator(":scope > :not(.timecode-editor-lane-label)").last(),
				),
			)
			.toBeLessThan(0.5);
		expect(await opacity(other.locator(".timecode-editor-lane-label"))).toBe(1);
		expect(await opacity(active)).toBe(1);
		expect(await opacity(editor.locator(".main-audio"))).toBe(1);
		const inspector = editor.locator(".timecode-editor-inspector");
		const sectionHeaders = await inspector
			.locator("summary")
			.evaluateAll((nodes) =>
				nodes.map((node) => {
					const box = node.getBoundingClientRect();
					return {
						name: node.textContent,
						left: box.left,
						width: box.width,
						height: box.height,
					};
				}),
			);
		expect(sectionHeaders.map((header) => header.name)).toEqual([
			"Cue In",
			"Cue Out",
			"Cuelist",
		]);
		for (const header of sectionHeaders) {
			expect(
				Math.abs(header.left - sectionHeaders[0].left),
			).toBeLessThanOrEqual(1);
			expect(
				Math.abs(header.width - sectionHeaders[0].width),
			).toBeLessThanOrEqual(1);
			expect(header.height).toBeGreaterThanOrEqual(44);
		}
		for (const name of ["Cue In", "Cue Out", "Cuelist"]) {
			const detail = inspector
				.locator("details")
				.filter({
					has: page
						.locator("summary")
						.filter({ hasText: new RegExp(`^${name}$`) }),
				});
			const summary = inspector
				.locator("summary")
				.filter({ hasText: new RegExp(`^${name}$`) });
			const wasOpen = await detail.getAttribute("open");
			await summary.tap();
			if (wasOpen === null) await expect(detail).toHaveAttribute("open", "");
			else await expect(detail).not.toHaveAttribute("open", "");
			await summary.click();
		}
		await inspector
			.locator("summary")
			.filter({ hasText: /^Cue In$/ })
			.tap();
		await inspector
			.locator("summary")
			.filter({ hasText: /^Cue Out$/ })
			.tap();
		const panel = (await inspector.boundingBox())!;
		await page.mouse.move(
			panel.x + panel.width / 2,
			panel.y + panel.height / 2,
		);
		await page.mouse.wheel(0, -10000);
		await window.screenshot({
			path: testInfo.outputPath("timecode-focus-sidebar-detail.png"),
		});
	});

	test("TIMECODE-001 @ui › the last Cue end grip preserves earlier triggers and retains a refused autosave draft", async ({
		api,
		show,
		desk,
		page,
	}, testInfo) => {
		const rig = await seedEditor(api, show.id);
		const originalCues = structuredClone(
			(await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body
				.cues,
		);
		const originalClip = await readClip(api, show.id, rig.definition.id);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await editor
			.getByRole("button", { name: "Edit Cue 3 Chorus", exact: true })
			.click({ position: { x: 32, y: 18 } });
		const end = editor.getByRole("slider", {
			name: "Extend last Cue",
			exact: true,
		});
		const divider = editor.getByRole("slider", {
			name: "Move Cue 3 trigger",
			exact: true,
		});
		const centerY = (handle: Locator) =>
			handle.evaluate((node) => {
				const box = node.getBoundingClientRect();
				return box.top + box.height / 2;
			});
		expect(
			Math.abs((await centerY(end)) - (await centerY(divider))),
		).toBeLessThanOrEqual(1);
		const nameBand = editor.getByRole("button", {
			name: "Edit Cue 3 Chorus",
			exact: true,
		});
		const nameBox = (await nameBand.boundingBox())!;
		expect(await centerY(end)).toBeGreaterThanOrEqual(nameBox.y);
		expect(await centerY(end)).toBeLessThanOrEqual(nameBox.y + nameBox.height);
		await dragFrames(page, editor, end, 44);
		await expect
			.poll(
				async () => (await readClip(api, show.id, rig.definition.id)).end_frame,
			)
			.toBe(660);
		await touchDragFrames(page, editor, end, 44);
		await expect
			.poll(
				async () => (await readClip(api, show.id, rig.definition.id)).end_frame,
			)
			.toBe(704);
		expect(
			(await readClip(api, show.id, rig.definition.id)).cue_starts,
		).toEqual(originalClip.cue_starts);
		expect(
			(await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body
				.cues,
		).toEqual(originalCues);
		await page.route("**/api/v2/timecodes/actions", (route) =>
			route.request().method() === "POST"
				? route.fulfill({
						status: 503,
						contentType: "application/json",
						body: JSON.stringify({
							error: { code: "save_refused", message: "Refused end extension" },
						}),
					})
				: route.continue(),
		);
		await dragFrames(page, editor, end, 4);
		await expect(
			page.getByText(/Refused end extension|503/).first(),
		).toBeVisible();
		await expect(page.getByText("Not saved", { exact: true })).toBeVisible();
		await expect(
			page.getByRole("button", { name: "Retry autosave", exact: true }),
		).toBeVisible();
		await expect(end).toHaveAttribute("aria-valuenow", "708");
		expect((await readClip(api, show.id, rig.definition.id)).end_frame).toBe(
			704,
		);
		await page.unroute("**/api/v2/timecodes/actions");
		await page.reload();
		await openEditor(page);
		await editor
			.getByRole("button", { name: "Edit Cue 3 Chorus", exact: true })
			.click({ position: { x: 32, y: 18 } });
		await expect(end).toHaveAttribute("aria-valuenow", "704");
		const collection = await api.request<{
			objects: Array<{ revision: number; definition: TimecodeDefinition }>;
		}>("GET", "/api/v2/timecodes", undefined, true, undefined, {
			showId: show.id,
		});
		await api.request(
			"POST",
			"/api/v2/timecodes/actions",
			{
				request_id: crypto.randomUUID(),
				action: {
					type: "update",
					timecode_id: rig.definition.id,
					expected_revision: collection.objects.find(
						(entry) => entry.definition.id === rig.definition.id,
					)!.revision,
					patch: { duration_frame: 704 },
				},
			},
			true,
			undefined,
			{ showId: show.id },
		);
		await page.reload();
		await openEditor(page);
		await editor
			.getByRole("button", { name: "Edit Cue 3 Chorus", exact: true })
			.click({ position: { x: 32, y: 18 } });
		const viewport = editor.getByLabel("Timecode timeline viewport", {
			exact: true,
		});
		const scrollBox = (await viewport.boundingBox())!;
		await page.mouse.move(
			scrollBox.x + scrollBox.width / 2,
			scrollBox.y + scrollBox.height / 2,
		);
		await page.mouse.wheel(100000, 0);
		await expect
			.poll(() =>
				end.evaluate((node) => {
					const target = node.getBoundingClientRect();
					const scroll = node.closest(".timecode-timeline-scroll")!;
					const box = scroll.getBoundingClientRect();
					return (
						target.width === 44 &&
						target.left >= box.left &&
						target.right <=
							box.left + scroll.clientLeft + scroll.clientWidth + 1
					);
				}),
			)
			.toBe(true);
		await assertHalfGrip(editor, end, 704, "end");
		await touchDragFrames(page, editor, end, -4, true);
		expect((await readClip(api, show.id, rig.definition.id)).end_frame).toBe(
			704,
		);
		await page
			.locator(".timecode-window.timecode-editor")
			.screenshot({
				path: testInfo.outputPath("timecode-end-grabber-detail.png"),
			});
		await page
			.getByRole("button", { name: "Delete clip", exact: true })
			.click();
		await expect(editor.locator(".timecode-cue-block")).toHaveCount(0);
	});

	test("TIMECODE-001 @ui › marker lines do not drag while the sticky ruler flag supports mouse and touch moves", async ({
		api,
		show,
		desk,
		page,
		request,
	}, testInfo) => {
		const { rig } = await seedAudioEditor(
			api,
			show.id,
			request,
			Array.from(
				{ length: 12 },
				(_, index) =>
					["Movement", "Chase", "Colour"][index % 3] + ` ${index + 1}`,
			),
		);
		const markerId = crypto.randomUUID();
		const current = await api.request<{
			objects: Array<{ revision: number; definition: TimecodeDefinition }>;
		}>("GET", "/api/v2/timecodes", undefined, true, undefined, {
			showId: show.id,
		});
		await api.request(
			"POST",
			"/api/v2/timecodes/actions",
			{
				request_id: crypto.randomUUID(),
				action: {
					type: "update",
					timecode_id: rig.definition.id,
					expected_revision: current.objects.find(
						(entry) => entry.definition.id === rig.definition.id,
					)!.revision,
					patch: {
						markers: [
							{ id: markerId, frame: 220, name: "Verse", color: "#ffffff" },
						],
					},
				},
			},
			true,
			undefined,
			{ showId: show.id },
		);
		const markerFrame = async () =>
			(
				await api.request<{
					objects: Array<{ definition: TimecodeDefinition }>;
				}>("GET", "/api/v2/timecodes", undefined, true, undefined, {
					showId: show.id,
				})
			).objects.find((entry) => entry.definition.id === rig.definition.id)!
				.definition.markers[0].frame;
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		const viewport = editor.getByLabel("Timecode timeline viewport", {
			exact: true,
		});
		const marker = editor.locator(".timecode-timeline-marker");
		const flag = marker.locator(".timecode-timeline-marker-label");
		await expect(flag).toContainText("Verse");
		await expect(flag.locator(".timecode-marker-grab")).toBeVisible();
		const audio = (await editor.locator(".main-audio").boundingBox())!;
		const line = (await marker
			.locator(".timecode-timeline-marker-line")
			.boundingBox())!;
		expect(
			await marker
				.locator(".timecode-timeline-marker-line")
				.evaluate((node) => Number(getComputedStyle(node).opacity)),
		).toBe(0.5);
		expect(
			await page.evaluate(
				({ x, y }) =>
					Boolean(
						document
							.elementFromPoint(x, y)
							?.closest(".timecode-timeline-marker"),
					),
				{ x: line.x, y: audio.y + audio.height - 8 },
			),
		).toBe(false);
		await page.mouse.move(line.x, audio.y + audio.height - 8);
		await page.mouse.down();
		await page.mouse.move(line.x + 40, audio.y + audio.height - 8, {
			steps: 5,
		});
		await page.mouse.up();
		expect(await markerFrame()).toBe(220);
		const top = await flag.evaluate((node) => node.getBoundingClientRect().top);
		const ruler = editor.locator(".timecode-timeline-heading");
		const rulerBox = (await ruler.boundingBox())!;
		expect(top).toBeGreaterThanOrEqual(rulerBox.y + 32);
		expect(top + 44).toBeLessThanOrEqual(rulerBox.y + rulerBox.height + 1);
		const box = (await viewport.boundingBox())!;
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.wheel(0, 300);
		await expect
			.poll(() => viewport.evaluate((node) => node.scrollTop))
			.toBeGreaterThan(0);
		expect(
			Math.abs(
				(await flag.evaluate((node) => node.getBoundingClientRect().top)) - top,
			),
		).toBeLessThanOrEqual(1);
		expect((await ruler.boundingBox())!.y).toBe(rulerBox.y);
		await dragFrames(page, editor, flag, 44);
		await expect.poll(markerFrame).toBe(264);
		await touchDragFrames(page, editor, flag, 44);
		await expect.poll(markerFrame).toBe(308);
		const context = { showId: show.id, deskId: api.session!.desk.id };
		const transport = (action: { type: string; frame?: number }) =>
			api.request(
				"POST",
				`/api/v2/timecodes/${rig.definition.id}/transport`,
				{ timecode_id: rig.definition.id, action },
				true,
				undefined,
				context,
			);
		await transport({ type: "go" });
		await transport({ type: "pause" });
		await transport({ type: "seek", frame: 320 });
		await expect(
			page.getByRole("button", { name: "Timecode position", exact: true }),
		).toHaveText(frameText(320));
		const playhead = editor.getByRole("button", {
			name: "Drag playhead to seek",
			exact: true,
		});
		await expect
			.poll(() =>
				playhead.evaluate((node) => {
					const box = node.getBoundingClientRect();
					const audio = node
						.closest(".timecode-timeline-canvas")!
						.querySelector(".main-audio")!
						.getBoundingClientRect();
					const flag = node
						.closest(".timecode-timeline-canvas")!
						.querySelector(".timecode-timeline-marker-label")!
						.getBoundingClientRect();
					return (
						box.left > flag.left &&
						box.left < flag.right &&
						[audio.top + audio.height / 2, flag.top + flag.height / 2].every(
							(y) =>
								Boolean(
									document
										.elementFromPoint(box.left + box.width / 2, y)
										?.closest(".timecode-editor-playhead"),
								),
						)
					);
				}),
			)
			.toBe(true);

		await page
			.locator(".timecode-window.timecode-editor")
			.screenshot({
				path: testInfo.outputPath("timecode-marker-ruler-flag.png"),
			});
	});

	test("TIMECODE-001 @ui › a long final Cue Out reference remains reachable after its independent horizontal scroll", async ({
		api,
		show,
		desk,
		page,
	}) => {
		const rig = await seedEditor(api, show.id);
		const record = (await api.showObject<CueList>(
			show.id,
			"cue_list",
			rig.list.id,
		))!;
		rig.list.cues[2].out_fade_millis = 30_000;
		await api.seedShowObject(
			show.id,
			"cue_list",
			rig.list.id,
			rig.list,
			record.revision,
		);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		await editor
			.getByRole("button", { name: "Edit Cue 3 Chorus", exact: true })
			.click({ position: { x: 32, y: 18 } });
		const reference = editor.locator(".timecode-cue-edit-rail.out.reference");
		const readCue = async () =>
			(await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body
				.cues[2];
		for (const [label, field] of [
			["Out fade", "out_fade_millis"],
			["Out delay", "out_delay_millis"],
		] as const) {
			const handle = editor.getByRole("slider", {
				name: `Cue 3 ${label}`,
				exact: true,
			});
			await handle.scrollIntoViewIfNeeded();
			// Native scrollIntoView does not account for the sticky lane-name column.
			// Pan the main timeline with the real wheel before touching an occluded point.
			const viewport = editor.getByLabel("Timecode timeline viewport", { exact: true });
			const viewportBox = (await viewport.boundingBox())!;
			const handleBox = (await handle.boundingBox())!;
			const coveredByHeader = viewportBox.x + 160 - handleBox.x;
			if (coveredByHeader > 0) {
				await page.mouse.move(viewportBox.x + viewportBox.width / 2, viewportBox.y + viewportBox.height - 30);
				await page.mouse.wheel(-coveredByHeader - 16, 0);
				await expect.poll(async () => (await handle.boundingBox())!.x).toBeGreaterThanOrEqual(viewportBox.x + 160);
			}
			await expect
				.poll(() =>
					handle.evaluate((node) => {
						const box = node.getBoundingClientRect();
						return node.contains(
							document.elementFromPoint(
								box.left + box.width / 2,
								box.top + box.height / 2,
							),
						);
					}),
				)
				.toBe(true);
			if (label === "Out fade")
				await expect
					.poll(() => reference.evaluate((node) => node.scrollLeft))
					.toBeGreaterThan(0);
			const before = (await readCue())[field];
			await touchDragFrames(page, editor, handle, 4);
			await expect
				.poll(async () => (await readCue())[field])
				.toBeGreaterThan(before ?? 0);
		}
		expect((await readClip(api, show.id, rig.definition.id)).end_frame).toBe(
			616,
		);
		expect((await readCue()).trigger.type).toBe("manual");
	});

	test("TIMECODE-001 @ui › the main audio waveform and volume points stay below the ruler while other lanes scroll", async ({
		api,
		show,
		desk,
		page,
		request,
	}, testInfo) => {
		const { rig, speedLanes, audioLaneId, lanes } = await seedAudioEditor(
			api,
			show.id,
			request,
			[
				"Movement",
				"Chase",
				"Colour",
				"Stage movement",
				"Audience movement",
				"Stage chase",
				"Audience chase",
				"Colour pulse",
				"Colour sweep",
				"Accent movement",
				"Accent chase",
				"Accent colour",
			],
		);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await expandSpeedGroups(editor);
		const viewport = editor.getByLabel("Timecode timeline viewport", {
			exact: true,
		});
		const ruler = editor.locator(".timecode-timeline-heading");
		const audio = editor.locator(
			".timecode-editor-lane.lane-audio_volume.main-audio",
		);
		const waveform = audio.locator(".timecode-waveform");
		const label = audio.locator(".timecode-editor-lane-label");
		const volume = audio.locator(".timecode-timeline-item.item-volume");
		const ordinary = editor.locator(
			`.timecode-editor-lane[data-lane-id="${speedLanes[0].id}"]`,
		);
		const speed = ordinary.locator(".timecode-timeline-item.item-speed");
		await expect(waveform.locator("path")).toHaveAttribute("d", /.+/);
		await expect(
			editor.locator(".timecode-editor-lane").first(),
		).toHaveAttribute("data-lane-id", audioLaneId);
		await expect(label).toContainText("Show Opener.wav");
		const readVolume = async () => {
			const saved = await api.request<{
				objects: Array<{ definition: TimecodeDefinition }>;
			}>("GET", "/api/v2/timecodes", undefined, true, undefined, {
				showId: show.id,
			});
			const content = saved.objects
				.find((entry) => entry.definition.id === rig.definition.id)!
				.definition.lanes.find((lane) => lane.id === audioLaneId)!.content;
			if (content.kind !== "audio_volume")
				throw new Error("Expected saved audio volume lane");
			return content.keyframes[0];
		};
		for (const [touch, value] of [
			[false, 0.5],
			[true, 0.8],
		] as const) {
			await dragAudioVolumeValue(
				page,
				audio,
				volume,
				(await readVolume()).value,
				value,
				touch,
			);
			await expect
				.poll(async () => Math.abs((await readVolume()).value - value))
				.toBeLessThan(0.005);
			expect((await readVolume()).frame).toBe(220);
		}
		await page.reload();
		await openEditor(page);
		await expandSpeedGroups(editor);
		await expect(volume).toHaveAttribute("title", /80%/);
		await ordinary.locator(".timecode-lane-select").click();
		await zoomIn(page, editor);
		const point = (locator: Locator) =>
			locator.evaluate((node) => {
				const box = node.getBoundingClientRect();
				return { x: box.x, y: box.y, width: box.width, height: box.height };
			});
		const initial = {
			waveform: await point(waveform),
			label: await point(label),
			volume: await point(volume),
			ruler: await point(ruler),
			ordinary: await point(ordinary),
			speed: await point(speed),
		};
		expect(initial.waveform.y).toBeGreaterThanOrEqual(
			initial.ruler.y + initial.ruler.height,
		);
		expect(initial.volume.x + initial.volume.width / 2).toBeCloseTo(
			initial.speed.x + initial.speed.width / 2,
			1,
		);
		const bounds = await viewport.boundingBox();
		if (!bounds) throw new Error("Missing timeline viewport");
		await page.mouse.move(
			bounds.x + bounds.width / 2,
			bounds.y + bounds.height - 30,
		);
		await page.mouse.wheel(0, 360);
		await expect
			.poll(() => viewport.evaluate((node) => node.scrollTop))
			.toBeGreaterThan(100);
		for (const [locator, previous] of [
			[waveform, initial.waveform],
			[label, initial.label],
			[volume, initial.volume],
			[ruler, initial.ruler],
		] as const)
			expect((await point(locator)).y).toBe(previous.y);
		expect((await point(ordinary)).y).toBeLessThan(initial.ordinary.y - 100);
		const beforePan = {
			waveform: await point(waveform),
			volume: await point(volume),
			speed: await point(speed),
			scroll: await viewport.evaluate((node) => node.scrollLeft),
		};
		await page.mouse.wheel(320, 0);
		await expect
			.poll(() => viewport.evaluate((node) => node.scrollLeft))
			.toBeGreaterThan(beforePan.scroll);
		const delta =
			(await viewport.evaluate((node) => node.scrollLeft)) - beforePan.scroll;
		for (const [locator, previous] of [
			[waveform, beforePan.waveform],
			[volume, beforePan.volume],
			[speed, beforePan.speed],
		] as const) {
			const moved = await point(locator);
			expect(moved.x).toBeCloseTo(previous.x - delta, 1);
			expect(moved.width).toBe(previous.width);
		}
		const collection = await api.request<{
			objects: Array<{ definition: TimecodeDefinition }>;
		}>("GET", "/api/v2/timecodes", undefined, true, undefined, {
			showId: show.id,
		});
		expect(
			collection.objects
				.find((entry) => entry.definition.id === rig.definition.id)!
				.definition.lanes.map((lane) => lane.id),
		).toEqual(lanes.map((lane) => lane.id));
		await page.screenshot({
			path: testInfo.outputPath("timecode-sticky-main-audio.png"),
		});
	});

	test("TIMECODE-001 @ui › grouped Speed navigation previews every group and enlarges only the selected lane", async ({
		api,
		show,
		desk,
		page,
		request,
	}, testInfo) => {
		const names = ["Movement", "Chase", "Colour"];
		const { rig, speedLanes, audioLaneId, lanes } = await seedAudioEditor(
			api,
			show.id,
			request,
			names,
		);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		const viewport = editor.getByLabel("Timecode timeline viewport", {
			exact: true,
		});
		const audio = editor.locator(".timecode-editor-lane.main-audio");
		const section = editor.locator(".timecode-speed-section");
		const toggle = section.getByRole("button", {
			name: "Speed Groups",
			exact: true,
		});
		const summary = section.locator(".timecode-speed-summary");
		const cueLane = editor.locator(
			`.timecode-editor-lane[data-lane-id="${rig.definition.lanes[0].id}"]`,
		);
		const speedLane = (index: number) =>
			editor.locator(
				`.timecode-editor-lane[data-lane-id="${speedLanes[index].id}"]`,
			);
		await expect(
			editor.locator(".timecode-editor-lane").first(),
		).toHaveAttribute("data-lane-id", audioLaneId);
		await expect(toggle).toHaveAttribute("aria-expanded", "true");
		await expect(toggle).toContainText("Collapse");
		await toggle.tap();
		await expect(toggle).toHaveAttribute("aria-expanded", "false");
		await expect(toggle).toContainText("Expand");
		await expect(
			section.getByRole("button", { name: "Expand Speed Groups", exact: true }),
		).toBeVisible();
		await expect(summary).toHaveAttribute("aria-description", names.join(", "));
		await expect(summary).toBeVisible();
		for (const [index, name] of names.entries()) {
			const preview = summary.locator(
				`[data-lane-id="${speedLanes[index].id}"]`,
			);
			await expect(preview).toBeVisible();
			await expect(preview).toHaveAttribute("data-lane-name", name);
		}
		const bottom = (locator: Locator) =>
			locator.evaluate((node) => node.getBoundingClientRect().bottom);
		const top = (locator: Locator) =>
			locator.evaluate((node) => node.getBoundingClientRect().top);
		expect(await top(section)).toBeGreaterThanOrEqual(await bottom(audio));
		expect(await top(cueLane)).toBeGreaterThanOrEqual(await bottom(section));
		await expect(
			editor.getByRole("button", { name: "Cuelists", exact: true }),
		).toHaveCount(0);
		await expect(
			editor.getByRole("button", { name: "Expand Cuelists", exact: true }),
		).toHaveCount(0);
		await expect(editor.locator(".lane-cue_list")).toHaveCount(2);
		await expect(
			editor.locator(".lane-cue_list").nth(0).locator(".timecode-lane-select"),
		).toContainText("Stage Looks");
		await expect(
			editor.locator(".lane-cue_list").nth(1).locator(".timecode-lane-select"),
		).toContainText("Audience Looks");
		const speedRowHeight = await section
			.locator(".timecode-section-row.collapsed")
			.evaluate((node) => node.getBoundingClientRect().height);
		expect(speedRowHeight).toBe(44);
		for (const row of await editor.locator(".lane-cue_list").all()) {
			expect(
				await row.evaluate((node) => node.getBoundingClientRect().height),
			).toBe(56);
			for (const cue of await row.locator(".timecode-cue-name").all())
				expect(
					await cue.evaluate((node) => node.getBoundingClientRect().height),
				).toBe(44);
		}
		await expect(editor.locator(".timecode-cue-envelope")).toHaveCount(0);
		await page.screenshot({
			path: testInfo.outputPath("timecode-grouped-overview.png"),
		});
		await page
			.locator(".timecode-window.timecode-editor")
			.screenshot({
				path: testInfo.outputPath("timecode-grouped-overview-detail.png"),
			});
		await toggle.tap();
		await expect(toggle).toHaveAttribute("aria-expanded", "true");
		await expect(summary).toBeHidden();
		const compactHeight = await speedLane(1).evaluate(
			(node) => node.getBoundingClientRect().height,
		);
		await toggle.tap();
		await expect(toggle).toHaveAttribute("aria-expanded", "false");
		// The combined miniature is one full-row touch target; choose a lane after expanding it.
		await section
			.getByRole("button", { name: "Expand Speed Groups", exact: true })
			.tap();
		await expect(toggle).toHaveAttribute("aria-expanded", "true");
		await expect(toggle).toContainText("Collapse");
		await speedLane(0).locator(".timecode-lane-select").tap();
		await expect(speedLane(0)).toHaveClass(/lane-expanded/);
		await expect(editor.locator(".lane-speed_group.lane-expanded")).toHaveCount(
			1,
		);
		expect(
			await speedLane(0).evaluate(
				(node) => node.getBoundingClientRect().height,
			),
		).toBeGreaterThan(compactHeight);
		await speedLane(1).locator(".timecode-lane-select").tap();
		await expect(speedLane(1)).toHaveClass(/lane-expanded/);
		await expect(speedLane(0)).not.toHaveClass(/lane-expanded/);
		await expect(editor.locator(".lane-speed_group.lane-expanded")).toHaveCount(
			1,
		);
		await zoomIn(page, editor);
		const point = speedLane(1).locator(".timecode-timeline-item.item-speed");
		const before = await geometry(editor, viewport, point);
		await point.click();
		await expect(editor.locator(".timecode-editor-inspector")).toBeVisible();
		expect(await geometry(editor, viewport, point)).toEqual(before);
		await page.screenshot({
			path: testInfo.outputPath("timecode-speed-group-editor.png"),
		});
		await page
			.locator(".timecode-window.timecode-editor")
			.screenshot({
				path: testInfo.outputPath("timecode-speed-group-editor-detail.png"),
			});
		await editor
			.getByRole("button", { name: "Close selection settings", exact: true })
			.click();
		expect(await geometry(editor, viewport, point)).toEqual(before);
		await speedLane(1)
			.getByRole("button", { name: "Close speed group editor", exact: true })
			.tap();
		await expect(editor.locator(".lane-speed_group.lane-expanded")).toHaveCount(
			0,
		);
		await toggle.tap();
		await expect(toggle).toHaveAttribute("aria-expanded", "false");
		await expect(summary).toBeVisible();
		for (const [index, name] of names.entries())
			await expect(
				summary.locator(`[data-lane-id="${speedLanes[index].id}"]`),
			).toHaveAttribute("data-lane-name", name);
		const collection = await api.request<{
			objects: Array<{ definition: TimecodeDefinition }>;
		}>("GET", "/api/v2/timecodes", undefined, true, undefined, {
			showId: show.id,
		});
		expect(
			collection.objects
				.find((entry) => entry.definition.id === rig.definition.id)!
				.definition.lanes.map((lane) => lane.id),
		).toEqual(lanes.map((lane) => lane.id));
	});

	test("TIMECODE-001 @ui › coincident In end and Out start remain independently editable by mouse and touch", async ({
		api,
		show,
		desk,
		page,
	}, testInfo) => {
		// Follow 0 with zero Out delay puts the handover exactly at the incoming fade's end.
		const rig = await seedEditor(api, show.id, true);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		await editor
			.getByRole("button", { name: "Edit Cue 1 House out", exact: true })
			.tap();
		await assertVisibleCueCorners(editor, 1);
		await page
			.locator(".timecode-window.timecode-editor")
			.screenshot({
				path: testInfo.outputPath("timecode-true-corners-coincident.png"),
			});
		const incoming = editor.getByRole("slider", {
			name: "Cue 1 In fade",
			exact: true,
		});
		const outgoing = editor.getByRole("slider", {
			name: "Cue 1 Out delay",
			exact: true,
		});
		const readCue = async () =>
			(await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body
				.cues[0];
		await expect(incoming).toHaveAttribute("aria-valuenow", "132");
		await expect(outgoing).toHaveAttribute("aria-valuenow", "132");
		await dragFrames(page, editor, incoming, 4);
		await expect
			.poll(async () => (await readCue()).fade_millis)
			.toBeGreaterThan(1000);
		expect((await readCue()).out_delay_millis).toBe(0);
		const inFadeAfterMouse = (await readCue()).fade_millis;
		await dragFrames(page, editor, outgoing, 4);
		await expect
			.poll(async () => (await readCue()).out_delay_millis)
			.toBeGreaterThan(0);
		expect((await readCue()).fade_millis).toBe(inFadeAfterMouse);

		const inEndFrame = Number(await incoming.getAttribute("aria-valuenow"));
		const exactOutStart = editor
			.locator(".timecode-cue-editor-details")
			.getByLabel("Out start", { exact: true });
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
		await expect
			.poll(async () => (await readCue()).out_delay_millis)
			.toBeGreaterThan(0);
		expect((await readCue()).fade_millis).toBe(1000);
	});

	test("TIMECODE-001 @ui › a short single-Cue clip with inherited fade can be repeatedly shortened and repaired through its End grip", async ({
		api,
		show,
		desk,
		page,
	}) => {
		const rig = await seedEditor(api, show.id);
		const record = (await api.showObject<CueList>(
			show.id,
			"cue_list",
			rig.list.id,
		))!;
		const cues = structuredClone(record.body);
		Object.assign(cues.cues[0], {
			fade_millis: 0,
			delay_millis: 0,
			out_delay_millis: 0,
			out_fade_millis: 0,
		});
		await api.seedShowObject(
			show.id,
			"cue_list",
			rig.list.id,
			cues,
			record.revision,
		);
		const lanes = structuredClone(rig.definition.lanes);
		if (lanes[0].content.kind !== "cue_list")
			throw new Error("Expected Cuelist lane");
		Object.assign(lanes[0].content.clips[0], {
			end_frame: 48,
			end_cue_id: lanes[0].content.clips[0].start_cue_id,
			cue_starts: [],
		});
		const collection = await api.request<{
			objects: Array<{ revision: number; definition: TimecodeDefinition }>;
		}>("GET", "/api/v2/timecodes", undefined, true, undefined, {
			showId: show.id,
		});
		await api.request(
			"POST",
			"/api/v2/timecodes/actions",
			{
				request_id: crypto.randomUUID(),
				action: {
					type: "update",
					timecode_id: rig.definition.id,
					expected_revision: collection.objects.find(
						(entry) => entry.definition.id === rig.definition.id,
					)!.revision,
					patch: { lanes },
				},
			},
			true,
			undefined,
			{ showId: show.id },
		);
		const baselineCues = (await api.showObject<CueList>(
			show.id,
			"cue_list",
			rig.list.id,
		))!.body.cues;
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		await editor
			.getByRole("button", { name: "Edit Cue 1 House out", exact: true })
			.click({ position: { x: 2, y: 22 } });
		const fade = editor.getByRole("slider", {
			name: "Cue 1 In fade",
			exact: true,
		});
		const fadeEnd = Number(await fade.getAttribute("aria-valuenow"));
		expect(fadeEnd).toBeGreaterThan(92);
		const end = editor.getByRole("slider", {
			name: "Extend last Cue",
			exact: true,
		});
		await expect(end).not.toHaveAttribute("aria-disabled", "true");
		let previous = 48;
		for (const [target, touch] of [
			[52, false],
			[56, true],
			[48, false],
			[92, true],
			[fadeEnd + 4, false],
			[48, true],
			[92, false],
			[fadeEnd + 4, true],
		] as const) {
			if (touch) await touchDragFrames(page, editor, end, target - previous);
			else await dragFrames(page, editor, end, target - previous);
			await expect
				.poll(
					async () =>
						(await readClip(api, show.id, rig.definition.id)).end_frame,
				)
				.toBe(target);
			await expect(fade).toHaveAttribute("aria-valuenow", String(fadeEnd));
			const clip = await readClip(api, show.id, rig.definition.id);
			expect(clip.start_frame).toBe(44);
			expect(clip.cue_starts).toEqual([]);
			expect(clip.end_cue_id).toBe(clip.start_cue_id);
			expect(
				(await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body
					.cues,
			).toEqual(baselineCues);
			previous = target;
		}
		await page.reload();
		await openEditor(page);
		await editor
			.getByRole("button", { name: "Edit Cue 1 House out", exact: true })
			.click({ position: { x: 32, y: 18 } });
		await expect(end).toHaveAttribute("aria-valuenow", String(fadeEnd + 4));
		await expect(fade).toHaveAttribute("aria-valuenow", String(fadeEnd));
		expect(
			(await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body
				.cues,
		).toEqual(baselineCues);
	});

	test("TIMECODE-001 @ui › a four-frame clip keeps its true width and Trigger alignment when moved", async ({
		api,
		show,
		desk,
		page,
	}) => {
		// A real nonzero 1ms In fade rounds to zero frames, avoiding zero-In inheritance.
		const rig = await seedEditor(api, show.id, false, true);
		const lanes = structuredClone(rig.definition.lanes);
		if (lanes[0].content.kind !== "cue_list")
			throw new Error("Expected Cuelist lane");
		const clip = lanes[0].content.clips[0];
		clip.end_frame = 48;
		clip.end_cue_id = clip.start_cue_id;
		clip.cue_starts = [];
		const current = await api.request<{
			objects: Array<{ revision: number; definition: TimecodeDefinition }>;
		}>("GET", "/api/v2/timecodes", undefined, true, undefined, {
			showId: show.id,
		});
		await api.request(
			"POST",
			"/api/v2/timecodes/actions",
			{
				request_id: crypto.randomUUID(),
				action: {
					type: "update",
					timecode_id: rig.definition.id,
					expected_revision: current.objects.find(
						(entry) => entry.definition.id === rig.definition.id,
					)!.revision,
					patch: { lanes },
				},
			},
			true,
			undefined,
			{ showId: show.id },
		);
		await desk.open(api.baseUrl);
		await openEditor(page);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		await zoomIn(page, editor);
		const scale = Number(
			await editor
				.locator(".timecode-timeline-canvas")
				.getAttribute("data-pixels-per-frame"),
		);
		const outer = editor.locator(".timecode-timeline-item.item-clip");
		const cue = editor.getByRole("button", {
			name: "Edit Cue 1 House out",
			exact: true,
		});
		for (const target of [outer, cue]) {
			const width = await target.evaluate(
				(node) => node.getBoundingClientRect().width,
			);
			expect(width).toBeLessThan(44);
			expect(Math.abs(width - 4 * scale)).toBeLessThanOrEqual(1);
		}
		await assertCueFrameAlignment(editor, 1, "House out", 44, false);
		await cue.click({ position: { x: 2, y: 22 } });
		await assertCueFrameAlignment(editor, 1, "House out", 44, true);
		await expect(
			editor.getByRole("slider", { name: "Cue 1 In fade", exact: true }),
		).toHaveAttribute("aria-valuenow", "44");
		const trigger = editor.getByRole("slider", {
			name: "Move clip start",
			exact: true,
		});
		await touchDragFrames(page, editor, trigger, 4, true);
		await assertCueFrameAlignment(editor, 1, "House out", 44, true);
		expect((await readClip(api, show.id, rig.definition.id)).start_frame).toBe(
			44,
		);
		await dragFrames(page, editor, trigger, 4);
		await expect
			.poll(
				async () =>
					(await readClip(api, show.id, rig.definition.id)).start_frame,
			)
			.toBe(48);
		expect((await readClip(api, show.id, rig.definition.id)).end_frame).toBe(
			52,
		);
		await assertCueFrameAlignment(editor, 1, "House out", 48, true);
		expect(
			(await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!.body
				.cues[0],
		).toMatchObject({
			fade_millis: 1,
			delay_millis: 0,
			out_delay_millis: 0,
			out_fade_millis: 0,
		});
	});

	test("TIMECODE-001 @ui › overview positions and viewport bounds match the main time axis through resize, sidebar, zoom and pan", async ({ api, show, desk, page, request }, testInfo) => {
		await page.setViewportSize({ width: 1280, height: 1000 });
		const { rig, speedLanes } = await seedAudioEditor(api, show.id, request, ["Movement"]);
		await patchTimecode(api, show.id, rig.definition.id, { markers: [{ id: crypto.randomUUID(), frame: 440, name: "Chorus", color: "#ffffff" }] });
		await desk.open(api.baseUrl);
		await openEditor(page, false);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		const viewport = editor.getByLabel("Timecode timeline viewport", { exact: true });
		const canvas = editor.locator(".timecode-timeline-canvas");
		const scale = Number(await canvas.getAttribute("data-pixels-per-frame"));
		await page.setViewportSize({ width: 2400, height: 1000 });
		await expect.poll(() => viewport.evaluate(node => node.clientWidth)).toBeGreaterThan(2000);
		expect(Number(await canvas.getAttribute("data-pixels-per-frame"))).toBe(scale);
		await expect.poll(() => canvas.evaluate(node => 880 * Number((node as HTMLElement).dataset.pixelsPerFrame) / (node.getBoundingClientRect().width - 160))).toBeLessThan(0.6);
		await assertOverviewMainAxis(editor);
		await page.locator(".timecode-window.timecode-editor").screenshot({ path: testInfo.outputPath("timecode-overview-wide-detail.png") });
		await setSelectionSidebar(page, true);
		await editor.locator(`[data-lane-id="${speedLanes[0].id}"].timecode-editor-lane .timecode-lane-select`).click();
		await expect(editor.locator(".timecode-editor-inspector")).toBeVisible();
		await assertOverviewMainAxis(editor);
		const overview = editor.getByRole("scrollbar", { name: "Timeline overview", exact: true });
		const track = (await overview.boundingBox())!;
		const end = editor.getByRole("separator", { name: "Resize timeline overview from end", exact: true });
		const handle = (await end.boundingBox())!;
		const initialDisplayedFrames = await canvas.evaluate(node => (node.getBoundingClientRect().width - 160) / Number((node as HTMLElement).dataset.pixelsPerFrame));
		await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2);
		await page.mouse.down();
		await page.mouse.move(track.x + track.width * 0.35, handle.y + handle.height / 2, { steps: 8 });
		await page.mouse.up();
		// The pointer requests 35% of the captured frame domain. The overview domain
		// may contract as zoom fills previous blank viewport space; its new thumb
		// percentage need not remain 35%. Verify the actual visible frame interval.
		await expect.poll(() => viewport.evaluate((node, frames) => {
			const currentScale = Number((node.querySelector(".timecode-timeline-canvas") as HTMLElement).dataset.pixelsPerFrame);
			return Math.abs((node.clientWidth - 160) - frames * 0.35 * currentScale);
		}, initialDisplayedFrames)).toBeLessThanOrEqual(2);
		await assertOverviewMainAxis(editor);
		// The captured wide domain contains blank viewport space. It must not
		// fabricate a scrollable tail when the actual show still fits on screen.
		await overview.press("ArrowRight");
		expect(await viewport.evaluate(node => node.scrollLeft)).toBe(0);
		// Narrow further so the actual show extends beyond the scrollport.
		await zoomIn(page, editor);
		await assertOverviewMainAxis(editor);
		await overview.press("ArrowRight");
		await expect.poll(() => viewport.evaluate(node => node.scrollLeft)).toBeGreaterThan(0);
		await assertOverviewMainAxis(editor);
		const pannedLeft = await viewport.evaluate(node => node.scrollLeft);
		const bounds = (await viewport.boundingBox())!;
		await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + 100);
		await page.mouse.wheel(100_000, 0);
		await expect.poll(() => viewport.evaluate(node => node.scrollLeft)).toBeGreaterThan(pannedLeft);
		await expect.poll(async () => {
			const previous = await viewport.evaluate(node => node.scrollLeft);
			await page.waitForTimeout(100);
			return viewport.evaluate((node, left) => node.scrollLeft === left &&
				node.querySelector(".timecode-timeline-canvas")!.getBoundingClientRect().right <=
				node.getBoundingClientRect().left + node.clientLeft + node.clientWidth + 1, previous);
		}).toBe(true);
		await assertOverviewMainAxis(editor);
		const left = await viewport.evaluate(node => node.scrollLeft);
		const zoomedScale = Number(await canvas.getAttribute("data-pixels-per-frame"));
		expect(zoomedScale).toBeGreaterThan(scale);
		await editor.getByRole("button", { name: "Close selection settings", exact: true }).click();
		expect(await viewport.evaluate(node => node.scrollLeft)).toBe(left);
		expect(Number(await canvas.getAttribute("data-pixels-per-frame"))).toBe(zoomedScale);
		await assertOverviewMainAxis(editor);
		await page.locator(".timecode-window.timecode-editor").screenshot({ path: testInfo.outputPath("timecode-overview-axis-detail.png") });
		// Retained sidebar space must not grow with zoom and make the real end
		// unreachable through the normal physical end-of-scroll gesture.
		const maximumTrack = (await overview.boundingBox())!;
		const maximumHandle = (await end.boundingBox())!;
		await page.mouse.move(maximumHandle.x + maximumHandle.width / 2, maximumHandle.y + maximumHandle.height / 2);
		await page.mouse.down();
		await page.mouse.move(maximumTrack.x + 1, maximumHandle.y + maximumHandle.height / 2, { steps: 8 });
		await page.mouse.up();
		await expect.poll(async () => Number(await canvas.getAttribute("data-pixels-per-frame"))).toBeCloseTo(17.5, 8);
		const finalBounds = (await viewport.boundingBox())!;
		await page.mouse.move(finalBounds.x + finalBounds.width / 2, finalBounds.y + 100);
		await page.mouse.wheel(100_000, 0);
		await expect(editor.locator(".timecode-ruler-final-tick")).toBeInViewport();
		await assertOverviewMainAxis(editor);
	});

	test("TIMECODE-001 @ui › pressing keyframes and Cue corners preserves their geometry and never saves a tap", async ({
		api,
		show,
		desk,
		page,
		request,
	}) => {
		const { rig, speedLanes } = await seedAudioEditor(api, show.id, request, [
			"Movement",
			"Chase",
			"Colour",
		]);
		await patchTimecode(api, show.id, rig.definition.id, {
			markers: [
				{
					id: crypto.randomUUID(),
					frame: 224,
					name: "Nearby",
					color: "#ffffff",
				},
			],
		});
		await desk.open(api.baseUrl);
		await openEditor(page, false);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true });
		const speedLane = editor.locator(
				`[data-lane-id="${speedLanes[0].id}"].timecode-editor-lane`,
			),
			speed = speedLane.locator(".item-speed"),
			volume = editor.locator(".main-audio .item-volume");
		await speedLane.locator(".timecode-lane-select").click();
		const before = (
			await api.request<{
				objects: Array<{ revision: number; definition: TimecodeDefinition }>;
			}>("GET", "/api/v2/timecodes", undefined, true, undefined, {
				showId: show.id,
			})
		).objects.find((item) => item.definition.id === rig.definition.id)!;
		const cueBefore = (await api.showObject<CueList>(
			show.id,
			"cue_list",
			rig.list.id,
		))!;
		let saves = 0;
		page.on("request", (req) => {
			if (
				req.method() === "POST" &&
				req.url().endsWith("/api/v2/timecodes/actions")
			)
				saves += 1;
		});
		for (const handle of [speed, volume]) {
			await assertNoMotionPress(page, editor, handle, false);
			await assertNoMotionPress(page, editor, handle, true);
		}
		await expect(speedLane).toHaveClass(/lane-expanded/);
		await editor
			.getByRole("button", { name: "Edit Cue 1 House out", exact: true })
			.click({ position: { x: 32, y: 18 } });
		for (const name of ["In delay", "In fade", "Out delay", "Out fade"]) {
			const handle = editor.getByRole("slider", {
				name: `Cue 1 ${name}`,
				exact: true,
			});
			await assertNoMotionPress(page, editor, handle, false);
			await assertNoMotionPress(page, editor, handle, true);
		}
		await page.waitForTimeout(350);
		expect(saves).toBe(0);
		const after = (
			await api.request<{
				objects: Array<{ revision: number; definition: TimecodeDefinition }>;
			}>("GET", "/api/v2/timecodes", undefined, true, undefined, {
				showId: show.id,
			})
		).objects.find((item) => item.definition.id === rig.definition.id)!;
		expect(after).toEqual(before);
		expect(
			(await api.showObject<CueList>(show.id, "cue_list", rig.list.id))!
				.revision,
		).toBe(cueBefore.revision);
		await editor
			.getByRole("button", { name: "Close cue editor", exact: true })
			.click();
		await speedLane.locator(".timecode-lane-select").click();
		await dragFrames(page, editor, speed, 44, undefined, 3);
		await expect
			.poll(async () => {
				const def = (
					await api.request<{
						objects: Array<{ definition: TimecodeDefinition }>;
					}>("GET", "/api/v2/timecodes", undefined, true, undefined, {
						showId: show.id,
					})
				).objects.find(
					(item) => item.definition.id === rig.definition.id,
				)!.definition;
				const lane = def.lanes.find((lane) => lane.id === speedLanes[0].id)!;
				return lane.content.kind === "speed_group"
					? lane.content.keyframes[0].frame
					: -1;
			})
			.toBe(264);
		await expect(speedLane).toHaveClass(/lane-expanded/);
		const audio = editor.locator(".main-audio");
		await dragAudioVolumeValue(page, audio, volume, 0.8, 0.5, true);
		await expect
			.poll(async () => {
				const def = (
					await api.request<{
						objects: Array<{ definition: TimecodeDefinition }>;
					}>("GET", "/api/v2/timecodes", undefined, true, undefined, {
						showId: show.id,
					})
				).objects.find(
					(item) => item.definition.id === rig.definition.id,
				)!.definition;
				const content = def.lanes.find(
					(lane) => lane.content.kind === "audio_volume",
				)!.content;
				return (
					content.kind === "audio_volume" &&
					content.keyframes[0].frame === 220 &&
					Math.abs(content.keyframes[0].value - 0.5) < 0.01
				);
			})
			.toBe(true);
	});

	test("TIMECODE-001 @ui › sidebar opt-in is independent of Cue expansion and title controls follow the selected context", async ({
		api,
		show,
		desk,
		page,
		request,
	}, testInfo) => {
		const { rig, speedLanes } = await seedAudioEditor(
			api,
			show.id,
			request,
			Array.from(
				{ length: 12 },
				(_, i) => ["Movement", "Chase", "Colour"][i % 3] + ` ${i + 1}`,
			),
		);
		await patchTimecode(api, show.id, rig.definition.id, {
			markers: [
				{
					id: crypto.randomUUID(),
					frame: 220,
					name: "Verse",
					color: "#ffffff",
				},
			],
		});
		await desk.open(api.baseUrl);
		await openEditor(page, false);
		const editor = page.getByLabel("Timecode timeline editor", { exact: true }),
			viewport = editor.getByLabel("Timecode timeline viewport", {
				exact: true,
			}),
			window = page.locator(".timecode-window.timecode-editor");
		await expect(
			window.getByRole("button", { name: "Fit timeline", exact: true }),
		).toHaveCount(0);
		const cue = editor.getByRole("button", {
			name: "Edit Cue 1 House out",
			exact: true,
		});
		await cue.scrollIntoViewIfNeeded();
		await cue.click({ position: { x: 32, y: 18 } });
		await expect(
			editor.getByRole("slider", { name: "Cue 1 In fade", exact: true }),
		).toBeVisible();
		await expect(editor.locator(".timecode-editor-inspector")).toHaveCount(0);
		const baseline = await geometry(editor, viewport, cue);
		await setSelectionSidebar(page, true);
		await expect(editor.locator(".timecode-editor-inspector")).toBeVisible();
		expect(await geometry(editor, viewport, cue)).toEqual(baseline);
		await editor
			.getByRole("button", { name: "Close selection settings", exact: true })
			.click();
		await expect(editor.locator(".timecode-editor-inspector")).toHaveCount(0);
		await expect(
			editor.getByRole("slider", { name: "Cue 1 In fade", exact: true }),
		).toBeVisible();
		expect(await geometry(editor, viewport, cue)).toEqual(baseline);
		for (const name of ["Prev Keyframe", "Next Keyframe", "Insert Keyframe"])
			await expect(
				window.getByRole("button", { name, exact: true }),
			).toHaveCount(0);
		const behaviors = editor
			.locator(".item-clip")
			.first()
			.locator(".timecode-clip-behaviors");
		await expect(
			behaviors.getByLabel("Start behavior: State Start"),
		).toBeVisible();
		await expect(behaviors.getByLabel("End behavior: Release")).toBeVisible();
		const clock = window.getByRole("button", {
				name: "Timecode position",
				exact: true,
			}),
			settings = window.getByRole("button", { name: "Settings", exact: true });
		const clockLeft = (await clock.boundingBox())!.x,
			settingsLeft = (await settings.boundingBox())!.x;
		const speed = editor.locator(
			`[data-lane-id="${speedLanes[0].id}"].timecode-editor-lane`,
		);
		await speed.scrollIntoViewIfNeeded();
		await expect(speed.locator(".timecode-lane-edit-hint")).toBeVisible();
		await speed.locator(".timecode-lane-select").click();
		for (const name of ["Prev Keyframe", "Next Keyframe", "Insert Keyframe"])
			await expect(
				window.getByRole("button", { name, exact: true }),
			).toBeVisible();
		expect((await clock.boundingBox())!.x).toBe(clockLeft);
		expect((await settings.boundingBox())!.x).toBe(settingsLeft);
		await page.setViewportSize({ width: 1280, height: 800 });
		const compactSettings = (await settings.boundingBox())!;
		const compactWindow = (await window.boundingBox())!;
		expect(compactSettings.x + compactSettings.width).toBeLessThanOrEqual(
			compactWindow.x + compactWindow.width,
		);
		const compactClockLeft = (await clock.boundingBox())!.x;
		await cue.scrollIntoViewIfNeeded();
		await cue.click({ position: { x: 32, y: 18 } });
		expect((await settings.boundingBox())!.x).toBe(compactSettings.x);
		expect((await clock.boundingBox())!.x).toBe(compactClockLeft);
		await editor.locator(".timecode-timeline-marker-label").click();
		for (const name of ["Prev Keyframe", "Next Keyframe", "Insert Keyframe"])
			await expect(
				window.getByRole("button", { name, exact: true }),
			).toHaveCount(0);
		const label = editor
			.getByRole("button", { name: "Drag playhead to seek", exact: true })
			.locator(":scope > span");
		const beforeTop = (await label.boundingBox())!.y;
		const box = (await viewport.boundingBox())!;
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.wheel(0, 250);
		await expect
			.poll(() => viewport.evaluate((node) => node.scrollTop))
			.toBeGreaterThan(0);
		expect(
			Math.abs((await label.boundingBox())!.y - beforeTop),
		).toBeLessThanOrEqual(1);
		await window.screenshot({
			path: testInfo.outputPath("timecode-local-sidebar-title-detail.png"),
		});
	});
});

async function seedEditor(
	api: ApiDriver,
	showId: string,
	coincident = false,
	nearZero = false,
) {
	const list: CueList = {
		id: crypto.randomUUID(),
		name: "Stage Looks",
		mode: "sequence",
		priority: 0,
		looped: false,
		cues: ["House out", "Verse wash", "Chorus"].map((name, index) => ({
			id: crypto.randomUUID(),
			number: String(index + 1),
			name,
			fade_millis: nearZero && index === 0 ? 1 : 1000,
			delay_millis: nearZero && index === 0 ? 0 : 1000,
			out_delay_millis: (coincident || nearZero) && index === 0 ? 0 : 500,
			out_fade_millis: nearZero && index === 0 ? 0 : 1000,
			trigger:
				coincident && index === 1
					? { type: "follow", delay_millis: 0 }
					: { type: "manual" as const },
			changes: [],
		})),
	};
	await api.seedShowObject(showId, "cue_list", list.id, list);
	const definition: TimecodeDefinition = {
		id: crypto.randomUUID(),
		number: 86,
		name: "Cue editor acceptance",
		duration_frame: 880,
		transport_offset_frame: 0,
		auto_start: false,
		audio: null,
		markers: [],
		lanes: [
			{
				id: crypto.randomUUID(),
				name: "Stage Looks",
				content: {
					kind: "cue_list",
					cue_list_id: list.id,
					clips: [
						{
							id: crypto.randomUUID(),
							start_frame: 44,
							end_frame: 616,
							start_cue_id: list.cues[0].id!,
							end_cue_id: list.cues[2].id!,
							start_behavior: "state",
							end_behavior: "release",
							in_fade_frames: 0,
							out_fade_frames: 0,
							cue_starts: [
								...(coincident
									? []
									: [{ cue_id: list.cues[1].id!, offset_frame: 176 }]),
								{ cue_id: list.cues[2].id!, offset_frame: 352 },
							],
						},
					],
				},
			},
		],
	};
	await api.request(
		"POST",
		"/api/v2/timecodes/actions",
		{ request_id: crypto.randomUUID(), action: { type: "create", definition } },
		true,
		undefined,
		{ showId },
	);
	return { list, definition };
}

async function openEditor(page: Page, sidebar = true) {
	await expect(page.locator(".connection-cover")).toBeHidden();
	const pool = page.getByRole("button", {
		name: "Timecode 86 Cue editor acceptance",
		exact: true,
	});
	if (!(await pool.isVisible().catch(() => false))) {
		const toggle = page.getByRole("button", {
			name: "Desktops / Built-ins",
			exact: true,
		});
		if ((await toggle.getAttribute("data-dock-mode")) !== "builtins")
			await toggle.click();
		const shift = page.locator('[data-keypad-key="SHIFT"]:visible').first();
		if (!(await shift.isVisible().catch(() => false)))
			await page.locator(".mode-toggle").click();
		await shift.click();
		await page
			.locator("[aria-label='Shift Built-ins']")
			.getByRole("button", { name: "Timecode", exact: true })
			.click();
	}
	await pool.click({ button: "right" });
	await expect(
		page.getByLabel("Timecode timeline editor", { exact: true }),
	).toBeVisible();
	if (sidebar) await setSelectionSidebar(page, true);
}

async function zoomIn(page: Page, editor: Locator) {
	const overview = editor.getByRole("scrollbar", { name: "Timeline overview" });
	const handle = overview.getByRole("separator", {
		name: "Resize timeline overview from end",
	});
	const box = await handle.boundingBox();
	const track = await overview.boundingBox();
	if (!box || !track) throw new Error("Missing overview controls");
	await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	await page.mouse.down();
	// Explicit 4x leaves enough space for the one-second ruler labels used below.
	await page.mouse.move(track.x + track.width / 4, box.y + box.height / 2, {
		steps: 8,
	});
	await page.mouse.up();
}

async function dragFrames(
	page: Page,
	editor: Locator,
	handle: Locator,
	frames: number,
	onPreview?: () => Promise<void>,
	grabOffset = 0,
) {
	await handle.scrollIntoViewIfNeeded();
	if (grabOffset === 0) grabOffset = await visibleHalfBarOffset(handle);
	const box = await handle.boundingBox();
	const scale = Number(
		await editor
			.locator(".timecode-timeline-canvas")
			.getAttribute("data-pixels-per-frame"),
	);
	if (!box || !(scale > 0))
		throw new Error("Missing timing point or timeline scale");
	await page.mouse.move(
		box.x + box.width / 2 + grabOffset,
		box.y + box.height / 2,
	);
	await page.mouse.down();
	await page.mouse.move(
		box.x + box.width / 2 + grabOffset + frames * scale,
		box.y + box.height / 2,
		{ steps: 10 },
	);
	await onPreview?.();
	await page.mouse.up();
}

async function geometry(editor: Locator, viewport: Locator, envelope: Locator) {
	return {
		scale: await editor
			.locator(".timecode-timeline-canvas")
			.getAttribute("data-pixels-per-frame"),
		scroll: await viewport.evaluate((node) => node.scrollLeft),
		cue: await envelope.evaluate((node) => ({
			left: node.getBoundingClientRect().left,
			width: node.getBoundingClientRect().width,
		})),
	};
}

async function readClip(api: ApiDriver, showId: string, timecodeId: string) {
	const collection = await api.request<{
		objects: Array<{ definition: TimecodeDefinition }>;
	}>("GET", "/api/v2/timecodes", undefined, true, undefined, { showId });
	const lane = collection.objects.find(
		(object) => object.definition.id === timecodeId,
	)!.definition.lanes[0];
	if (lane.content.kind !== "cue_list") throw new Error("Missing Cuelist lane");
	return lane.content.clips[0];
}

async function touchDragFrames(
	page: Page,
	editor: Locator,
	handle: Locator,
	frames: number,
	cancel = false,
	grabOffset = 0,
) {
	await handle.scrollIntoViewIfNeeded();
	if (grabOffset === 0) grabOffset = await visibleHalfBarOffset(handle);
	const box = await handle.boundingBox();
	const scale = Number(
		await editor
			.locator(".timecode-timeline-canvas")
			.getAttribute("data-pixels-per-frame"),
	);
	if (!box || !(scale > 0)) throw new Error("Missing touch timing point");
	const initial = await handle.getAttribute("aria-valuenow");
	const cdp = await page.context().newCDPSession(page);
	const touch = (x: number) => [
		{ x, y: box.y + box.height / 2, id: 1, radiusX: 2, radiusY: 2, force: 1 },
	];
	const x = box.x + box.width / 2 + grabOffset;
	try {
		await cdp.send("Input.dispatchTouchEvent", {
			type: "touchStart",
			touchPoints: touch(x),
		});
		for (let step = 1; step <= 8; step += 1)
			await cdp.send("Input.dispatchTouchEvent", {
				type: "touchMove",
				touchPoints: touch(x + (frames * scale * step) / 8),
			});
		if (cancel)
			await expect(handle).not.toHaveAttribute("aria-valuenow", initial!);
		await cdp.send("Input.dispatchTouchEvent", {
			type: cancel ? "touchCancel" : "touchEnd",
			touchPoints: [],
		});
	} finally {
		await cdp.detach();
	}
}

function referenceWav() {
	const sampleRate = 8000;
	const frames = sampleRate * 20;
	const buffer = Buffer.alloc(44 + frames * 2);
	buffer.write("RIFF", 0);
	buffer.writeUInt32LE(buffer.length - 8, 4);
	buffer.write("WAVEfmt ", 8);
	buffer.writeUInt32LE(16, 16);
	buffer.writeUInt16LE(1, 20);
	buffer.writeUInt16LE(1, 22);
	buffer.writeUInt32LE(sampleRate, 24);
	buffer.writeUInt32LE(sampleRate * 2, 28);
	buffer.writeUInt16LE(2, 32);
	buffer.writeUInt16LE(16, 34);
	buffer.write("data", 36);
	buffer.writeUInt32LE(frames * 2, 40);
	for (let index = 0; index < frames; index += 1) {
		const envelope =
			0.3 + 0.6 * Math.abs(Math.sin((index / sampleRate) * Math.PI));
		buffer.writeInt16LE(
			Math.round(
				Math.sin((index / sampleRate) * 2 * Math.PI * 440) * envelope * 24000,
			),
			44 + index * 2,
		);
	}
	return buffer;
}

async function seedAudioEditor(
	api: ApiDriver,
	showId: string,
	request: APIRequestContext,
	names: string[],
) {
	const rig = await seedEditor(api, showId);
	const imported = await request.post(
		`${api.baseUrl}/api/v2/timecodes/audio/import?name=Show%20Opener.wav`,
		{
			headers: {
				authorization: `Bearer ${api.session!.token}`,
				"x-tosk-show": showId,
				"content-type": "audio/wav",
			},
			data: referenceWav(),
		},
	);
	expect(imported.ok()).toBe(true);
	const asset = (await imported.json()) as {
		asset_id: string;
		asset_revision: number;
	};
	const audience: CueList = {
		...rig.list,
		id: crypto.randomUUID(),
		name: "Audience Looks",
		cues: rig.list.cues.map((cue, index) => ({
			...cue,
			id: crypto.randomUUID(),
			name: ["Warm audience", "Audience sweep", "Audience pulse"][index],
		})),
	};
	await api.seedShowObject(showId, "cue_list", audience.id, audience);
	const audienceLane: TimecodeDefinition["lanes"][number] = {
		id: crypto.randomUUID(),
		name: "Audience Looks",
		content: {
			kind: "cue_list",
			cue_list_id: audience.id,
			clips: [
				{
					id: crypto.randomUUID(),
					start_frame: 44,
					end_frame: 616,
					start_cue_id: audience.cues[0].id!,
					end_cue_id: audience.cues[2].id!,
					start_behavior: "state",
					end_behavior: "release",
					in_fade_frames: 0,
					out_fade_frames: 0,
					cue_starts: [
						{ cue_id: audience.cues[1].id!, offset_frame: 176 },
						{ cue_id: audience.cues[2].id!, offset_frame: 352 },
					],
				},
			],
		},
	};
	const speedLanes: TimecodeDefinition["lanes"] = names.map((name, index) => ({
		id: crypto.randomUUID(),
		name,
		content: {
			kind: "speed_group",
			group: String.fromCharCode(65 + index),
			keyframes: [
				{
					id: crypto.randomUUID(),
					frame: 220,
					bpm: [96, 128, 72][index % 3],
					phase: 0,
				},
			],
		},
	}));
	const audioLaneId = crypto.randomUUID();
	const lanes: TimecodeDefinition["lanes"] = [
		...rig.definition.lanes,
		audienceLane,
		...speedLanes,
		{
			id: audioLaneId,
			name: "Main audio",
			content: {
				kind: "audio_volume",
				keyframes: [
					{
						id: crypto.randomUUID(),
						frame: 220,
						value: 0.8,
						fade_frames: 0,
						curve: "linear",
					},
				],
			},
		},
	];
	const current = await api.request<{
		objects: Array<{ revision: number; definition: TimecodeDefinition }>;
	}>("GET", "/api/v2/timecodes", undefined, true, undefined, { showId });
	await api.request(
		"POST",
		"/api/v2/timecodes/actions",
		{
			request_id: crypto.randomUUID(),
			action: {
				type: "update",
				timecode_id: rig.definition.id,
				expected_revision: current.objects.find(
					(entry) => entry.definition.id === rig.definition.id,
				)!.revision,
				patch: {
					audio: {
						asset_id: asset.asset_id,
						asset_revision: asset.asset_revision,
						file_name: "Show Opener.wav",
					},
					lanes,
				},
			},
		},
		true,
		undefined,
		{ showId },
	);
	return { rig, speedLanes, audioLaneId, lanes };
}

async function expandSpeedGroups(editor: Locator) {
	const toggle = editor.getByRole("button", {
		name: "Speed Groups",
		exact: true,
	});
	if ((await toggle.getAttribute("aria-expanded")) === "false")
		await toggle.click();
}

function frameText(frame: number) {
	const totalSeconds = Math.floor(frame / 44);
	const pad = (value: number) => String(value).padStart(2, "0");
	return `${pad(Math.floor(totalSeconds / 3600))}:${pad(Math.floor(totalSeconds / 60) % 60)}:${pad(totalSeconds % 60)}.${pad(frame % 44)}`;
}

async function assertCueFrameAlignment(
	editor: Locator,
	number: number,
	name: string,
	frame: number,
	expanded: boolean,
) {
	const canvas = editor.locator(".timecode-timeline-canvas");
	const axis = await canvas.evaluate((node) => ({
		left: node.getBoundingClientRect().left,
		origin: Number(node.getAttribute("data-time-origin-px")),
		scale: Number(node.getAttribute("data-pixels-per-frame")),
	}));
	expect(axis.origin).toBeGreaterThan(0);
	const expectedX = axis.left + axis.origin + frame * axis.scale;
	const block = editor.getByRole("button", {
		name: `Edit Cue ${number} ${name}`,
		exact: true,
	});
	expect(
		Math.abs(
			(await block.evaluate((node) => node.getBoundingClientRect().left)) -
				expectedX,
		),
	).toBeLessThanOrEqual(1);
	if (expanded) {
		const boundary = editor.locator(
			`.timecode-cue-trigger-boundary[data-trigger-frame="${frame}"]`,
		);
		expect(
			Math.abs(
				(await boundary.evaluate((node) => node.getBoundingClientRect().left)) -
					expectedX,
			),
		).toBeLessThanOrEqual(1);
		await expect(
			editor
				.locator(".timecode-cue-editor-details")
				.getByLabel("Trigger", { exact: true }),
		).toHaveValue(frameText(frame));
	}
	if (frame % 44 === 0) {
		const rulerTick = editor
			.locator(".timecode-ruler")
			.getByText(frameText(frame), { exact: true });
		await expect(rulerTick).toBeVisible();
		expect(
			Math.abs(
				(await rulerTick.evaluate(
					(node) => node.getBoundingClientRect().left,
				)) - expectedX,
			),
		).toBeLessThanOrEqual(1);
	}
}

async function assertVisibleCueCorners(editor: Locator, number: number) {
	for (const [name, kind, index] of [
		["In delay", "in", 0],
		["In fade", "in", 1],
		["Out delay", "out", 0],
		["Out fade", "out", 1],
	] as const) {
		const corner = await editor
			.locator(`.timecode-cue-timing.selected .timecode-cue-fade-guide.${kind}`)
			.evaluate((node, index) => {
				const guide = node as SVGPolylineElement;
				const point = guide.points.getItem(index);
				const screen = new DOMPoint(point.x, point.y).matrixTransform(
					guide.getScreenCTM()!,
				);
				return { x: screen.x, y: screen.y };
			}, index);
		const handle = editor.getByRole("slider", {
			name: `Cue ${number} ${name}`,
			exact: true,
		});
		const dot = (await handle
			.locator(".timecode-cue-point-anchor")
			.boundingBox())!;
		expect(
			Math.abs(dot.x + dot.width / 2 - corner.x),
			`${name} visible dot X`,
		).toBeLessThanOrEqual(0.75);
		expect(
			Math.abs(dot.y + dot.height / 2 - corner.y),
			`${name} visible dot Y`,
		).toBeLessThanOrEqual(0.75);
		await expect(handle).toHaveCSS("width", "44px");
		await expect(handle).toHaveCSS("height", "44px");
	}
}

async function dragAudioVolumeValue(
	page: Page,
	audio: Locator,
	handle: Locator,
	from: number,
	to: number,
	touch: boolean,
) {
	const box = (await handle.boundingBox())!;
	const x = box.x + box.width / 2;
	const y = box.y + box.height / 2;
	const travel = await audio.evaluate((node) => node.clientHeight * 0.76);
	const target = y + (from - to) * travel;
	if (touch) {
		const cdp = await page.context().newCDPSession(page);
		const points = (y: number) => [
			{ x, y, id: 1, radiusX: 2, radiusY: 2, force: 1 },
		];
		try {
			await cdp.send("Input.dispatchTouchEvent", {
				type: "touchStart",
				touchPoints: points(y),
			});
			for (let step = 1; step <= 8; step += 1)
				await cdp.send("Input.dispatchTouchEvent", {
					type: "touchMove",
					touchPoints: points(y + ((target - y) * step) / 8),
				});
			await cdp.send("Input.dispatchTouchEvent", {
				type: "touchEnd",
				touchPoints: [],
			});
		} finally {
			await cdp.detach();
		}
	} else {
		await page.mouse.move(x, y);
		await page.mouse.down();
		await page.mouse.move(x, target, { steps: 8 });
		await page.mouse.up();
	}
	await expect
		.poll(() =>
			handle.evaluate((node, target) => {
				const box = node.getBoundingClientRect();
				return Math.abs(box.top + box.height / 2 - target);
			}, target),
		)
		.toBeLessThanOrEqual(0.75);
	const delta = await audio.evaluate((lane) => {
		const line = lane.querySelector<SVGPolylineElement>(
			".timecode-audio-keyframe-curve polyline",
		)!;
		const vertex = line.points.getItem(0);
		const point = new DOMPoint(vertex.x, vertex.y).matrixTransform(
			line.getScreenCTM()!,
		);
		const box = lane.querySelector(".item-volume")!.getBoundingClientRect();
		return Math.max(
			Math.abs(point.x - box.left - box.width / 2),
			Math.abs(point.y - box.top - box.height / 2),
		);
	});
	expect(delta).toBeLessThanOrEqual(0.75);
}

async function gripMetrics(handle: Locator) {
	return handle.evaluate((node) => {
		const target = node.getBoundingClientRect(),
			pill = node.querySelector<HTMLElement>(":scope > span")!,
			box = pill.getBoundingClientRect();
		const bars = ["::before", "::after"].map((pseudo) => {
			const style = getComputedStyle(pill, pseudo),
				width = Number.parseFloat(style.width),
				height = Number.parseFloat(style.height);
			const left =
				box.left +
				pill.clientLeft +
				(style.left === "auto"
					? pill.clientWidth - Number.parseFloat(style.right) - width
					: Number.parseFloat(style.left));
			return {
				center: left + width / 2,
				width,
				height,
				visible: Math.max(
					0,
					Math.min(target.right, left + width) - Math.max(target.left, left),
				),
			};
		});
		return {
			left: target.left,
			right: target.right,
			width: target.width,
			pillCenter: box.left + box.width / 2,
			pillWidth: box.width,
			pillHeight: box.height,
			visible:
				Math.min(target.right, box.right) - Math.max(target.left, box.left),
			overflow: getComputedStyle(node).overflowX,
			bars,
		};
	});
}
async function visibleHalfBarOffset(handle: Locator) {
	const side = await handle.evaluate((node) =>
		node.classList.contains("cue-start-grip")
			? "start"
			: node.classList.contains("cue-end-grip")
				? "end"
				: null,
	);
	if (!side) return 0;
	const shape = await gripMetrics(handle);
	return (
		shape.bars[side === "start" ? 1 : 0].center - (shape.left + shape.width / 2)
	);
}
async function assertGripBars(
	editor: Locator,
	handle: Locator,
	frame: number,
	side?: "start" | "end",
) {
	const axis = await editor
		.locator(".timecode-timeline-canvas")
		.evaluate((node) => ({
			left: node.getBoundingClientRect().left,
			origin: Number(node.dataset.timeOriginPx),
			scale: Number(node.dataset.pixelsPerFrame),
		}));
	const expected = axis.left + axis.origin + frame * axis.scale,
		shape = await gripMetrics(handle);
	expect(shape.width).toBe(44);
	expect(shape.pillWidth).toBe(20);
	expect(shape.pillHeight).toBe(24);
	expect(Math.abs(shape.pillCenter - expected)).toBeLessThanOrEqual(0.75);
	const [left, right] = shape.bars;
	expect(left.width).toBe(2);
	expect(right.width).toBe(2);
	expect(left.height).toBe(14);
	expect(right.height).toBe(14);
	expect(left.center).toBeLessThan(expected);
	expect(right.center).toBeGreaterThan(expected);
	expect(
		Math.abs(expected - left.center - (right.center - expected)),
	).toBeLessThanOrEqual(0.75);
	if (side) {
		expect(
			Math.abs((side === "start" ? shape.left : shape.right) - expected),
		).toBeLessThanOrEqual(0.75);
		expect(shape.overflow).toBe("hidden");
		expect(Math.abs(shape.visible - shape.pillWidth / 2)).toBeLessThanOrEqual(
			0.75,
		);
		expect(left.visible).toBe(side === "start" ? 0 : 2);
		expect(right.visible).toBe(side === "start" ? 2 : 0);
	} else {
		expect(left.visible).toBe(2);
		expect(right.visible).toBe(2);
	}
}
async function assertHalfGrip(
	editor: Locator,
	handle: Locator,
	frame: number,
	side: "start" | "end",
) {
	await assertGripBars(editor, handle, frame, side);
}

async function setSelectionSidebar(page: Page, visible: boolean) {
	await page
		.locator(".timecode-window.timecode-editor")
		.getByRole("button", { name: "Settings", exact: true })
		.click();
	const settings = page.getByRole("dialog", {
		name: "Timecode Settings",
		exact: true,
	});
	await settings.getByRole("tab", { name: "Generic", exact: true }).click();
	await settings
		.getByRole("checkbox", { name: "Show selection sidebar", exact: true })
		.setChecked(visible);
	await settings.getByRole("button", { name: /Close/ }).click();
}

async function assertOverviewMainAxis(editor: Locator) {
	await expect.poll(() => editor.evaluate(node => {
		const canvas = node.querySelector<HTMLElement>(".timecode-timeline-canvas")!;
		const viewport = node.querySelector<HTMLElement>(".timecode-timeline-scroll")!;
		const overview = node.querySelector<HTMLElement>(".timecode-timeline-overview")!;
		const laneTrack = overview.querySelector<HTMLElement>(".timecode-timeline-overview-lane")!.getBoundingClientRect();
		const canvasBounds = canvas.getBoundingClientRect();
		const originWidth = Number(canvas.dataset.timeOriginPx);
		const origin = canvasBounds.left + originWidth;
		const extent = canvasBounds.width - originWidth;
		const project = (x: number) => laneTrack.left + ((x - origin) / extent) * laneTrack.width;
		let error = 0;
		for (const lane of canvas.querySelectorAll<HTMLElement>(".timecode-editor-lane")) {
			const miniature = overview.querySelector<HTMLElement>(`.timecode-timeline-overview-lane[data-overview-lane-id="${lane.dataset.laneId}"]`);
			if (!miniature) throw new Error(`Missing overview lane ${lane.dataset.laneId}`);
			const items = lane.querySelectorAll<HTMLElement>(".timecode-timeline-item");
			const minis = miniature.querySelectorAll<HTMLElement>(":scope > i");
			if (items.length !== minis.length) throw new Error(`Overview item count differs for ${lane.dataset.laneId}`);
			items.forEach((item, index) => {
				const bounds = item.getBoundingClientRect(), mini = minis[index].getBoundingClientRect();
				const clip = item.classList.contains("item-clip");
				const start = clip ? bounds.left : bounds.left + bounds.width / 2;
				error = Math.max(error, Math.abs(mini.left - project(start)));
				if (clip) error = Math.max(error, Math.abs(mini.right - project(bounds.right)));
			});
		}
		const markers = canvas.querySelectorAll<HTMLElement>(".timecode-timeline-marker");
		const miniMarkers = overview.querySelectorAll<HTMLElement>(".timecode-timeline-overview-marker");
		markers.forEach((marker, index) => {
			error = Math.max(error, Math.abs(miniMarkers[index].getBoundingClientRect().left - project(marker.getBoundingClientRect().left)));
		});
		const visible = overview.querySelector<HTMLElement>(".timecode-timeline-overview-visible")!.getBoundingClientRect();
		const expectedLeft = laneTrack.left + viewport.scrollLeft / extent * laneTrack.width;
		const expectedWidth = Math.min(1, (viewport.clientWidth - originWidth) / extent) * laneTrack.width;
		error = Math.max(error, Math.abs(visible.left - expectedLeft), Math.abs(visible.width - expectedWidth));
		return error;
	})).toBeLessThanOrEqual(1);
}

async function fitUsingOverview(page: Page, editor: Locator) {
	const overview = editor.getByRole("scrollbar", {
		name: "Timeline overview",
		exact: true,
	});
	const bounds = (await overview.boundingBox())!;
	for (const [name, x] of [
		["Resize timeline overview from start", bounds.x - 1],
		["Resize timeline overview from end", bounds.x + bounds.width + 1],
	] as const) {
		const edge = editor.getByRole("separator", { name, exact: true }),
			box = (await edge.boundingBox())!;
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down();
		await page.mouse.move(x, box.y + box.height / 2, { steps: 8 });
		await page.mouse.up();
	}
}

async function patchTimecode(
	api: ApiDriver,
	showId: string,
	id: string,
	patch: Partial<TimecodeDefinition>,
) {
	const current = await api.request<{
		objects: Array<{ revision: number; definition: TimecodeDefinition }>;
	}>("GET", "/api/v2/timecodes", undefined, true, undefined, { showId });
	await api.request(
		"POST",
		"/api/v2/timecodes/actions",
		{
			request_id: crypto.randomUUID(),
			action: {
				type: "update",
				timecode_id: id,
				expected_revision: current.objects.find(
					(item) => item.definition.id === id,
				)!.revision,
				patch,
			},
		},
		true,
		undefined,
		{ showId },
	);
}
async function assertNoMotionPress(
	page: Page,
	editor: Locator,
	handle: Locator,
	touch: boolean,
) {
	await handle.scrollIntoViewIfNeeded();
	await page.waitForTimeout(100);
	const box = (await handle.boundingBox())!;
	const snapshot = () =>
		handle.evaluate((node) => {
			const b = node.getBoundingClientRect(),
				lane = node.closest(".timecode-editor-lane")!.getBoundingClientRect(),
				canvas = node.closest<HTMLElement>(".timecode-timeline-canvas")!;
			return {
				x: b.x,
				y: b.y,
				width: b.width,
				height: b.height,
				laneHeight: lane.height,
				scale: canvas.dataset.pixelsPerFrame,
				frame: node.getAttribute("aria-valuenow"),
			};
		});
	const before = await snapshot(),
		x = box.x + box.width / 2 + Math.min(2, box.width / 8),
		y = box.y + box.height / 2;
	const hit = await handle.evaluate(
		(node, point) => {
			const actual = document.elementFromPoint(point.x, point.y);
			return {
				matches: node.contains(actual),
				label: node.getAttribute("aria-label") ?? node.getAttribute("title"),
				actual: actual?.outerHTML.slice(0, 220),
				box: node.getBoundingClientRect().toJSON(),
				point,
				style: {
					pointer: getComputedStyle(node).pointerEvents,
					visibility: getComputedStyle(node).visibility,
					clip: getComputedStyle(node).clipPath,
				},
				hits: document
					.elementsFromPoint(point.x, point.y)
					.slice(0, 5)
					.map((x) => x.className),
			};
		},
		{ x, y },
	);
	expect(hit.matches, JSON.stringify(hit)).toBe(true);
	if (touch) {
		const cdp = await page.context().newCDPSession(page);
		try {
			await cdp.send("Input.dispatchTouchEvent", {
				type: "touchStart",
				touchPoints: [{ x, y, id: 1 }],
			});
			await cdp.send("Input.dispatchTouchEvent", {
				type: "touchMove",
				touchPoints: [{ x, y, id: 1 }],
			});
			await page.waitForTimeout(40);
			expect(await snapshot()).toEqual(before);
			await cdp.send("Input.dispatchTouchEvent", {
				type: "touchEnd",
				touchPoints: [],
			});
		} finally {
			await cdp.detach();
		}
	} else {
		await page.mouse.move(x, y);
		await page.mouse.down();
		await page.mouse.move(x, y);
		await page.waitForTimeout(40);
		expect(await snapshot()).toEqual(before);
		await page.mouse.up();
	}
	expect(await snapshot()).toEqual(before);
}
