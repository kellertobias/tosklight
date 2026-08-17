import { expect, type Locator, type Page } from "@playwright/test";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type { SimulatedHardware } from "../hardware/hardwareScenario";
import type {
	BrowserDesktops,
	PaneHandle,
} from "../window-system/desktopScenario";
import type { PaneType } from "../window-system/paneTypes";
import { optionRadio } from "./playback-configuration/ui";
import { PlaybackButton } from "./playbackScenario";

type VirtualPlaybackPane = PaneHandle<PaneType.VirtualPlaybacks>;

interface PlaybackPageBody {
	slots?: Record<string, number>;
	virtual_playbacks?: Record<string, Record<string, unknown>>;
}

export interface VirtualPlaybackIdentity {
	page: number;
	playbackNumber: number;
}

interface VirtualPlaybackZone {
	id?: string;
	name: string;
	playback_numbers: number[];
}

interface VirtualPlaybackZoneSnapshot {
	show_id: string;
	revision: number;
	zones: VirtualPlaybackZone[];
}

export class BrowserVirtualPlaybacks {
	readonly expect = {
		logicalCells: async (pane: VirtualPlaybackPane, count: number) => {
			const grid = this.pane(pane).locator(".virtual-playback-grid");
			const logical = await grid.getAttribute("data-logical-cells");
			if (logical === null)
				await expect(grid.locator(".virtual-playback-box")).toHaveCount(count);
			else expect(Number(logical)).toBe(count);
		},
		mountedCellsBelow: async (pane: VirtualPlaybackPane, maximum: number) =>
			expect
				.poll(() => this.pane(pane).locator(".virtual-playback-box").count())
				.toBeLessThan(maximum),
		button: async (
			pane: VirtualPlaybackPane,
			cell: number,
			label: string,
			page = 1,
		) => expect(this.cell(pane, cell, undefined, page)).toContainText(label),
		effectivePage: async (pane: VirtualPlaybackPane, page: number) =>
			expect(
				this.pane(pane).locator(".virtual-playback-box").first(),
			).toHaveAccessibleName(
				new RegExp(
					`^Virtual playback ${virtualPlaybackNumber(1, page)} page ${validPage(page)} cell `,
				),
			),
		fence: async (
			pane: VirtualPlaybackPane,
			cell: number,
			sides: string,
			page = 1,
		) => {
			const target = this.cell(pane, cell, undefined, page);
			await expect(target).toHaveAttribute("data-exclusion-fence", sides);
			const expected = new Set(sides.split(" "));
			await expect
				.poll(() =>
					target.evaluate((element) => {
						const style = getComputedStyle(element, "::after");
						return {
							top: style.borderTopWidth,
							right: style.borderRightWidth,
							bottom: style.borderBottomWidth,
							left: style.borderLeftWidth,
							inset: [style.top, style.right, style.bottom, style.left],
						};
					}),
				)
				.toEqual({
					top: expected.has("top") ? "3px" : "0px",
					right: expected.has("right") ? "3px" : "0px",
					bottom: expected.has("bottom") ? "3px" : "0px",
					left: expected.has("left") ? "3px" : "0px",
					inset: ["-1px", "-1px", "-1px", "-1px"],
				});
		},
		runtime: async (
			identity: VirtualPlaybackIdentity,
			expected: Record<string, unknown>,
		) =>
			expect
				.poll(async () => await this.runtime(identity))
				.toMatchObject(expected),
		physicalRuntimeAbsent: async (playbackNumber: number) =>
			expect
				.poll(async () => {
					const overview = await this.api.request<any>(
						"GET",
						"/api/v2/playback-overview",
					);
					return overview.active.some(
						(entry: any) =>
							entry.playback_identity?.kind === "physical" &&
							entry.playback_identity?.playback_number === playbackNumber,
					);
				})
				.toBe(false),
		assignment: async (
			identity: VirtualPlaybackIdentity,
			expected: Record<string, unknown>,
		) => {
			const page = await this.api.showObject<PlaybackPageBody>(
				this.showId(),
				"playback_page",
				String(validPage(identity.page)),
			);
			expect(
				page?.body.virtual_playbacks?.[
					String(validVirtualNumber(identity.playbackNumber))
				],
			).toMatchObject(expected);
		},
		zones: async (zones: VirtualPlaybackZone[]) =>
			expect(await this.zones()).toEqual(zones),
	};

	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly desktops: BrowserDesktops,
		private readonly hardware: SimulatedHardware,
		private readonly showId: () => string,
	) {}

	async assignSource(
		pane: VirtualPlaybackPane,
		sourceName: string,
		cell: number,
		page = 1,
	): Promise<VirtualPlaybackIdentity> {
		const sourcePlayback = (
			await this.api.showObjects<any>(this.showId(), "playback")
		).find((playback) => playback.body.name === sourceName);
		if (!sourcePlayback || sourcePlayback.body.target?.type !== "cue_list")
			throw new Error(
				`Playback source ${sourceName} does not target a Cuelist`,
			);
		const sourceCueList = await this.api.showObject<any>(
			this.showId(),
			"cue_list",
			sourcePlayback.body.target.cue_list_id,
		);
		if (!sourceCueList)
			throw new Error(`Cuelist for playback source ${sourceName} is absent`);
		await this.emptyCell(pane, cell, page).click({ button: "right" });
		const modal = this.page.getByRole("dialog", {
			name: "Playback Configuration",
		});
		await expect(modal).toBeVisible();
		await modal.getByLabel("Playback name").fill(sourceName);
		await this.desk.click(
			optionRadio(modal, this.page, sourceCueList.body.name),
		);
		await this.desk.click(
			modal.getByRole("button", { name: "Layout", exact: true }),
		);
		await chooseOption(
			this.page,
			modal,
			"Top button",
			playbackButtonLabel(sourcePlayback.body.buttons[0]),
		);
		await this.desk.click(
			modal.getByRole("button", { name: "Apply", exact: true }),
		);
		await expect(modal).toBeHidden();
		await expect(this.cell(pane, cell, sourceName, page)).toBeVisible();
		const playbackPage = await this.api.showObject<PlaybackPageBody>(
			this.showId(),
			"playback_page",
			String(validPage(page)),
		);
		const playbackNumber = virtualPlaybackNumber(cell, page);
		const playback =
			playbackPage?.body.virtual_playbacks?.[String(playbackNumber)];
		if (!playback)
			throw new Error(
				`Virtual Playback ${page}.${playbackNumber} did not persist a dedicated source`,
			);
		return { page, playbackNumber };
	}

	async configureTopButton(
		pane: VirtualPlaybackPane,
		cell: number,
		button: PlaybackButton,
		page = 1,
	): Promise<void> {
		await this.desk.click(
			this.page.getByRole("button", { name: "SET", exact: true }),
		);
		await this.desk.click(this.cell(pane, cell, undefined, page));
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
		// SET is a latched desk modifier. Applying a configuration does not release it, so disarm it
		// before the next ordinary Virtual Playback activation.
		await this.desk.click(
			this.page.getByRole("button", { name: "SET", exact: true }),
		);
	}

	async createExclusionZone(
		pane: VirtualPlaybackPane,
		name: string,
		cells: number[],
	): Promise<void> {
		if (cells.length < 2)
			throw new Error("A Virtual Playback exclusion zone needs two cells");
		await this.page.keyboard.down("Shift");
		try {
			for (const cell of cells) await this.desk.click(this.cell(pane, cell));
		} finally {
			await this.page.keyboard.up("Shift");
		}
		await this.desk.click(
			this.pane(pane).getByRole("button", {
				name: "Create Exclusion Zone",
				exact: true,
			}),
		);
		const dialog = this.page.getByRole("dialog", {
			name: "Create Exclusion Zone",
		});
		await expect(dialog).toBeVisible();
		await this.page.waitForTimeout(100);
		const nameInput = dialog.getByLabel("Zone name");
		await nameInput.fill(name);
		await expect(nameInput).toHaveValue(name);
		await nameInput.press("Tab");
		await this.desk.click(
			dialog.getByRole("button", { name: "Create zone", exact: true }),
		);
		await expect(dialog).toBeHidden();
	}

	async createExclusionZoneWithAttachedShift(
		pane: VirtualPlaybackPane,
		name: string,
		cells: number[],
	): Promise<void> {
		if (cells.length < 2)
			throw new Error("A Virtual Playback exclusion zone needs two cells");
		const alias = this.api.session?.desk.osc_alias;
		if (!alias)
			throw new Error("Attached Shift proof requires a desk OSC alias");
		await this.hardware.connect();
		try {
			await this.hardware.send(`/light/${alias}/programmer/shift`, [true]);
			for (const cell of cells) {
				const target = this.cell(pane, cell);
				await this.desk.click(target);
				await expect(target).toHaveAttribute("aria-pressed", "true");
			}
			await expect(
				this.pane(pane).getByRole("button", {
					name: "Create Exclusion Zone",
					exact: true,
				}),
			).toBeVisible();
			await this.hardware
				.send(`/light/${alias}/programmer/shift`, [false])
				.catch(() => undefined);
			const createButton = this.pane(pane).getByRole("button", {
				name: "Create Exclusion Zone",
				exact: true,
			});
			await this.desk.click(createButton);
			const dialog = this.page.getByRole("dialog", {
				name: "Create Exclusion Zone",
			});
			await expect(dialog).toBeVisible();
			const nameInput = dialog.getByLabel("Zone name");
			await nameInput.fill(name);
			await expect(nameInput).toHaveValue(name);
			await nameInput.press("Tab");
			await this.desk.click(
				dialog.getByRole("button", { name: "Create zone", exact: true }),
			);
			await expect(dialog).toBeHidden();
		} finally {
			await this.hardware
				.send(`/light/${alias}/programmer/shift`, [false])
				.catch(() => undefined);
			await this.hardware.disconnect();
		}
	}

	async deleteExclusionZone(
		pane: VirtualPlaybackPane,
		name: string,
	): Promise<void> {
		await this.desk.click(
			this.pane(pane).getByRole("button", { name: "Settings", exact: true }),
		);
		const settings = this.page.getByRole("dialog", { name: "Pane Settings" });
		await this.desk.click(
			settings.getByRole("tab", { name: "Exclusion Zones", exact: true }),
		);
		await expect(settings.getByLabel(`Name for ${name}`)).toBeVisible();
		await this.desk.click(
			settings.getByRole("button", { name: "Delete zone", exact: true }),
		);
		await expect
			.poll(async () => (await this.zones()).some((zone) => zone.name === name))
			.toBe(false);
		await this.desk.click(
			settings.getByRole("button", { name: "Close settings", exact: true }),
		);
	}

	async activate(
		pane: VirtualPlaybackPane,
		cell: number,
		page = 1,
	): Promise<void> {
		await this.desk.click(this.cell(pane, cell, undefined, page));
	}

	async setMainPage(page: number): Promise<void> {
		if (!this.api.session)
			throw new Error("Virtual Playback page changes require an API session");
		await this.api.request(
			"POST",
			`/api/v2/control-desks/${this.api.session.desk.id}/actions`,
			{
				request_id: crypto.randomUUID(),
				action: {
					type: "set_page",
					page: validPage(page),
					existing_only: false,
				},
			},
		);
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

	private cell(
		pane: VirtualPlaybackPane,
		cell: number,
		sourceName?: string,
		page = 1,
	): Locator {
		const suffix = sourceName ? ` ${escapeRegExp(sourceName)}` : "";
		return this.pane(pane).getByRole("button", {
			name: new RegExp(
				`^Virtual playback ${virtualPlaybackNumber(cell, page)} page ${validPage(page)} cell ${validCell(cell)}${suffix}(?: |$)`,
			),
		});
	}

	private emptyCell(
		pane: VirtualPlaybackPane,
		cell: number,
		page = 1,
	): Locator {
		return this.pane(pane).getByRole("button", {
			name: new RegExp(
				`^Virtual playback ${virtualPlaybackNumber(cell, page)} page ${validPage(page)} cell ${validCell(cell)} empty$`,
			),
		});
	}

	private async runtime(identity: VirtualPlaybackIdentity): Promise<any> {
		if (!this.api.session)
			throw new Error("Virtual Playback runtime requires an API session");
		const snapshot = await this.api.request<any>(
			"POST",
			"/api/v2/playback-runtime/snapshot",
			{
				identities: [
					{
						kind: "virtual",
						page: validPage(identity.page),
						playback_number: validVirtualNumber(identity.playbackNumber),
					},
				],
			},
			true,
			undefined,
			{ showId: this.showId(), deskId: this.api.session.desk.id },
		);
		const projection = snapshot.projections?.[0];
		return projection?.target === "cue_list" && projection.runtime == null
			? { ...projection, runtime: { enabled: false } }
			: projection;
	}

	private async zones(): Promise<VirtualPlaybackZone[]> {
		if (!this.api.session)
			throw new Error("Virtual Playback zones require an API session");
		const response = await this.api.request<VirtualPlaybackZoneSnapshot>(
			"GET",
			"/api/v2/virtual-playback-exclusion-zones",
			undefined,
			true,
			undefined,
			{ showId: this.showId() },
		);
		return response.zones
			.map((zone) => ({
				name: zone.name,
				playback_numbers: [...zone.playback_numbers],
			}))
			.sort((left, right) => left.name.localeCompare(right.name));
	}
}

function validCell(cell: number): number {
	if (!Number.isSafeInteger(cell) || cell < 1 || cell > 300)
		throw new Error("Virtual Playback cell must be within 1-300");
	return cell;
}

function validPage(page: number): number {
	if (!Number.isSafeInteger(page) || page < 1 || page > 127)
		throw new Error("Virtual Playback page must be within 1-127");
	return page;
}

function validVirtualNumber(playbackNumber: number): number {
	if (
		!Number.isSafeInteger(playbackNumber) ||
		playbackNumber < 1001 ||
		playbackNumber > 39_100
	)
		throw new Error("Virtual Playback number must be within 1001-39100");
	return playbackNumber;
}

export function virtualPlaybackNumber(cell: number, page = 1): number {
	const playbackNumber = 1000 + 300 * (validPage(page) - 1) + validCell(cell);
	return validVirtualNumber(playbackNumber);
}

function playbackButtonLabel(button: PlaybackButton): string {
	const labels: Partial<Record<PlaybackButton, string>> = {
		[PlaybackButton.Go]: "GO +",
		[PlaybackButton.GoBack]: "GO −",
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
