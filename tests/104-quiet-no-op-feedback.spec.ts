import { expect, test } from "./bench/core/fixtures";
import { expectProgrammer, selectedNumbers } from "./support/catalog";

test.use({ viewport: { width: 1600, height: 1000 } });

test.describe("docs/testing/23-quiet-no-op-feedback.md", () => {
	test("NOTICE-001 @ui › Align with nothing selected shows a brief notice instead of a desk error", async ({
		api,
		desk,
		page,
	}) => {
		await api.executeCommandLine("FIXTURE 999");
		await expectProgrammer(api, (programmer) =>
			expect(programmer.selected).toEqual([]),
		);
		await desk.open(api.baseUrl);

		const align = page.getByRole("button", { name: "Align Off" }).first();
		const notice = page.getByLabel("Desk notice");
		await align.click();

		await expect(notice).toHaveText(
			/No fixtures selected\. Align stays Off; nothing changed\./u,
		);
		await expect(
			page.getByRole("status").filter({ has: notice }),
		).toHaveAttribute("aria-live", "polite");
		await expect(align).toHaveAccessibleName("Align Off");
		await expect(page.getByText("Desk needs attention")).toHaveCount(0);
		await expect(page.getByRole("alert", { name: "Desk failure" })).toHaveCount(
			0,
		);
		await expect(page.getByRole("button", { name: "Dismiss" })).toHaveCount(0);
		await expectProgrammer(api, (programmer) =>
			expect(programmer.values).toEqual([]),
		);
		// The notice leaves the encoder controls usable and focus where the operator left it.
		await expect(align).toBeFocused();
		await expect(notice).toBeHidden({ timeout: 6_000 });

		await align.click();
		await expect(notice).toBeVisible();
		await page.getByRole("button", { name: "Dismiss notice" }).click();
		await expect(notice).toBeHidden();

		await api.executeCommandLine("FIXTURE 10 THRU 11");
		await expect.poll(() => selectedNumbers(api)).toEqual([10, 11]);
		await page.getByRole("button", { name: "Align Off" }).first().click();
		await expect(
			page.getByRole("button", { name: "Align Left" }).first(),
		).toBeVisible();
		await expect(notice).toHaveCount(0);
		await expect(page.getByText("Desk needs attention")).toHaveCount(0);
	});
});
