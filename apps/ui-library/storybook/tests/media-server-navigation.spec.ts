import { expect, test } from "@playwright/test";

test("Media Server dock and settings navigate between Storybook screens", async ({
	page,
}) => {
	await page.goto("/?path=/story/tosklight-media-server--library");

	const story = page.frameLocator("#storybook-preview-iframe");
	await story.getByRole("button", { name: "Visualizers", exact: true }).click();
	await expect(page).toHaveURL(
		/\?path=\/story\/tosklight-media-server--visualizers$/u,
	);

	await story.getByRole("button", { name: "Settings" }).click();
	await expect(page).toHaveURL(
		/\?path=\/story\/tosklight-media-server--settings-libraries$/u,
	);

	await story.getByRole("radio", { name: "Network & Inputs" }).click();
	await expect(page).toHaveURL(
		/\?path=\/story\/tosklight-media-server--settings-network-and-inputs$/u,
	);
});

test("Media Server Library story exercises metadata, multi-move, folder configuration, and upload", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-media-server--library&viewMode=story",
	);

	const secondFolder = page.getByRole("button", {
		name: /002\s+Empty folder/u,
	});
	await secondFolder.click({ button: "right" });
	await page.getByLabel("Folder name").fill("Act two");
	await page.getByRole("button", { name: "Save folder" }).click();
	await expect(
		page.getByRole("button", { name: /002\s+Act two/u }),
	).toBeVisible();

	await page.getByRole("button", { name: /001\s+Show content/u }).click();
	await page.getByRole("button", { name: /Storm Clouds/u }).click();
	await page.getByLabel("Media name").fill("Storm reprise");
	await page.getByLabel("BPM").fill("126");
	await page.getByRole("button", { name: "Save media" }).click();
	await expect(
		page.getByRole("button", { name: /Storm reprise.*126 BPM/u }),
	).toBeVisible();

	const first = page.getByRole("button", { name: /Storm reprise/u });
	const second = page.getByRole("button", { name: /Island sunrise/u });
	await first.click();
	await second.click({ modifiers: ["Meta"] });
	await expect(page.getByText("2 selected")).toBeVisible();
	await first.dragTo(page.getByRole("button", { name: /002\s+Act two/u }));
	await page.getByRole("button", { name: /002\s+Act two/u }).click();
	await expect(
		page.getByRole("button", { name: /Storm reprise/u }),
	).toBeVisible();
	await expect(
		page.getByRole("button", { name: /Island sunrise/u }),
	).toBeVisible();

	await page.evaluate(() => {
		const folder = [
			...document.querySelectorAll<HTMLButtonElement>(".media-library-folder"),
		].find((candidate) => candidate.textContent?.includes("Act two"));
		if (!folder) throw new Error("Act two folder is missing");
		const transfer = new DataTransfer();
		transfer.items.add(
			new File(["still"], "new-look.png", { type: "image/png" }),
		);
		folder.dispatchEvent(
			new DragEvent("drop", {
				bubbles: true,
				cancelable: true,
				dataTransfer: transfer,
			}),
		);
	});
	await expect(page.getByRole("button", { name: /new-look/u })).toBeVisible();
});

test("Media Server Library reorders pooled folders and occupied file slots", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-media-server--library&viewMode=story",
	);
	await page.locator('.media-library-folder[data-folder="900"]').waitFor({
		state: "attached",
	});

	await page.evaluate(() => {
		const first = document.querySelector<HTMLButtonElement>(
			'.media-library-folder[data-folder="1"]',
		);
		const parking = document.querySelector<HTMLButtonElement>(
			'.media-library-folder[data-folder="900"]',
		);
		if (!first || !parking) throw new Error("folder cards are missing");
		const transfer = new DataTransfer();
		first.dispatchEvent(
			new DragEvent("dragstart", { bubbles: true, dataTransfer: transfer }),
		);
		parking.dispatchEvent(
			new DragEvent("drop", { bubbles: true, dataTransfer: transfer }),
		);
	});
	await expect(
		page.getByRole("button", { name: /001\s+Parking\s+1\/254/u }),
	).toBeVisible();
	await expect(
		page.getByRole("button", { name: /900\s+Show content\s+4\/254/u }),
	).toBeVisible();

	await page.getByRole("button", { name: /900\s+Show content/u }).click();
	const storm = page.getByRole("button", { name: /Storm Clouds/u });
	const sunrise = page.getByRole("button", { name: /Island sunrise/u });
	await storm.dragTo(sunrise);
	await expect(storm).toHaveAttribute("data-pool-position", "17");
	await expect(sunrise).toHaveAttribute("data-pool-position", "11");
});

test("Media Server Playback keeps the shared Media Pane folder pool above its file pool", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-media-server--media&viewMode=story",
	);
	const folders = await page.locator(".media-folder-pool").boundingBox();
	const files = await page.locator(".media-file-pool").boundingBox();
	expect(folders).not.toBeNull();
	expect(files).not.toBeNull();
	expect(folders?.y ?? Number.POSITIVE_INFINITY).toBeLessThan(
		files?.y ?? Number.NEGATIVE_INFINITY,
	);
	expect(folders?.x).toBe(files?.x);
});
