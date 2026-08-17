import { expect, type Page } from "@playwright/test";
import type { ApiDriver } from "../core/api";

export class BrowserSystemIntegrations {
	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
	) {}

	async expectFixtureProfileCreation(
		manufacturer: string,
		name: string,
	): Promise<void> {
		await this.enterSetup();
		await this.page
			.getByRole("button", { name: "Open Fixture Library", exact: true })
			.click();
		await this.page
			.getByRole("button", { name: "Create fixture", exact: true })
			.click();
		const editor = this.page.getByRole("dialog", {
			name: "Create fixture profile",
		});
		await editor.getByLabel(/^Manufacturer/).fill(manufacturer);
		await editor.getByLabel(/^Fixture name/).fill(name);
		await editor.getByRole("button", { name: "Save fixture" }).click();
		await expect(editor).toBeHidden();
		await expect
			.poll(async () =>
				(await this.api.fixtureLibrarySnapshot()).profiles.some(
					(profile: any) =>
						profile.manufacturer === manufacturer && profile.name === name,
				),
			)
			.toBe(true);
	}

	async expectMatterBridgeToggle(): Promise<void> {
		await this.enterSetup();
		await this.openSetupSection("Network & Inputs");
		// Network & Inputs files its sections as title-bar tabs.
		await this.page.getByRole("tab", { name: "Bridges", exact: true }).click();
		const settings = this.page.getByLabel("Matter bridge settings");
		await expect(
			settings.getByText(
				"Desk installation · shared across shows and Desktops",
			),
		).toBeVisible();
		const matterSwitch = settings.getByRole("switch", {
			name: "Matter server",
			exact: true,
		});
		const matterTrack = matterSwitch.locator("..").locator(".ui-switch-track");
		if (!(await matterSwitch.isChecked())) await matterTrack.click();
		await expect(matterSwitch).toBeChecked();
		await expect
			.poll(
				async () =>
					(await this.api.request<any>("GET", "/api/v2/configuration"))
						.configuration.matter_enabled,
			)
			.toBe(true);
		await matterTrack.click();
		await expect
			.poll(
				async () =>
					(await this.api.request<any>("GET", "/api/v2/configuration"))
						.configuration.matter_enabled,
			)
			.toBe(false);
	}

	async expectSoundToLightConfiguration(): Promise<void> {
		await this.page.locator(".mode-toggle").click();
		const speed = this.page.getByRole("button", {
			name: /Speed group A, .* BPM/,
		});
		await speed.click();
		await expect(
			this.page.getByRole("dialog", {
				name: "Speed Group A Sound to Light",
			}),
		).toHaveCount(0);
		await speed.click({ modifiers: ["Shift"] });
		const modal = this.page.getByRole("dialog", {
			name: "Speed Group A Sound to Light",
		});
		await modal.getByRole("button", { name: "Manual", exact: true }).click();
		await this.page
			.getByRole("option", { name: "Sound to Light", exact: true })
			.click();
		await modal.getByRole("button", { name: /Low ·/ }).click();
		await this.page.getByRole("option", { name: "Custom range" }).click();
		await modal.getByLabel("Custom low frequency").fill("45");
		await modal.getByLabel("Custom high frequency").fill("140");
		await modal.getByLabel("Minimum accepted BPM").fill("60");
		await modal.getByLabel("Maximum accepted BPM").fill("180");
		await modal.getByLabel("Signal hold seconds").fill("3.5");
		await modal.getByLabel("Sound multiplier").fill("2");
		await modal.getByRole("button", { name: "Apply" }).click();
		await expect(modal).toBeHidden();
		await this.api.request("POST", "/api/v2/speed-groups/A/observations", {
			captured_at_millis: 1,
			source_available: true,
			usable_signal: true,
			level: 0.72,
			selected_band_level: 0.84,
			detected_bpm: 120,
			confidence: 0.92,
		});
		await expect
			.poll(
				async () =>
					(await this.api.request<any>("GET", "/api/v2/speed-groups/A"))
						.snapshot.source,
			)
			.toBe("sound");
	}

	async expectHighlightErrorOverlay(): Promise<void> {
		await this.page.routeWebSocket("**/api/v2/events", (socket) => {
			const server = socket.connectToServer();
			socket.onMessage((message) => {
				const parsed = JSON.parse(String(message));
				if (parsed.type === "action" && parsed.action?.type === "highlight") {
					socket.send(
						JSON.stringify({
							protocol_version: 2,
							request_id: parsed.request_id,
							ok: false,
							revision: 0,
							error: "The Highlight action was rejected by the desk",
						}),
					);
				} else server.send(message);
			});
			server.onMessage((message) => socket.send(message));
		});
		await this.page.reload();
		await expect(this.page.locator(".connection-cover")).toBeHidden();
		await this.page.locator('[data-keypad-key="HIGH"]').click();
		const alert = this.page.locator("[data-highlight-error-alert]");
		await expect(alert).toContainText(
			"The Highlight action was rejected by the desk",
		);
		await alert
			.getByRole("button", { name: "Dismiss Highlight error" })
			.click();
		await expect(alert).toBeHidden();
	}

	private async enterSetup(): Promise<void> {
		if (await this.page.locator(".setup-window").isVisible()) return;
		await this.page.getByRole("button", { name: /Open show menu/ }).click();
		await this.page.getByRole("button", { name: "Enter Setup" }).click();
	}

	private async openSetupSection(name: string): Promise<void> {
		await this.page
			.locator(".setup-window nav")
			.getByRole("button", { name, exact: true })
			.click();
	}
}
