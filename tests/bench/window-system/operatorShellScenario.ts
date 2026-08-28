import { expect, type Locator, type Page } from "@playwright/test";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type { LightBench } from "../core/lightBench";

/** Operator-shell wording, fixture-browser, picker, and diagnostics contracts. */
export class BrowserOperatorShell {
	constructor(
		private readonly api: ApiDriver,
		private readonly bench: LightBench,
		private readonly page: Page,
		private readonly desk: DeskDriver,
	) {}

	async expectDesktopAndDeskTerminology(): Promise<void> {
		const physicalDesk = { ...this.api.session!.desk };
		const sessionId = this.api.session!.session_id;
		const desktops = this.page.getByRole("button", {
			name: "Desktops / Built-ins",
			exact: true,
		});
		await expect(desktops).toBeVisible();
		await expect(
			this.page.getByRole("button", { name: "DESKS", exact: true }),
		).toHaveCount(0);
		if ((await desktops.getAttribute("data-dock-mode")) !== "desks")
			await this.desk.click(desktops);
		await expect(
			this.page.getByRole("button", { name: "New desktop", exact: false }),
		).toBeVisible();
		const programming = this.dockEntry("Programming");
		await this.desk.click(programming);
		await expect(this.page.locator(".desk-pane")).toHaveCount(3);
		await this.longPress(programming);
		let settings = this.page.getByRole("dialog", {
			name: "Desktop settings",
		});
		await expect(
			settings.getByRole("heading", { name: "Desktop", exact: true }),
		).toBeVisible();
		await expect(
			this.page.getByRole("dialog", { name: "Desk settings" }),
		).toHaveCount(0);
		await settings.getByLabel("Name").fill("Programming Desktop");
		await settings.getByLabel("Name").press("Tab");
		await settings
			.getByRole("button", { name: "Clone current desktop" })
			.click();
		const clone = this.dockEntry("Desktop 4");
		await expect(clone).toBeVisible();
		await this.longPress(clone);
		settings = this.page.getByRole("dialog", { name: "Desktop settings" });
		await settings.getByRole("button", { name: "Delete desktop" }).click();
		await this.page
			.getByRole("alertdialog", { name: "Delete desktop" })
			.getByRole("button", { name: "Confirm delete" })
			.click();
		await expect(clone).toHaveCount(0);
		const bootstrap = await this.api.request<any>(
			"GET",
			"/api/v2/bootstrap",
			undefined,
			false,
		);
		expect(this.api.session).toMatchObject({
			session_id: sessionId,
			desk: physicalDesk,
		});
		expect(bootstrap.desk).toMatchObject({ id: physicalDesk.id });
		const hardware = await this.bench.osc();
		const clientId = `manual-019-${crypto.randomUUID()}`;
		try {
			await hardware.subscribe(clientId, "desk");
			const mark = hardware.mark();
			await hardware.send(
				`/light/desk/programmer/digit-1`,
				[true],
			);
			await hardware.expectAfter(
				mark,
				`/light/desk/feedback/command-line`,
			);
		} finally {
			await hardware
				.send("/light/unsubscribe", [clientId])
				.catch(() => undefined);
			await hardware.close();
		}
		await this.desk.click(this.page.locator(".dock-identity"));
		const show = this.page.locator(".show-modal");
		await expect(
			show.getByRole("button", { name: "Desk Status" }),
		).toBeVisible();
		await expect(
			show.getByRole("button", { name: "Shut Down Desk" }),
		).toBeVisible();
		await show.getByRole("button", { name: "Enter Setup" }).click();
		await expect(this.page.locator(".ui-window-title")).toHaveText("Desk Setup");
	}

	async expectFixtureBrowserAlignment(): Promise<void> {
		await this.page.getByRole("button", { name: /Open show menu/ }).click();
		await this.page
			.getByRole("button", { name: "Show Patch", exact: true })
			.click();
		await this.page
			.getByRole("button", { name: "+ Add fixture", exact: true })
			.click();
		const browser = this.page.locator(".fixture-browser-modal");
		await expect(
			browser.getByRole("textbox", { name: "Search", exact: true }),
		).toBeVisible();
		await expect(
			browser
				.locator(".fixture-picker-columns > section")
				.nth(0)
				.locator("button span")
				.first(),
		).toHaveCSS("text-align", "left");
		await expect(
			browser
				.locator(".fixture-picker-columns > section")
				.nth(1)
				.locator("button small")
				.first(),
		).toHaveCSS("text-align", "right");
		await browser.getByRole("button", { name: "Close Add fixture" }).click();
		await this.page.getByRole("button", { name: /Open show menu/ }).click();
		await this.page
			.getByRole("button", { name: "Enter Setup", exact: true })
			.click();
		await this.page
			.getByRole("button", { name: "Open Fixture Library", exact: true })
			.click();
		const library = this.page.getByRole("dialog", { name: "Fixture Library" });
		await expect(library).toBeVisible();
		await expect(
			library.getByRole("textbox", {
				name: "Search fixture library",
				exact: true,
			}),
		).toBeVisible();
		for (const name of ["Import GDTF", "Create fixture"])
			await expect(
				library.getByRole("button", { name, exact: true }),
			).toBeVisible();
		await expect(
			this.page
				.locator(".fixture-library-columns > section")
				.nth(0)
				.locator("button span")
				.first(),
		).toHaveCSS("text-align", "left");
		await expect(
			this.page
				.locator(".fixture-library-columns > section")
				.nth(1)
				.locator("button small")
				.first(),
		).toHaveCSS("text-align", "right");
		await expect(
			this.page.locator(".fixture-library-detail dd").first(),
		).toHaveCSS("text-align", "right");
	}

	async expectOperatorFilePickerContracts(): Promise<void> {
		const configuration = await this.api.request<any>(
			"GET",
			"/api/v2/configuration",
		);
		await this.api.request("PUT", "/api/v2/configuration", {
			...configuration.configuration,
			file_manager_system_picker_fallback: false,
		});
		const prefix = `manual-picker-${crypto.randomUUID()}`;
		const files = {
			invalid: `${prefix}.txt`,
			show: `${prefix}.show`,
			mvr: `${prefix}.mvr`,
			gdtf: `${prefix}.gdtf`,
			wallpaper: `${prefix}.png`,
			scene: `${prefix}.glb`,
		};
		try {
			for (const name of Object.values(files))
				await this.api.request("POST", "/api/v2/files/shows/operations", {
					operation: "create_file",
					sources: [],
					destination: "",
					name,
				});
			await this.page.getByRole("button", { name: /Open show menu/ }).click();
			await this.page
				.getByRole("button", { name: "Load", exact: true })
				.click();
			const loadShow = this.page.getByRole("dialog", { name: "Load show" });
			await loadShow
				.getByRole("button", { name: "Show from USB", exact: true })
				.click();
			await this.expectPickerConstraint(files.invalid, files.show);
			await loadShow
				.getByRole("button", { name: "Load from MVR", exact: true })
				.click();
			const mvr = this.page.getByRole("dialog", {
				name: "MVR import and export",
			});
			// Loading from MVR asks for the file itself, so the chooser may already be up. Waiting
			// for it is the only safe way to ask: a count() is a snapshot, and a chooser that opens
			// in the gap between the snapshot and the click lands on top of the button being
			// clicked, so the click retries against the overlay until it times out. The timeout is
			// the answer "it did not open on its own", not a failure.
			const mvrPicker = this.page.getByRole("dialog", {
				name: "Choose files or folders",
			});
			const pickerAlreadyOpen = await mvrPicker
				.waitFor({ state: "visible", timeout: 2000 })
				.then(() => true)
				.catch(() => false);
			if (!pickerAlreadyOpen)
				await mvr
					.getByRole("button", { name: "Choose MVR file", exact: true })
					.click();
			await this.expectPickerConstraint(files.invalid, files.mvr);
			await mvr.getByRole("button", { name: "Close modal" }).click();
			await this.page
				.locator(".show-modal")
				.getByRole("button", { name: "Enter Setup", exact: true })
				.click();
			const setupNav = this.page.locator(".setup-window nav");
			await this.page
				.getByRole("button", { name: "Open Fixture Library", exact: true })
				.click();
			await this.page
				.getByRole("button", { name: "Import GDTF", exact: true })
				.click();
			const gdtf = this.page.locator(".gdtf-import-modal");
			await gdtf
				.getByRole("button", { name: "Choose GDTF file", exact: true })
				.click();
			await this.expectPickerConstraint(files.invalid, files.gdtf);
			await gdtf.locator("header button").click();
			await this.page
				.getByRole("button", { name: "Create fixture", exact: true })
				.click();
			const editor = this.page.locator(".fixture-profile-editor-modal");
			await editor
				.getByRole("button", { name: "Choose fixture icon", exact: true })
				.click();
			await this.expectPickerConstraint(files.invalid, files.wallpaper);
			await editor
				.getByRole("button", {
					name: "Choose visualizer glb model",
					exact: true,
				})
				.click();
			await this.expectPickerConstraint(files.invalid, files.scene);
			await editor
				.getByRole("button", { name: "Close fixture editor" })
				.click();
			await this.page
				.getByRole("button", { name: "Close Fixture Library", exact: true })
				.click();
			await setupNav
				.getByRole("button", { name: "Screens & playback", exact: true })
				.click();
			await this.page
				.getByRole("button", { name: "Configure desk lock", exact: true })
				.click();
			await this.page
				.getByRole("button", { name: "Choose lock wallpaper", exact: true })
				.click();
			await this.expectPickerConstraint(files.invalid, files.wallpaper);
		} finally {
			await this.api
				.request("POST", "/api/v2/files/shows/operations", {
					operation: "delete",
					sources: Object.values(files),
				})
				.catch(() => undefined);
		}
	}

	private dockEntry(name: string): Locator {
		return this.page.locator(".dock-entry").filter({ hasText: name }).first();
	}

	private async longPress(target: Locator): Promise<void> {
		await target.dispatchEvent("pointerdown", {
			pointerId: 1,
			pointerType: "mouse",
			button: 0,
		});
		await this.page.waitForTimeout(700);
		await target.dispatchEvent("pointerup", {
			pointerId: 1,
			pointerType: "mouse",
			button: 0,
		});
	}

	private async expectPickerConstraint(
		invalidName: string,
		allowedName: string,
	): Promise<void> {
		const picker = this.page.getByRole("dialog", {
			name: "Choose files or folders",
		});
		await expect(picker).toBeVisible();
		await expect(
			picker.getByRole("button", {
				name: "Open system file picker",
				exact: true,
			}),
		).toHaveCount(0);
		const select = picker.getByRole("button", {
			name: "Select",
			exact: true,
		});
		await picker
			.getByRole("button", { name: `${invalidName}, file`, exact: true })
			.click();
		await expect(select).toBeDisabled();
		await picker
			.getByRole("button", { name: `${allowedName}, file`, exact: true })
			.click();
		await expect(select).toBeEnabled();
		await picker
			.getByRole("button", { name: "Close File Manager", exact: true })
			.click();
		await expect(picker).toHaveCount(0);
	}
}
