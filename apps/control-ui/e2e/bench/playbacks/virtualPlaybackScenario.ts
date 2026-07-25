import { expect, type Locator, type Page } from "@playwright/test";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type {
	BrowserDesktops,
	PaneHandle,
} from "../window-system/desktopScenario";
import type { PaneType } from "../window-system/paneTypes";
import { PlaybackButton } from "./playbackScenario";

type VirtualPlaybackPane = PaneHandle<PaneType.VirtualPlaybacks>;

interface PlaybackPageBody {
	slots?: Record<string, number>;
}

export class BrowserVirtualPlaybacks {
	readonly expect = {
		cells: async (pane: VirtualPlaybackPane, count: number) =>
			expect(this.pane(pane).locator(".virtual-playback-cell")).toHaveCount(
				count,
			),
		button: async (pane: VirtualPlaybackPane, cell: number, label: string) =>
			expect(this.cell(pane, cell)).toContainText(label),
	};

	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly desktops: BrowserDesktops,
		private readonly showId: () => string,
	) {}

	async assignSource(
		pane: VirtualPlaybackPane,
		sourceName: string,
		cell: number,
	): Promise<number> {
		await this.desk.click(
			this.pane(pane).getByRole("button", {
				name: "Set Source",
				exact: true,
			}),
		);
		await this.desk.click(
			this.page.getByRole("button", { name: "BUILT-INS", exact: true }),
		);
		await this.desk.click(
			this.page.locator(".dock-entry").filter({ hasText: "Cuelists" }),
		);
		await this.desk.click(
			this.page.locator(".cuelist-card").filter({ hasText: sourceName }),
		);
		await this.restorePane(pane);
		await this.desk.click(this.emptyCell(pane, cell));
		await expect(this.cell(pane, cell, sourceName)).toBeVisible();
		const playbackPage = await this.api.showObject<PlaybackPageBody>(
			this.showId(),
			"playback_page",
			"1",
		);
		const playback = playbackPage?.body.slots?.[String(cell)];
		if (!Number.isSafeInteger(playback))
			throw new Error(
				`Virtual Playback cell ${cell} did not persist a Playback source`,
			);
		return playback;
	}

	async configureTopButton(
		pane: VirtualPlaybackPane,
		cell: number,
		button: PlaybackButton,
	): Promise<void> {
		await this.desk.click(
			this.page.getByRole("button", { name: "SET", exact: true }),
		);
		await this.desk.click(this.cell(pane, cell));
		const dialog = this.page.getByRole("dialog", {
			name: "Playback Configuration",
		});
		await expect(dialog).toHaveAttribute(
			"data-topology",
			"1 button · faderless",
		);
		await this.desk.click(
			dialog.getByRole("button", { name: "Layout", exact: true }),
		);
		await chooseOption(
			this.page,
			dialog,
			"Top button",
			playbackButtonLabel(button),
		);
		await this.desk.click(
			dialog.getByRole("button", { name: "Apply", exact: true }),
		);
		await expect(dialog).toBeHidden();
	}

	async activate(pane: VirtualPlaybackPane, cell: number): Promise<void> {
		await this.desk.click(this.cell(pane, cell));
	}

	async reload(pane: VirtualPlaybackPane): Promise<void> {
		await this.page.waitForTimeout(900);
		await this.page.reload();
		await expect(this.page.locator(".connection-cover")).toBeHidden({
			timeout: 10_000,
		});
		await expect(this.pane(pane)).toBeVisible();
	}

	private pane(pane: VirtualPlaybackPane): Locator {
		return this.desktops.locatorFor(pane);
	}

	private async restorePane(pane: VirtualPlaybackPane): Promise<void> {
		await this.desk.click(
			this.page.getByRole("button", { name: "DESKTOPS", exact: true }),
		);
		if (
			!(await this.pane(pane)
				.isVisible()
				.catch(() => false))
		) {
			const activeDesktop = this.page.locator(".dock-list .dock-entry.active");
			if (await activeDesktop.isVisible().catch(() => false))
				await this.desk.click(activeDesktop);
			else
				await this.desk.click(
					this.page.locator(".dock-list .dock-entry").last(),
				);
		}
		await expect(this.pane(pane)).toBeVisible();
	}

	private cell(
		pane: VirtualPlaybackPane,
		cell: number,
		sourceName?: string,
	): Locator {
		const suffix = sourceName ? ` ${escapeRegExp(sourceName)}` : "";
		return this.pane(pane).getByRole("button", {
			name: new RegExp(
				`Virtual playback page 1 cell ${validCell(cell)}${suffix}`,
			),
		});
	}

	private emptyCell(pane: VirtualPlaybackPane, cell: number): Locator {
		return this.pane(pane).getByRole("button", {
			name: new RegExp(`Virtual playback page 1 cell ${validCell(cell)} empty`),
		});
	}
}

function validCell(cell: number): number {
	if (!Number.isSafeInteger(cell) || cell < 1)
		throw new Error("Virtual Playback cell must be a positive integer");
	return cell;
}

function playbackButtonLabel(button: PlaybackButton): string {
	const labels: Partial<Record<PlaybackButton, string>> = {
		[PlaybackButton.Go]: "Go",
		[PlaybackButton.GoBack]: "Go back",
		[PlaybackButton.On]: "On",
		[PlaybackButton.Off]: "Off",
		[PlaybackButton.Toggle]: "Toggle",
		[PlaybackButton.Flash]: "Flash",
		[PlaybackButton.Temp]: "Temp",
		[PlaybackButton.Swap]: "Swap",
		[PlaybackButton.Pause]: "Pause",
		[PlaybackButton.Empty]: "None",
	};
	const label = labels[button];
	if (!label)
		throw new Error(`Playback button ${button} is not assignable in a slot`);
	return label;
}

async function chooseOption(
	page: Page,
	root: Locator,
	label: string,
	option: string,
): Promise<void> {
	await root
		.locator(".ui-form-field")
		.filter({ hasText: new RegExp(`^\\s*${escapeRegExp(label)}`) })
		.locator(".ui-select-trigger")
		.click();
	await page
		.getByRole("dialog", { name: `Choose ${label} function` })
		.getByRole("button", {
			name: new RegExp(`^${escapeRegExp(option)}(?:\\s|$)`),
		})
		.click();
}

function escapeRegExp(value: string): string {
	return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
