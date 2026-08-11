import { expect, test } from "./bench/core/fixtures";
import { loadCanonicalCopy, object, putObject } from "./support/catalog";

interface CueListSnapshot {
	cues: Array<{ fade_millis: number; out_delay_millis?: number }>;
}

test("CUELIST-LAYOUT-001 @ui › Cue timing and trigger cells use exact-property Save/Cancel modals", async ({
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
	const shift = page.getByRole("button", { name: "SHIFT", exact: true });
	if (!(await shift.isVisible().catch(() => false)))
		await page.locator(".mode-toggle").click();
	await shift.click();
	await page.getByRole("button", { name: "4", exact: true }).click();
	await page
		.locator(".cuelist-card")
		.filter({ hasText: "Layout Sequence" })
		.click();

	await expect(page.locator(".cue-table thead th")).toHaveText([
		"Preview",
		"No.",
		"Name",
		"Trigger",
		"Trigger Time",
		"In Delay",
		"In Fade",
		"Out Delay",
		"Out Fade",
	]);
	await expect(page.locator(".cue-properties")).toHaveCount(0);

	const firstRow = page.locator(".cue-table tbody tr").first();
	for (const property of [
		"Trigger",
		"Trigger Time",
		"In Delay",
		"In Fade",
		"Out Delay",
		"Out Fade",
	]) {
		await firstRow.getByRole("button", { name: property, exact: true }).click();
		const modal = page.getByRole("dialog", { name: property, exact: true });
		await expect(modal).toBeVisible();
		await expect(
			modal.getByRole("button", { name: "Save", exact: true }),
		).toBeVisible();
		await expect(
			modal.getByRole("button", { name: "Cancel", exact: true }),
		).toBeVisible();
		await modal.getByRole("button", { name: "Cancel", exact: true }).click();
	}

	await firstRow.getByRole("button", { name: "In Fade", exact: true }).click();
	const fadeModal = page.getByRole("dialog", { name: "In Fade", exact: true });
	await expect(fadeModal.locator("..")).toHaveAttribute("data-modal-top", "true");
	await fadeModal.getByRole("textbox", { name: "In Fade", exact: true }).fill("5");
	await fadeModal.getByRole("button", { name: "Save", exact: true }).click();
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
	await cancelModal
		.getByRole("textbox", { name: "Out Delay", exact: true })
		.fill("9");
	await cancelModal
		.getByRole("button", { name: "Cancel", exact: true })
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
