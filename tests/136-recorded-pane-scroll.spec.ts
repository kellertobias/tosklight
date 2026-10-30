import { expect, test } from "@playwright/test";
import { build } from "esbuild";
import { fileURLToPath } from "node:url";

test("@ui recorded Desktop pools restore independent slots after resize, delayed content and layout reload", async ({
	page,
}, testInfo) => {
	const result = await build({
		entryPoints: [
			fileURLToPath(
				new URL(
					"./bench/pools/recordedPaneScroll.fixture.tsx",
					import.meta.url,
				),
			),
		],
		bundle: true,
		write: false,
		format: "iife",
		jsx: "automatic",
		outfile: testInfo.outputPath("recorded-scroll.js"),
		define: {
			"process.env.NODE_ENV": '"production"',
			"import.meta.glob": "fixtureGlob",
		},
		banner: { js: "function fixtureGlob() { return {}; }" },
		loader: {
			".woff2": "dataurl",
			".woff": "dataurl",
			".svg": "dataurl",
			".png": "dataurl",
		},
	});
	await page.setViewportSize({ width: 900, height: 650 });
	// A stable origin lets the fixture simulate serializing and reloading stored layout data.
	await page.route("http://pane-scroll.test/**", (route) =>
		route.fulfill({ contentType: "text/html", body: '<div id="root"></div>' }),
	);
	const mount = async () => {
		for (const file of result.outputFiles) {
			if (file.path.endsWith(".css"))
				await page.addStyleTag({ content: file.text });
		}
		// Production stylesheets are loaded before the application mounts.
		for (const file of result.outputFiles) {
			if (file.path.endsWith(".js"))
				await page.addScriptTag({ content: file.text });
		}
	};
	await page.goto("http://pane-scroll.test/");
	await mount();
	const scroller = page.locator(".ui-window-scroller");
	const card = (number: number) =>
		page.locator(`[data-pool-position="${number - 1}"]`);
	const move = (number: number) =>
		card(number).evaluate((node) => {
			const viewport = node.closest<HTMLElement>(".ui-window-scroller")!;
			viewport.scrollTop +=
				node.getBoundingClientRect().top - viewport.getBoundingClientRect().top;
		});
	await expect(card(384)).toHaveCount(1);
	await page
		.getByRole("button", { name: "Record positions", exact: true })
		.click();
	await page.getByRole("button", { name: "Later", exact: true }).click();
	await move(100);
	await page
		.getByRole("button", { name: "Record positions", exact: true })
		.click();
	const recorded = await page.getByTestId("recorded-state").textContent();
	await move(200);
	expect(await page.getByTestId("recorded-state").textContent()).toBe(recorded);
	await page.getByRole("button", { name: "Early", exact: true }).click();
	await expect(card(1)).toBeInViewport();
	await page.setViewportSize({ width: 600, height: 550 });
	await page
		.getByRole("button", { name: "Unload content", exact: true })
		.click();
	await page.getByRole("button", { name: "Later", exact: true }).click();
	await page.getByRole("button", { name: "Load content", exact: true }).click();
	await expect(card(100)).toBeInViewport();
	await expect
		.poll(() => scroller.evaluate((node) => node.scrollTop))
		.toBeGreaterThan(0);
	await Promise.all([
		page.waitForEvent("load"),
		page.getByRole("button", { name: "Restart layout", exact: true }).click(),
	]);
	await mount();
	await expect(page.getByTestId("active-view")).toHaveText("Later");

	await expect(card(100)).toBeInViewport();
	// Cloning records the current working area while preserving the earlier view's snapshot.
	await move(250);
	await page.getByRole("button", { name: "Clone view", exact: true }).click();
	await expect(page.getByTestId("active-view")).toHaveText("Desktop 3");
	await expect(card(250)).toBeInViewport();
	await page.getByRole("button", { name: "Later", exact: true }).click();
	await expect(card(100)).toBeInViewport();
	await page.getByRole("button", { name: "Early", exact: true }).click();
	await expect(card(1)).toBeInViewport();
});
