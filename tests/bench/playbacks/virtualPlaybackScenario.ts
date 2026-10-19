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
import type { PlaybackAddress } from "../../../apps/light-desktop/src/features/playbackTopology/contracts";

type VirtualPlaybackPane = PaneHandle<PaneType.VirtualPlaybacks>;

interface PlaybackPageBody { assignments: Record<string, Record<string, any>> }
interface PlaybackSoloRegion { name: string; members: PlaybackAddress[] }
interface PlaybackSoloSnapshot { show_id: string; revision: number; regions: PlaybackSoloRegion[] }

export class BrowserVirtualPlaybacks {
	readonly expect = {
        logicalCells: async (pane: VirtualPlaybackPane, count: number) =>
            expect(this.pane(pane).locator("[data-playback-slot]")).toHaveCount(count),
        mountedCellsBelow: async (pane: VirtualPlaybackPane, maximum: number) =>
            expect.poll(() => this.pane(pane).locator("[data-playback-slot]").count()).toBeLessThan(maximum),
        button: async (pane: VirtualPlaybackPane, cell: number, label: string, page = 1) => {
            const card = this.cell(pane, cell, undefined, page);
            const tile = card.locator(".playback-tile-action");
            if (await tile.count()) await expect(tile).toHaveAccessibleDescription(new RegExp(label));
            else await expect(card).toContainText(label);
        },
        effectivePage: async (pane: VirtualPlaybackPane, page: number) =>
            expect(this.pane(pane).locator("[data-playback-slot]").first()).toHaveAttribute("data-page", String(validPage(page))),
		fence: async (
			pane: VirtualPlaybackPane,
			cell: number,
			sides: string,
			page = 1,
		) => {
			const target = this.cell(pane, cell, undefined, page);
			for (const side of ["top", "right", "bottom", "left"]) {
                const pattern = new RegExp(`(?:^| )solo-fence-${side}(?: |$)`);
                if (sides.split(" ").includes(side)) await expect(target).toHaveClass(pattern);
                else await expect(target).not.toHaveClass(pattern);
            }
			const overlay = this.pane(pane).locator(".playback-solo-fence-overlay");
			await expect(overlay).toBeVisible();
			await expect(overlay.locator("path").first()).toBeVisible();
			const expected = new Set(sides.split(" "));
			await expect.poll(() => target.evaluate((element) => {
				const bank = element.closest(".playback-fader-bank");
				const svg = bank?.querySelector<SVGSVGElement>(".playback-solo-fence-overlay");
				if (!bank || !svg) return null;
				const bounds = bank.getBoundingClientRect();
				const tile = element.getBoundingClientRect();
				const style = getComputedStyle(bank);
				const gapX = Number.parseFloat(style.columnGap);
				const gapY = Number.parseFloat(style.rowGap);
				const viewBox = svg.viewBox.baseVal;
				if (!bounds.width || !bounds.height || !viewBox.width || !viewBox.height) return null;
				// Independently measure the shared half-gap perimeter in bank pixels, then
				// convert to the SVG's coordinates. Rounded corners leave side midpoints intact.
				const left = Math.max(1.5, tile.left - bounds.left - gapX / 2);
				const right = Math.min(bounds.width - 1.5, tile.right - bounds.left + gapX / 2);
				const top = Math.max(1.5, tile.top - bounds.top - gapY / 2);
				const bottom = Math.min(bounds.height - 1.5, tile.bottom - bounds.top + gapY / 2);
				const midpoints = {
					top: [(left + right) / 2, top],
					right: [right, (top + bottom) / 2],
					bottom: [(left + right) / 2, bottom],
					left: [left, (top + bottom) / 2],
				};
				const paths = [...svg.querySelectorAll<SVGGeometryElement>("path")];
				const stroked = Object.fromEntries(Object.entries(midpoints).map(([side, [x, y]]) => {
					const point = new DOMPoint(
						viewBox.x + x * viewBox.width / bounds.width,
						viewBox.y + y * viewBox.height / bounds.height,
					);
					return [side, paths.some((path) => path.isPointInStroke(point))];
				}));
				return {
					positiveGaps: gapX > 0 && gapY > 0,
					stroked,
					closedRounded: paths.length > 0 && paths.every((path) => {
						const drawing = path.getAttribute("d") ?? "";
						return drawing.includes("Q") && drawing.trim().endsWith("Z");
					}),
					visibleTwoPixelStroke: paths.length > 0 && paths.every((path) => {
						const paint = getComputedStyle(path);
						return paint.strokeWidth === "2px" && paint.stroke !== "none"
							&& paint.visibility === "visible" && paint.display !== "none"
							&& Number(paint.opacity) > 0;
					}),
				};
			})).toEqual({
				positiveGaps: true,
				stroked: {
					top: expected.has("top"),
					right: expected.has("right"),
					bottom: expected.has("bottom"),
					left: expected.has("left"),
				},
				closedRounded: true,
				visibleTwoPixelStroke: true,
			});
		},
		runtime: async (
			identity: PlaybackAddress,
			expected: Record<string, unknown>,
		) =>
			expect
				.poll(async () => await this.runtime(identity))
				.toMatchObject(expected),
		assignment: async (
			identity: PlaybackAddress,
			expected: Record<string, unknown>,
		) => {
			const page = await this.api.showObject<PlaybackPageBody>(
				this.showId(),
				"playback_page",
				String(validPage(identity.page)),
			);
			expect(
				page?.body.assignments[String(validPosition(identity.position))],
			).toMatchObject(expected);
		},
		zones: async (zones: PlaybackSoloRegion[]) =>
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
	): Promise<PlaybackAddress> {
        const pages = await this.api.showObjects<PlaybackPageBody>(this.showId(), "playback_page");
        const sourcePlayback = pages.flatMap(page => Object.values(page.body.assignments)).find(definition => definition.name === sourceName);
		if (!sourcePlayback || sourcePlayback.target?.type !== "cue_list")
			throw new Error(
				`Playback source ${sourceName} does not target a Cuelist`,
			);
		const sourceCueList = await this.api.showObject<any>(
			this.showId(),
			"cue_list",
			sourcePlayback.target.cue_list_id,
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
			playbackButtonLabel(sourcePlayback.buttons[0]),
		);
		await this.desk.click(
			modal.getByRole("button", { name: "Apply", exact: true }),
		);
		// Apply closes the modal only once the desk has stored the playback, which takes
		// noticeably longer on a loaded machine than on a developer's.
		await expect(modal).toBeHidden({ timeout: 10_000 });
		await expect(this.cell(pane, cell, sourceName, page)).toBeVisible();
		const playbackPage = await this.api.showObject<PlaybackPageBody>(
			this.showId(),
			"playback_page",
			String(validPage(page)),
		);
		const position = panePlaybackAddress(cell, page).position;
		const playback =
			playbackPage?.body.assignments[String(position)];
		if (!playback)
			throw new Error(
				`Virtual Playback ${page}.${position} did not persist a dedicated source`,
			);
		return { page, position };
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
		// Apply may already clear SET. Only disarm a visibly active modifier; clicking
		// an inactive SET would arm configuration again before the ordinary action.
		const set = this.page.getByRole("button", { name: "SET", exact: true });
		const armed = () => set.evaluate((element) =>
			element.getAttribute("aria-pressed") === "true"
				|| element.classList.contains("patch-set-armed"),
		);
		if (await armed()) await this.desk.click(set);
		await expect.poll(armed).toBe(false);
	}

	async createSoloRegion(pane: VirtualPlaybackPane, name: string, cells: number[]): Promise<void> {
		if (cells.length < 2) throw new Error("A Solo Region needs two Playback members");
		const members = cells.map((cell) => panePlaybackAddress(cell));
		const before = await Promise.all(members.map((address) => this.runtime(address)));
		await this.desk.click(this.pane(pane).getByRole("button", { name: "Create Solo Region", exact: true }));
		for (const cell of cells) await this.desk.click(this.cell(pane, cell));
		await this.pane(pane).getByLabel("Solo Region name").fill(name);
		// Choosing members must not operate either existing or stopped Playback owners.
		expect(await Promise.all(members.map((address) => this.runtime(address)))).toEqual(before);
		await this.desk.click(this.pane(pane).getByRole("button", { name: "Save Solo Region", exact: true }));
		await expect.poll(async () => (await this.zones()).find((region) => region.name === name))
			.toEqual({ name, members });
	}

    async createSoloRegionWithAttachedShift(pane: VirtualPlaybackPane, name: string, cells: number[]): Promise<void> {
        const alias = "desk";
        await this.hardware.connect();
        try {
            await this.hardware.send(`/light/${alias}/programmer/shift`, [true]);
            await this.createSoloRegion(pane, name, cells);
        } finally {
            await this.hardware.send(`/light/${alias}/programmer/shift`, [false]).catch(() => undefined);
            await this.hardware.disconnect();
        }
    }

	async deleteSoloRegion(
		pane: VirtualPlaybackPane,
		name: string,
	): Promise<void> {
		await this.desk.click(
			this.pane(pane).getByRole("button", { name: "Settings", exact: true }),
		);
		const settings = this.page.getByRole("dialog", { name: "Pane Settings" });
		await this.desk.click(
			settings.getByRole("tab", { name: "Manage Solo Regions", exact: true }),
		);
		await this.desk.click(settings.getByRole("button", { name, exact: true }));
		const remove = settings.getByRole("button", { name: "Delete", exact: true });
		await expect(remove).toBeVisible();
		await this.desk.click(remove);
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
		await this.desk.click(this.cell(pane, cell, undefined, page).getByRole("button", { name: /^(Playback \d+\.\d+:|GO|TOGGLE|TEMP|ON|Toggle|On)/ }).first());
	}

    async setMainPage(page: number): Promise<void> {
        if (!this.api.session) throw new Error("Playback page changes require an API session");
        await this.api.request("POST", "/api/v2/playback-surfaces/main/page", { page: validPage(page) }, true, undefined,
            { showId: this.showId(), deskId: this.api.session.desk.id });
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

    private cell(pane: VirtualPlaybackPane, cell: number, sourceName?: string, page = 1): Locator {
        const address = panePlaybackAddress(cell, page);
        const target = this.pane(pane).locator(`[data-page="${address.page}"][data-playback-slot="${address.position}"]`);
        return sourceName ? target.filter({ hasText: sourceName }) : target;
    }

    private emptyCell(pane: VirtualPlaybackPane, cell: number, page = 1): Locator {
        return this.cell(pane, cell, undefined, page);
    }

	private async runtime(identity: PlaybackAddress): Promise<any> {
		if (!this.api.session)
			throw new Error("Virtual Playback runtime requires an API session");
		const snapshot = await this.api.request<any>(
			"POST",
			"/api/v2/playback-runtime/snapshot",
			{
				identities: [
					{
						kind: "playback",
						page: validPage(identity.page),
						position: validPosition(identity.position),
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

	private async zones(): Promise<PlaybackSoloRegion[]> {
		if (!this.api.session)
			throw new Error("Virtual Playback zones require an API session");
		const response = await this.api.request<PlaybackSoloSnapshot>(
			"GET",
			"/api/v2/playback-solo-regions",
			undefined,
			true,
			undefined,
			{ showId: this.showId() },
		);
		return response.regions
			.map((zone) => ({
				name: zone.name,
				members: [...zone.members],
			}))
			.sort((left, right) => left.name.localeCompare(right.name));
	}
}

function validPage(page: number): number {
    if (!Number.isSafeInteger(page) || page < 1 || page > 99) throw new Error("Playback page must be within 1-99");
    return page;
}
function validPosition(position: number): number {
    if (!Number.isSafeInteger(position) || position < 1 || position > 1000) throw new Error("Playback position must be within 1-1000");
    return position;
}
export function panePlaybackAddress(cell: number, page = 1, firstPosition = 401): PlaybackAddress {
    if (!Number.isSafeInteger(cell) || cell < 1) throw new Error("Playback cell must be a positive integer");
    validPosition(firstPosition);
    return { page: validPage(page), position: validPosition(firstPosition + cell - 1) };
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
