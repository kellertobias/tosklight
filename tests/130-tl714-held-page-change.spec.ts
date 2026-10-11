import type { Page } from "@playwright/test";
import { BrowserScenarioWorld } from "./bench/core/browserScenario";
import { expect, test } from "./bench/core/fixtures";
import { PlaybackButton } from "./bench/playbacks/playbackScenario";
import { Show } from "./bench/show/showScenario";

const original = { page: 1, position: 1 };
const destination = { page: 2, position: 1 };

async function prepare(t: BrowserScenarioWorld) {
	await t.show.use(Show.DefaultStage);
	await t.app.open();
	await t.app.expect.ready();
	await t.timing.programmerFade.via.api.set("0s");
	for (const [position, fixture, level] of [[1, 101, 25], [2, 102, 75]]) {
		await t.selection.clear();
		await t.selection.fixtures.via.api.item(fixture);
		await t.encoder.intensity.dimmer.via.api.set(level);
		const address = await t.record.playback(position);
		await t.encoder.clear();
		await t.playback.configure(address, {
			buttons: [PlaybackButton.Flash, PlaybackButton.Swap, PlaybackButton.On],
			buttonCount: 3,
			autoOff: false,
		});
		await t.playback.via.api.off(address);
	}
	await t.page.create(2);
	await t.page.rename(2, "Held destination");
	await t.page.map({
		sourceAddress: { page: 1, position: 2 },
		targetAddress: destination,
	});
	await t.page.via.api.select(1);
	await t.playback.open();
}

// Keyboard activates real visible Page buttons without releasing a pointer or held Enter.
async function selectDestinationWhileHeld(page: Page) {
	const software = page.locator(".playback-page-current:visible");
	const selector = (await software.count())
		? software.first()
		: page
				.locator(".hardware-control-summary")
				.getByRole("button", { name: /^Page \d+$/ });
	await selector.focus();
	await page.keyboard.press("Space");
	const choice = page
		.locator(".playback-page-modal")
		.getByRole("button")
		.filter({ hasText: "Held destination" });
	await expect(choice).toBeVisible();
	await choice.focus();
	await page.keyboard.press("Space");
	await expect(page.locator(".playback-page-modal")).not.toBeVisible();
}

for (const gesture of ["pointer", "keyboard"] as const) {
	test(
		`TL714-HELD-${gesture.toUpperCase()} @ui › held Flash survives a visible Page change until its original release`,
		async ({ api, bench, desk, page, show }, testInfo) => {
			testInfo.setTimeout(90_000);
			const t = new BrowserScenarioWorld(page, desk, bench, api, show, testInfo);
			try {
				await prepare(t);
				const flash = page
					.locator('.playback-fader-bank [data-page="1"][data-playback-slot="1"]:visible')
					.getByRole("button", { name: "FLASH", exact: true });
				if (gesture === "pointer") {
					await flash.hover();
					await page.mouse.down();
				} else {
					await flash.focus();
					await page.keyboard.down("Enter");
				}
				await t.playback.expect(original).runtime({ flash: true, temporary_active: true });
				await selectDestinationWhileHeld(page);
				await t.page.expect(2).selected();
				await t.playback.expect(original).runtime({ flash: true, temporary_active: true });
				await t.playback.expect(destination).runtime({ enabled: false, flash: false });
				await t.playback.via.api.on(destination);
				await t.playback.expect(destination).runtime({ enabled: true, flash: false });
				if (gesture === "pointer") await page.mouse.up();
				else await page.keyboard.up("Enter");
				await t.playback.expect(original).runtime({ flash: false, temporary_active: false, enabled: false });
				await t.playback.expect(destination).runtime({ enabled: true, flash: false });
			} finally {
				await page.mouse.up();
				await page.keyboard.up("Enter");
				await t.finish();
			}
		},
	);
}

test(
	"TL714-HELD-OSC @ui @osc › compact held capture and explicit-page independent sources survive Page changes",
	async ({ api, bench, desk, page, show }, testInfo) => {
		testInfo.setTimeout(90_000);
		const t = new BrowserScenarioWorld(page, desk, bench, api, show, testInfo);
		const first = await bench.osc();
		const second = await bench.osc();
		const firstId = `held-first-${crypto.randomUUID()}`;
		const secondId = `held-second-${crypto.randomUUID()}`;
		try {
			await prepare(t);
			await first.send("/light/subscribe", [firstId, "desk", first.feedbackPort]);
			await second.send("/light/subscribe", [secondId, "desk", second.feedbackPort]);
			await first.send("/light/desk/playback/1/flash", [true, 71]);
			await t.playback.expect(original).runtime({ flash: true });
			await selectDestinationWhileHeld(page);
			await t.page.expect(2).selected();
			await t.playback.expect(original).runtime({ flash: true });
			await t.playback.expect(destination).runtime({ flash: false });
			await second.send("/light/desk/playback/2/1/flash", [true, 71]);
			await t.playback.expect(destination).runtime({ flash: true });
			await first.send("/light/desk/playback/1/flash", [true, 71]);
			await first.send("/light/desk/playback/1/flash", [false, 71]);
			await t.playback.expect(original).runtime({ flash: false, temporary_active: false });
			await t.playback.expect(destination).runtime({ flash: true });
			await second.send("/light/desk/playback/2/1/flash", [false, 71]);
			await t.playback.expect(destination).runtime({ flash: false, temporary_active: false });
		} finally {
			await first.send("/light/unsubscribe", [firstId]);
			await second.send("/light/unsubscribe", [secondId]);
			await t.finish();
		}
	},
);
