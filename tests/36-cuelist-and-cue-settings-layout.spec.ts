import { expect, test } from "./bench/core/fixtures";
import { loadCanonicalCopy, object, putObject } from "./support/catalog";

interface CueListSnapshot {
	cues: Array<{ fade_millis: number; out_delay_millis?: number }>;
}

test("CUELIST-LAYOUT-001 @ui › Cue timing and trigger cells open their own exact-property editors", async ({
	api,
	bench,
	desk,
	page,
}) => {
	await loadCanonicalCopy(api, bench, "cuelist-layout-001", "compact-rig");
	const cueListId = crypto.randomUUID();
	await putObject(api, "cue_list", cueListId, {
		id: cueListId,
		name: "Layout Sequence",
		priority: 0,
		mode: "sequence",
		looped: false,
		chaser_step_millis: 1000,
		speed_group: null,
		cues: [1, 2].map((number) => ({
			id: crypto.randomUUID(),
			number,
			name: `Cue ${number}`,
			changes: [],
			group_changes: [],
			fade_millis: 0,
			delay_millis: 0,
			trigger: { type: "manual" },
		})),
	});
	await putObject(api, "playback", "1", {
		number: 1,
		name: "Layout Sequence",
		target: { type: "cue_list", cue_list_id: cueListId },
		buttons: ["go_minus", "go", "flash"],
		button_count: 3,
		fader: "master",
		has_fader: true,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		color: "#20c997",
		flash_release: "release_all",
		protect_from_swap: false,
	});
	await putObject(api, "playback_page", "1", {
		number: 1,
		name: "Main",
		slots: { "1": 1 },
		virtual_playbacks: {},
	});

	await desk.open(api.baseUrl);
	await page.setViewportSize({ width: 1280, height: 720 });
	await openCuelistPoolFromDock(page);
	await page
		.locator(".cuelist-card")
		.filter({ hasText: "Layout Sequence" })
		.click();

	await expect(page.locator(".cue-table thead th")).toHaveText([
		"Preview",
		"No.",
		"Name",
		"Info",
		"Jump",
		"Jump Count",
		"Trigger",
		"Trigger Time",
		"In Delay",
		"In Fade",
		"Out Delay",
		"Out Fade",
	]);
	await expect(page.locator(".cue-properties")).toHaveCount(0);

	const firstRow = page.locator(".cue-table tbody tr").first();
	// The Trigger cell chooses a type and applies it at once, so it closes rather than saving.
	await firstRow.getByRole("button", { name: "Trigger", exact: true }).click();
	const trigger = page.getByRole("dialog", { name: "Trigger", exact: true });
	await expect(trigger).toBeVisible();
	for (const type of ["GO", "FOLLOW", "TIME", "TIMECODE"])
		await expect(
			trigger.getByRole("button", { name: new RegExp(`^${type}\\b`) }),
		).toBeVisible();
	await trigger.getByRole("button", { name: "Close Trigger" }).click();
	await expect(trigger).toBeHidden();

	for (const property of [
		"Trigger Time",
		"In Delay",
		"In Fade",
		"Out Delay",
		"Out Fade",
	]) {
		await firstRow.getByRole("button", { name: property, exact: true }).click();
		const modal = page.getByRole("dialog", { name: property, exact: true });
		await expect(modal).toBeVisible();
		// A timing cell edits its value on a keypad: ENTER commits it and Close leaves it alone.
		await expect(
			modal.getByRole("textbox", { name: `${property} · Cue 1 value` }),
		).toBeVisible();
		await expect(
			modal.getByRole("button", { name: "ENTER", exact: true }),
		).toBeVisible();
		await modal.getByRole("button", { name: `Close ${property}` }).click();
		await expect(modal).toBeHidden();
	}

	await firstRow.getByRole("button", { name: "In Fade", exact: true }).click();
	const fadeModal = page.getByRole("dialog", { name: "In Fade", exact: true });
	await expect(fadeModal.locator("..")).toHaveAttribute("data-modal-top", "true");
	// The value box is driven by the editor's own keypad rather than typed into.
	await fadeModal.getByRole("button", { name: "5", exact: true }).click();
	await fadeModal.getByRole("button", { name: "ENTER", exact: true }).click();
	await expect
		.poll(
			async () =>
				(await object<CueListSnapshot>(api, "cue_list", cueListId)).body.cues[0]
					.fade_millis,
		)
		.toBe(5_000);

	await firstRow.getByRole("button", { name: "Out Delay", exact: true }).click();
	const cancelModal = page.getByRole("dialog", {
		name: "Out Delay",
		exact: true,
	});
	await expect(cancelModal.locator("..")).toHaveAttribute("data-modal-top", "true");
	await cancelModal.getByRole("button", { name: "9", exact: true }).click();
	// Closing the editor leaves the Cue as it was: the editor asks first, and discarding keeps
	// the stored value untouched.
	await cancelModal.getByRole("button", { name: "Close Out Delay" }).click();
	await page
		.getByRole("dialog", { name: "Unsaved Out Delay changes" })
		.getByRole("button", { name: "Discard changes" })
		.click();
	expect(
		(await object<CueListSnapshot>(api, "cue_list", cueListId)).body.cues[0]
			.out_delay_millis,
	).toBeUndefined();

	await page
		.getByRole("button", { name: "Cuelist Settings", exact: true })
		.click();
	const settings = page.getByRole("dialog", { name: "Cuelist Settings" });
	await expect(settings).toBeVisible();
	await expect(page.locator(".cue-table")).toBeVisible();
	await expect(
		settings.getByRole("button", { name: "Save", exact: true }),
	).toBeVisible();
	await expect(
		settings.getByRole("button", { name: /Mode\s*\(Sequence\)/ }),
	).toBeVisible();
	await expect(settings.getByRole("heading", { level: 3 })).toHaveText([
		"Priority",
		"Restart behavior",
		"Timing",
	]);
});

async function openCuelistPoolFromDock(page: Page): Promise<void> {
	// The Cuelist Pool is a dock Built-in. SHIFT lives on the playback face of the programmer
	// surface and the digits on the other, so no single face offers the old SHIFT + 4 shortcut.
	const toggle = page.getByRole("button", {
		name: "Desktops / Built-ins",
		exact: true,
	});
	if ((await toggle.getAttribute("data-dock-mode")) !== "builtins")
		await toggle.click();
	await page
		.locator("[aria-label='Built-ins']")
		.getByRole("button", { name: "Cue Lists", exact: true })
		.click();
}
