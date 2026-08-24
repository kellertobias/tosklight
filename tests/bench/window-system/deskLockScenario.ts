import { expect, type Page } from "@playwright/test";
import type { ApiDriver, Session } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type { LightBench } from "../core/lightBench";

/** Operator-level Desk Lock behavior shared by every application screen. */
export class BrowserDeskLock {
	constructor(
		private readonly api: ApiDriver,
		private readonly bench: LightBench,
		private readonly page: Page,
		private readonly desk: DeskDriver,
	) {}

	async expectPinProtectionAcrossScreens(): Promise<void> {
		const session = await this.desk.session();
		this.api.session = session;
		const peer = await this.page.context().newPage();
		const late = await this.page.context().newPage();
		try {
			await peer.goto(this.bench.baseUrl);
			await expect(peer.locator(".connection-cover")).toBeHidden({
				timeout: 10_000,
			});
			await this.api.setCommandLineText("");
			const wallpaper =
				"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='8' height='8'%3E%3Cpath fill='%23123456' d='M0 0h8v8H0z'/%3E%3C/svg%3E";
			await this.api.request("PUT", "/api/v2/desk-lock", {
				message: "Call the operator",
				wallpaper,
				unlock_mode: "pin",
				pin: "1234",
			});
			await this.lockFromMenu();
			const primaryLock = this.lockDialog(this.page);
			const peerLock = this.lockDialog(peer);
			await expect(primaryLock).toBeVisible();
			await expect(peerLock).toBeVisible();
			await expect(primaryLock).toContainText("Call the operator");
			await expect(primaryLock).toHaveCSS(
				"background-image",
				/data:image\/svg\+xml/,
			);

			await late.goto(this.bench.baseUrl);
			const lateLock = this.lockDialog(late);
			await expect(lateLock).toBeVisible();
			await expect(lateLock).toContainText("Call the operator");

			await primaryLock.getByLabel("PIN").fill("9999");
			await primaryLock.getByRole("button", { name: "Unlock Desk" }).click();
			await expect(primaryLock.getByText("Incorrect PIN")).toBeVisible();
			await expect(peerLock).toBeVisible();
			await primaryLock.getByLabel("PIN").fill("1234");
			await primaryLock.getByRole("button", { name: "Unlock Desk" }).click();
			await expect(primaryLock).toBeHidden();
			await expect(peerLock).toBeHidden();
			await expect(lateLock).toBeHidden();
			await expect.poll(async () => (await this.snapshot()).locked).toBe(false);
		} finally {
			await late.close();
			await peer.close();
		}
	}

	async expectButtonFallback(): Promise<void> {
		await this.api.request("PUT", "/api/v2/desk-lock", {
			message: "",
			wallpaper: "https://invalid.example.test/unavailable-lock-screen.png",
			unlock_mode: "button",
			pin: null,
		});
		await this.lockFromMenu();
		const lock = this.lockDialog(this.page);
		await expect(lock).toBeVisible();
		await expect(lock).toContainText("This desk is locked.");
		await expect(lock.getByLabel("PIN")).toHaveCount(0);
		await lock.getByRole("button", { name: "Unlock Desk" }).click();
		await expect(lock).toBeHidden();
	}

	async expectScreenSettingsOwnership(): Promise<void> {
		await this.page.getByRole("button", { name: /Open show menu/ }).click();
		await this.page
			.getByRole("button", { name: "Enter Setup", exact: true })
			.click();
		const navigation = this.page.locator(".setup-window nav");
		await navigation
			.getByRole("button", { name: "Screens & playback", exact: true })
			.click();
		const defaultScreen = this.page.locator(".default-screen-settings");
		const undo = this.page.getByRole("button", { name: "Undo", exact: true });
		await expect(undo).toBeDisabled();
		await expect(
			this.page.getByRole("button", { name: "Save changes", exact: true }),
		).toHaveCount(0);

		await defaultScreen
			.getByRole("button", { name: "Configure Playbacks", exact: true })
			.click();
		await this.page
			.getByRole("button", { name: "Close playback configuration" })
			.click();
		await expect(undo).toBeDisabled();
		await this.switchSetupSection(navigation, "Network & Inputs");
		await this.switchSetupSection(navigation, "Screens & playback");
		await expect(undo).toBeDisabled();

		const deskName = defaultScreen.getByLabel("Desk name");
		const originalName = await deskName.inputValue();
		await deskName.fill(`${originalName}-undo`);
		await expect(undo).toBeEnabled();
		await expect
			.poll(() => this.currentDeskName())
			.toBe(`${originalName}-undo`);
		await this.switchSetupSection(navigation, "Network & Inputs");
		await this.switchSetupSection(navigation, "Screens & playback");
		await undo.click();
		await expect(deskName).toHaveValue(originalName);
		await expect(undo).toBeDisabled();
		await expect.poll(() => this.currentDeskName()).toBe(originalName);

		const shortcuts = defaultScreen.getByRole("switch", {
			name: "Enable software keyboard shortcuts",
		});
		await expect(shortcuts).toBeChecked();
		await shortcuts.locator("..").locator(".ui-switch-track").click();
		await expect(shortcuts).not.toBeChecked();
		await expect
			.poll(() =>
				this.page.evaluate(
					() =>
						JSON.parse(localStorage.getItem("light.desk-controls") ?? "{}")
							.regularNumberShortcuts,
				),
			)
			.toBe(false);

		await this.page
			.getByRole("button", { name: "Configure desk lock", exact: true })
			.click();
		const settings = this.page.getByRole("dialog", { name: "Desk Lock" });
		const titleBar = settings.locator(":scope > .ui-modal-titlebar");
		const save = titleBar.getByRole("button", {
			name: "Save Lock Configuration",
		});
		await expect(save).toBeVisible();
		await settings.getByLabel("Lock message").fill("Stand by for the operator");
		await save.click();
		await expect(settings).toBeHidden();
		await expect
			.poll(async () => (await this.snapshot()).message)
			.toBe("Stand by for the operator");
	}

	private async lockFromMenu(): Promise<void> {
		await this.page.getByRole("button", { name: /Open show menu/ }).click();
		await this.page
			.getByRole("button", { name: "Lock Desk", exact: true })
			.click();
	}

	private lockDialog(page: Page) {
		return page.getByRole("dialog", { name: "Desk locked" });
	}

	private snapshot(): Promise<{ locked: boolean; message: string }> {
		return this.api.request("GET", "/api/v2/desk-lock");
	}

	private async switchSetupSection(
		navigation: ReturnType<Page["locator"]>,
		name: string,
	): Promise<void> {
		await navigation.getByRole("button", { name, exact: true }).click();
	}

	private async currentDeskName(): Promise<string> {
		const deskId = this.api.session?.desk.id;
		if (!deskId) throw new Error("Desk Lock requires an authenticated desk");
		const session = await this.api.request<Session>(
			"POST",
			"/api/v2/sessions",
			{ username: "Operator", desk_id: deskId },
			false,
		);
		return session.desk.name;
	}
}
