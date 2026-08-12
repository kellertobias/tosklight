import { expect, test } from "@playwright/test";

test("Media Server dock and settings navigate between Storybook screens", async ({
	page,
}) => {
	await page.goto("/?path=/story/tosklight-media-server--library");

	const story = page.frameLocator("#storybook-preview-iframe");
	await story.getByRole("button", { name: "Visualizers" }).click();
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
