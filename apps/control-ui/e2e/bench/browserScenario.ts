import {
	expect,
	type Locator,
	type Page,
	type TestInfo,
} from "@playwright/test";
import type { ControllableDesktopAction } from "../../src/platform/desktop/controllableBrowserDesktopBridge";
import type { ApiDriver } from "./api";
import { BrowserClock } from "./clockScenario";
import { BrowserCommands, BrowserKeypad } from "./commandScenario";
import type { DeskDriver } from "./desk";
import { BrowserDesktops } from "./desktopScenario";
import { BrowserDmx, type DmxUniverseExpectation } from "./dmxScenario";
import { BrowserEncoders } from "./encoderScenario";
import {
	type FixtureDMXExpectation,
	type FixtureDMXTarget,
	FixtureDmxAssertions,
} from "./fixtureDmx";
import { type SimulatedHardware, simulatedHardware } from "./hardwareScenario";
import { BrowserHighlight } from "./highlightScenario";
import type { LightBench, TestShow } from "./lightBench";
import {
	BrowserOutputPackets,
	type OutputPacketAssertion,
} from "./outputPacketScenario";
import { BrowserOutput } from "./outputScenario";
import type { BuiltInPaneType } from "./paneTypes";
import { builtInLabels } from "./paneTypes";
import type { DmxProtocol } from "./protocols";
import { BrowserTiming } from "./programmerFadeScenario";
import { BrowserRoutedSelection } from "./routedSelectionScenario";
import type { SelectionTarget } from "./selectionContract";
import { BrowserSelection } from "./selectionScenario";
import { BrowserShows } from "./showScenario";

export interface ScreenConfigurationIntent {
	name: string;
	desktop?: string;
	showDock?: boolean;
	showPlaybacks?: boolean;
	showPageControls?: boolean;
	display?: { id: string; name: string } | null;
	bounds?: { x: number; y: number; width: number; height: number };
	fullscreen?: boolean;
	desiredOpen?: boolean;
	playbacks?: {
		perRow: number;
		rows: Array<{ first: number; fader: boolean; buttons: number }>;
		pageMode: "follow-main" | "dedicated";
	};
}

export interface ScreenHandle {
	readonly name: string;
	open(): Promise<void>;
	close(): Promise<void>;
	remove(): Promise<void>;
	expectBridgeAction(type: ControllableDesktopAction["type"]): Promise<void>;
}

export class BrowserScenarioWorld {
	readonly desktop: BrowserDesktops;
	readonly app: BrowserApplication;
	readonly builtIn: BrowserBuiltIns;
	readonly screenshot: BrowserScreenshots;
	readonly screen: BrowserScreens;
	readonly show: BrowserShows;
	readonly clock: BrowserClock;
	readonly command: BrowserCommands;
	readonly keypad: BrowserKeypad;
	readonly selection: BrowserRoutedSelection;
	readonly encoder: BrowserEncoders;
	readonly hardware: SimulatedHardware;
	readonly highlight: BrowserHighlight;
	readonly dmx: BrowserDmx;
	readonly output: BrowserOutput;
	readonly timing: BrowserTiming;
	readonly expect: {
		dmx: (universe: number) => DmxUniverseExpectation;
		outputPacket: (
			protocol: DmxProtocol,
			universe: number,
			assertion: OutputPacketAssertion,
		) => Promise<void>;
		selection: (
			...targets: SelectionTarget[]
		) => ReturnType<BrowserSelection["expectSelection"]>;
	};
	readonly expectFixtureDMX: (
		target: FixtureDMXTarget,
		expected: FixtureDMXExpectation,
	) => Promise<void>;
	readonly expectFixtureDMXAbsent: (target: FixtureDMXTarget) => Promise<void>;

	constructor(
		page: Page,
		desk: DeskDriver,
		bench: LightBench,
		api: ApiDriver,
		initialShow: TestShow,
		testInfo: TestInfo,
	) {
		const attach = async (name: string, body: Buffer) => {
			const safeName = artifactName(name);
			await testInfo.attach(safeName, { body, contentType: "image/png" });
		};
		this.app = new BrowserApplication(page, desk, bench.baseUrl);
		this.builtIn = new BrowserBuiltIns(page);
		this.desktop = new BrowserDesktops(page, attach);
		this.screenshot = new BrowserScreenshots(page, attach, this.builtIn);
		this.screen = new BrowserScreens(page, desk);
		this.show = new BrowserShows(api, bench, desk, initialShow, page);
		this.clock = new BrowserClock(bench, desk);
		this.command = new BrowserCommands(api, desk, page);
		this.keypad = new BrowserKeypad(desk, page);
		this.hardware = simulatedHardware(bench, api);
		const coreSelection = new BrowserSelection(api);
		this.selection = new BrowserRoutedSelection(
			coreSelection,
			api,
			this.command,
			this.keypad,
			this.hardware,
			page,
			desk,
			`${testInfo.workerIndex}:${testInfo.title}`,
		);
		this.encoder = new BrowserEncoders(
			api,
			coreSelection,
			page,
			desk,
			this.hardware,
			`${testInfo.workerIndex}:${testInfo.title}:encoder`,
		);
		this.highlight = new BrowserHighlight(page, api, this.hardware);
		this.dmx = new BrowserDmx(api);
		this.output = new BrowserOutput(api, desk);
		this.timing = new BrowserTiming(
			api,
			page,
			desk,
			this.hardware,
			`${testInfo.workerIndex}:${testInfo.title}:timing`,
		);
		const outputPackets = new BrowserOutputPackets(bench);
		this.expect = {
			dmx: (universe) => this.dmx.expect(universe),
			outputPacket: (protocol, universe, assertion) =>
				outputPackets.expect(protocol, universe, assertion),
			selection: (...targets) => this.selection.expectSelection(...targets),
		};
		const fixtureDmx = new FixtureDmxAssertions(api, undefined, () =>
			bench.applicationTime(),
		);
		this.expectFixtureDMX = (target, expected) =>
			fixtureDmx.expect(target, expected);
		this.expectFixtureDMXAbsent = (target) => fixtureDmx.expectAbsent(target);
	}
}

export class BrowserApplication {
	readonly expect: { ready: () => Promise<void> };

	constructor(
		page: Page,
		private readonly desk: DeskDriver,
		private readonly baseUrl: string,
	) {
		this.expect = {
			ready: async () => {
				await expect(applicationRoot(page)).toBeVisible();
				await expect(page.locator(".connection-cover")).toBeHidden();
				await expect(page.locator(".connection-banner")).toBeHidden();
				await desk.session();
			},
		};
	}

	async open(): Promise<void> {
		await this.desk.enableControllableDesktop();
		await this.desk.open(this.baseUrl);
	}
}

export class BrowserBuiltIns {
	readonly expect: { active: (type: BuiltInPaneType) => Promise<void> };

	constructor(private readonly page: Page) {
		this.expect = {
			active: async (type) => expect(this.root(type)).toBeVisible(),
		};
	}

	async open(type: BuiltInPaneType): Promise<void> {
		await this.page
			.getByRole("button", { name: "BUILT-INS", exact: true })
			.click();
		await this.page
			.locator("[aria-label='Built-ins']")
			.getByRole("button", { name: builtInLabels[type], exact: true })
			.click();
		await this.expect.active(type);
	}

	root(type: BuiltInPaneType): Locator {
		return this.page.locator(
			`[data-light-surface="built-in"][data-pane-type="${type}"]`,
		);
	}
}

export class BrowserScreenshots {
	constructor(
		private readonly page: Page,
		private readonly attach: (name: string, body: Buffer) => Promise<void>,
		private readonly builtIns: BrowserBuiltIns,
	) {}

	async application(name: string): Promise<void> {
		await this.attach(name, await this.page.screenshot());
	}

	async builtIn(type: BuiltInPaneType, name: string): Promise<void> {
		await this.attach(name, await this.builtIns.root(type).screenshot());
	}

	async dialog(accessibleName: string, name: string): Promise<void> {
		await this.attach(
			name,
			await this.page
				.getByRole("dialog", { name: accessibleName, exact: true })
				.screenshot(),
		);
	}
}

class BrowserScreenHandle implements ScreenHandle {
	constructor(
		readonly name: string,
		private readonly runtimeId: string,
		private readonly page: Page,
		private readonly actions: readonly ControllableDesktopAction[],
	) {}

	async open(): Promise<void> {
		const card = this.card();
		const button = card.getByRole("button", { name: "Open Screen" });
		if (await button.count()) await button.click();
	}

	async close(): Promise<void> {
		const card = this.card();
		const button = card.getByRole("button", { name: "Close Screen" });
		if (await button.count()) await button.click();
	}

	async remove(): Promise<void> {
		const card = this.card();
		await card.getByRole("button", { name: "Remove Screen" }).click();
		await expect(card).toHaveCount(0);
	}

	async expectBridgeAction(
		type: ControllableDesktopAction["type"],
	): Promise<void> {
		await expect
			.poll(() => this.actions.some((action) => action.type === type))
			.toBe(true);
	}

	private card(): Locator {
		return this.page.locator(`[data-screen-id="${this.runtimeId}"]`);
	}
}

export class BrowserScreens {
	constructor(
		private readonly page: Page,
		private readonly desk: DeskDriver,
	) {}

	async create(
		configuration: ScreenConfigurationIntent,
	): Promise<ScreenHandle> {
		const control = await this.desk.enableControllableDesktop();
		if (configuration.display) control.setDisplays([configuration.display]);
		await this.openSetup();
		const before = await this.page.locator(".screen-settings-card").count();
		await this.page
			.getByRole("button", { name: "+ Add screen", exact: true })
			.click();
		const card = this.page.locator(".screen-settings-card").nth(before);
		await expect(card).toBeVisible();
		const runtimeId = await card.getAttribute("data-screen-id");
		if (!runtimeId) throw new Error("Created screen has no runtime identity");
		await card.getByLabel("Screen name").fill(configuration.name);
		if (configuration.desktop)
			await chooseOption(this.page, card, "Desktop", configuration.desktop);
		await setSwitch(card, "Show Dock", configuration.showDock);
		await setSwitch(card, "Show Playbacks", configuration.showPlaybacks);
		await setSwitch(card, "Show Page Controls", configuration.showPageControls);
		if (configuration.display)
			await chooseOption(
				this.page,
				card,
				"Physical Display",
				configuration.display.name,
			);
		await setSwitch(card, "Fullscreen", configuration.fullscreen);
		if (configuration.bounds) {
			for (const [label, value] of [
				["Window X", configuration.bounds.x],
				["Window Y", configuration.bounds.y],
				["Window width", configuration.bounds.width],
				["Window height", configuration.bounds.height],
			] as const) {
				await card.getByLabel(label).fill(String(value));
				await card.getByLabel(label).blur();
			}
		}
		if (configuration.playbacks)
			await this.configurePlaybacks(card, configuration.playbacks);
		const handle = new BrowserScreenHandle(
			configuration.name,
			runtimeId,
			this.page,
			control.actions,
		);
		if (configuration.desiredOpen) await handle.open();
		return handle;
	}

	private async openSetup(): Promise<void> {
		if (!(await this.page.locator(".setup-window").count())) {
			await this.page.getByRole("button", { name: /Open show menu/ }).click();
			await this.page
				.getByRole("button", { name: "Enter Setup", exact: true })
				.click();
		}
		await this.page
			.locator(".setup-window nav")
			.getByRole("button", { name: "Screens & playback", exact: true })
			.click();
	}

	private async configurePlaybacks(
		card: Locator,
		playback: NonNullable<ScreenConfigurationIntent["playbacks"]>,
	): Promise<void> {
		await card.getByRole("button", { name: "Configure Playbacks" }).click();
		const dialog = this.page.getByRole("dialog", {
			name: "Configure Playbacks",
		});
		await dialog.getByLabel("Playbacks per row").fill(String(playback.perRow));
		await chooseOption(
			this.page,
			dialog,
			"Page Mode",
			playback.pageMode === "dedicated" ? "Dedicated Page" : "Follow Main",
		);
		const rows = dialog.locator("[data-playback-row-index]");
		while ((await rows.count()) < playback.rows.length)
			await dialog.getByRole("button", { name: "Add Row" }).click();
		while ((await rows.count()) > playback.rows.length)
			await rows
				.last()
				.getByRole("button", { name: /Remove row/ })
				.click();
		for (let index = 0; index < playback.rows.length; index += 1) {
			const row = rows.nth(index);
			const desired = playback.rows[index];
			await row.getByLabel("First Playback Number").fill(String(desired.first));
			await setSwitch(row, "Fader", desired.fader);
			await row.getByLabel("Buttons").fill(String(desired.buttons));
		}
		await dialog.getByRole("button", { name: "Save", exact: true }).click();
	}
}

function applicationRoot(page: Page): Locator {
	return page.locator('[data-light-surface="application"]');
}

function artifactName(name: string): string {
	const safe = name
		.trim()
		.replace(/[^a-zA-Z0-9._-]+/g, "-")
		.replace(/^-+|-+$/g, "");
	if (!safe) throw new Error("Screenshot name must contain a letter or number");
	return safe;
}

async function setSwitch(
	root: Locator,
	name: string,
	value: boolean | undefined,
): Promise<void> {
	if (value === undefined) return;
	const control = root.getByRole("switch", { name });
	if ((await control.isChecked()) !== value)
		await control.locator("..").locator(".ui-switch-track").click();
}

async function chooseOption(
	page: Page,
	root: Locator,
	label: string,
	option: string,
): Promise<void> {
	await root
		.getByText(label, { exact: true })
		.locator("..")
		.getByRole("button")
		.click();
	await page
		.getByRole("listbox", { name: label })
		.getByRole("option", { name: option, exact: true })
		.click();
}
