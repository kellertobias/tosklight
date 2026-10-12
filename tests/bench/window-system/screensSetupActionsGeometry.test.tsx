import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { chromium } from "playwright";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { DefaultScreenSettings } from "../../../apps/light-desktop/src/components/setup/screens/ScreenSettingsCards";

const require = createRequire(import.meta.url);
function stylesheet(path: string): string {
	return readFileSync(path, "utf8").replace(
		/@import "([^"]+)";/g,
		(_, name: string) =>
			stylesheet(
				name.startsWith("@")
					? require.resolve(name)
					: resolve(dirname(path), name),
			),
	);
}
const applicationStyles = readFileSync(
	"apps/light-desktop/src/applicationStyles.ts",
	"utf8",
);
const css = [...applicationStyles.matchAll(/import "([^"]+\.css)";/g)]
	.map(([, name]) =>
		stylesheet(
			name.startsWith("@")
				? require.resolve(name)
				: resolve("apps/light-desktop/src", name),
		),
	)
	.join("\n");
const noop = () => {};
const markup = renderToStaticMarkup(
	<section className="setup-window">
		<div className="screens-setup-list">
			<DefaultScreenSettings
				deskName="Desk"
				onDeskName={noop}
				showCompactEncoders={false}
				onShowCompactEncoders={noop}
				onTextFocus={noop}
				onTextBlur={noop}
				keyboardShortcuts={true}
				onKeyboardShortcuts={noop}
				onConfigurePlaybacks={noop}
				onChooseDefault={noop}
				singleClientMode={true}
				onSingleClientMode={noop}
			/>
		</div>
	</section>,
);

it("keeps both default-screen actions fully inside the narrow CI Setup area with long information", async () => {
	const browser = await chromium.launch({ headless: true });
	try {
		const page = await browser.newPage({
			viewport: { width: 760, height: 900 },
		});
		// The CI trace measured413px available for row cells and a557px lower clip.
		// Render the actual card/fields/actions and complete application CSS cascade.
		await page.setContent(`<style>${css}
		.setup-window{position:absolute;left:290px;top:130px;width:435px;height:427px;overflow:hidden}
		.setup-window>.screens-setup-list{display:block;margin-top:0}
		</style>${markup}`);
		await page
			.locator(".default-screen-compact-row > .ui-form-field small")
			.first()
			.evaluate((element) => {
				element.textContent =
					"Keyboard shortcuts are always disabled while hardware controls are connected. " +
					"This deliberately long information text wraps over several lines so the row proves " +
					"that informational copy can never push, overlap, or misalign the neighbouring controls.";
			});
		const bounds = await page.evaluate(() => {
			const row = document.querySelector(".default-screen-compact-row");
			const actions = row?.querySelector(".screen-settings-actions");
			const clip = document.querySelector(".setup-window");
			if (!row || !actions || !clip)
				throw new Error("Setup fixture did not render its controls");
			return {
				clip: clip.getBoundingClientRect().toJSON(),
				row: row.getBoundingClientRect().toJSON(),
				fields: [...row.querySelectorAll(":scope > .ui-form-field")].map(
					(field) => field.getBoundingClientRect().toJSON(),
				),
				actions: actions.getBoundingClientRect().toJSON(),
				buttons: [...actions.querySelectorAll("button")].map((button) =>
					button.getBoundingClientRect().toJSON(),
				),
				horizontalOverflow: row.scrollWidth - row.clientWidth,
			};
		});
		expect(bounds.fields).toHaveLength(4);
		expect(bounds.buttons).toHaveLength(2);
		expect(bounds.horizontalOverflow).toBe(0);
		expect(bounds.fields[0].y).toBe(bounds.fields[1].y);
		expect(bounds.fields[2].y).toBe(bounds.fields[3].y);
		expect(bounds.buttons[0].y).toBe(bounds.buttons[1].y);
		for (const button of bounds.buttons) {
			expect(button.height).toBeGreaterThanOrEqual(44);
			expect(button.x).toBeGreaterThanOrEqual(bounds.row.x);
			expect(button.right).toBeLessThanOrEqual(bounds.row.right);
			expect(button.bottom, JSON.stringify(bounds)).toBeLessThanOrEqual(
				bounds.clip.bottom,
			);
		}
	} finally {
		await browser.close();
	}
});
