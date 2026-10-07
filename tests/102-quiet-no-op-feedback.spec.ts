import { expect, test } from "./bench/core/fixtures";
import { expectProgrammer, selectedNumbers } from "./support/catalog";

test.use({ viewport: { width: 1600, height: 1000 } });

test.describe("docs/testing/23-quiet-no-op-feedback.md", () => {
	test("NOTICE-001 @ui › Align with nothing selected stays silent and leaves controls usable", async ({
		api,
		desk,
		page,
	}, testInfo) => {
		await api.executeCommandLine("FIXTURE 999");
		await expectProgrammer(api, (programmer) =>
			expect(programmer.selected).toEqual([]),
		);
		await desk.open(api.baseUrl);

		const align = page.getByRole("button", { name: "Align Off" }).first();
		const notice = page.getByLabel("Desk notice");
		await align.click();

		await expect(notice).toHaveCount(0);
		await expect(align).toHaveAccessibleName("Align Off");
		await expect(page.getByText("Desk needs attention")).toHaveCount(0);
		await expect(page.getByRole("alert", { name: "Desk failure" })).toHaveCount(
			0,
		);
		await expect(page.getByRole("button", { name: "Dismiss" })).toHaveCount(0);
		await expectProgrammer(api, (programmer) =>
			expect(programmer.values).toEqual([]),
		);
		// No notice or modal steals focus, and a repeated no-op stays silent.
		await expect(align).toBeFocused();
		await align.click();
		await expect(notice).toHaveCount(0);

		await api.executeCommandLine("FIXTURE 10 THRU 11");
		await expect.poll(() => selectedNumbers(api)).toEqual([10, 11]);
		await page.getByRole("button", { name: "Align Off" }).first().click();
		await expect(
			page.getByRole("button", { name: "Align Left" }).first(),
		).toBeVisible();
		await expect(notice).toHaveCount(0);
		await expect(page.getByText("Desk needs attention")).toHaveCount(0);

		// Another control surface changes the same desk modifier; reconnect hydrates it.
		await api.alignProgrammerSelection("right");
		await expect(page.getByRole("button", { name: "Align Right" }).first()).toBeVisible();
		await page.reload();
		const right = page.getByRole("button", { name: "Align Right" }).first();
		await expect(right).toBeVisible();
		await right.click();
		const out = page.getByRole("button", { name: "Align Out" }).first();
		await expect(out).toBeVisible();
		await page.screenshot({ path: testInfo.outputPath("align-authority.png") });
		await out.click({ modifiers: ["Shift"] });
		await expect(page.getByRole("button", { name: "Align Off" }).first()).toBeVisible();
		await expect(notice).toHaveCount(0);
	});
});
