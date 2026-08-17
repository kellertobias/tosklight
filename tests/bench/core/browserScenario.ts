import {
	expect,
	type Locator,
	type Page,
	type TestInfo,
} from "@playwright/test";
import type { FixedScreenPane } from "../../../apps/light-desktop/src/api/types";
import type { ControllableDesktopAction } from "../../../apps/light-desktop/src/platform/desktop/controllableBrowserDesktopBridge";
import {
	BrowserCommands,
	BrowserKeypad,
} from "../command-selection/commandScenario";
import { BrowserRoutedSelection } from "../command-selection/routedSelectionScenario";
import type { SelectionTarget } from "../command-selection/selectionContract";
import { BrowserSelection } from "../command-selection/selectionScenario";
import { BrowserAttachedEncoders } from "../encoders/attachedEncoderScenario";
import { BrowserEncoders } from "../encoders/encoderScenario";
import { BrowserGroups } from "../groups-presets/groupScenario";
import { BrowserPresets } from "../groups-presets/presetScenario";
import { BrowserCrossSurface } from "../hardware/crossSurfaceScenario";
import {
	type SimulatedHardware,
	simulatedHardware,
} from "../hardware/hardwareScenario";
import { BrowserHardwareSimulator } from "../hardware/hardwareSimulatorScenario";
import { BrowserDmx, type DmxUniverseExpectation } from "../output/dmxScenario";
import {
	type FixtureDMXExpectation,
	type FixtureDMXTarget,
	FixtureDmxAssertions,
} from "../output/fixtureDmx";
import type { FixtureReference } from "../output/fixtureDmxContract";
import {
	FixtureValueAssertions,
	type FixtureValueExpectation,
} from "../output/fixtureValueScenario";
import {
	BrowserOutputPackets,
	type OutputPacketAssertion,
} from "../output/outputPacketScenario";
import { BrowserOutput } from "../output/outputScenario";
import { BrowserProgrammerActionTiming } from "../performance/programmerActionTiming";
import {
	BrowserCues,
	BrowserRecording,
} from "../playbacks/cuePlaybackScenario";
import { BrowserMoveInBlack } from "../playbacks/moveInBlackScenario";
import { BrowserPages, BrowserPreload } from "../playbacks/pagePreloadScenario";
import { BrowserPlaybackConfiguration } from "../playbacks/playback-configuration/scenario";
import { BrowserPlaybacks } from "../playbacks/playbackScenario";
import { BrowserSpeedGroups } from "../playbacks/speedGroupScenario";
import { BrowserVirtualPlaybacks } from "../playbacks/virtualPlaybackScenario";
import { BrowserHighlight } from "../programmer/highlightScenario";
import { BrowserTiming } from "../programmer/programmerFadeScenario";
import { BrowserProgrammer } from "../programmer/programmerPriority";
import { BrowserProgrammerSpecials } from "../programmer/programmerSpecialScenario";
import { BrowserProductDemo } from "../show/productDemoScenario";
import { BrowserShows } from "../show/showScenario";
import { BrowserPatch } from "../show-setup/patchScenario";
import { BrowserSystemIntegrations } from "../show-setup/systemIntegrationScenario";
import { BrowserFiles } from "../specific-features/fileTextScenario";
import { BrowserDeskLock } from "../window-system/deskLockScenario";
import { BrowserDesktops } from "../window-system/desktopScenario";
import { BrowserOperatorShell } from "../window-system/operatorShellScenario";
import type { BuiltInPaneType } from "../window-system/paneTypes";
import { builtInLabels } from "../window-system/paneTypes";
import type { ApiDriver } from "./api";
import { BrowserClock } from "./clockScenario";
import type { DeskDriver } from "./desk";
import type { LightBench, TestShow } from "./lightBench";
import type { DmxProtocol } from "./protocols";
import { BrowserRecipes } from "./recipeScenario";

export interface ScreenConfigurationIntent {
	name: string;
	desktop?: string;
	fixedPane?: FixedScreenPane["type"];
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
	readonly page: {
		select(number: number): Promise<void>;
		expectSelected(number: number): Promise<void>;
	};
	open(): Promise<void>;
	close(): Promise<void>;
	remove(): Promise<void>;
	expectFixedPane(type: FixedScreenPane["type"]): Promise<void>;
	expectBridgeAction(type: ControllableDesktopAction["type"]): Promise<void>;
}

export class BrowserScenarioWorld {
	readonly desktop: BrowserDesktops;
	readonly operatorShell: BrowserOperatorShell;
	readonly deskLock: BrowserDeskLock;
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
	readonly attachedEncoder: BrowserAttachedEncoders;
	readonly hardware: SimulatedHardware;
	readonly hardwareSimulator: BrowserHardwareSimulator;
	readonly crossSurface: BrowserCrossSurface;
	readonly highlight: BrowserHighlight;
	readonly group: BrowserGroups;
	readonly preset: BrowserPresets;
	readonly demo: BrowserProductDemo;
	readonly record: BrowserRecording;
	readonly cue: BrowserCues;
	readonly playback: BrowserPlaybacks;
	readonly virtualPlayback: BrowserVirtualPlaybacks;
	readonly page: BrowserPages;
	readonly preload: BrowserPreload;
	readonly moveInBlack: BrowserMoveInBlack;
	readonly playbackConfiguration: BrowserPlaybackConfiguration;
	readonly patch: BrowserPatch;
	readonly systemIntegration: BrowserSystemIntegrations;
	readonly dmx: BrowserDmx;
	readonly output: BrowserOutput;
	readonly programmerActionTiming: BrowserProgrammerActionTiming;
	readonly timing: BrowserTiming;
	readonly programmer: BrowserProgrammer;
	readonly speedGroup: BrowserSpeedGroups;
	readonly special: BrowserProgrammerSpecials;
	readonly recipe: BrowserRecipes;
	readonly files: BrowserFiles;
	readonly routeSeed: string;
	private readonly semanticTrace: Array<{
		title: string;
		description: string;
	}> = [];
	private readonly stopObservingSteps: () => void;
	private readonly testInfo: TestInfo;
	private readonly evidencePage: Page;
	private readonly evidenceApi: ApiDriver;
	private readonly evidenceDesk: DeskDriver;
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
	readonly expectFixtureValue: (
		target: FixtureReference,
		expected: FixtureValueExpectation,
	) => Promise<void>;

	constructor(
		page: Page,
		desk: DeskDriver,
		bench: LightBench,
		api: ApiDriver,
		initialShow: TestShow,
		testInfo: TestInfo,
	) {
		this.testInfo = testInfo;
		this.evidencePage = page;
		this.evidenceApi = api;
		this.evidenceDesk = desk;
		const configuredSeed = process.env.LIGHT_TEST_ROUTE_SEED?.trim();
		const baseSeed = configuredSeed || crypto.randomUUID();
		this.routeSeed = [
			baseSeed,
			testInfo.project.name,
			testInfo.title,
			`retry=${testInfo.retry}`,
		].join(":");
		this.stopObservingSteps = desk.observeSemanticSteps((step) =>
			this.semanticTrace.push(step),
		);
		const attach = async (name: string, body: Buffer) => {
			const safeName = artifactName(name);
			await testInfo.attach(safeName, { body, contentType: "image/png" });
		};
		this.app = new BrowserApplication(page, desk, bench.baseUrl);
		this.deskLock = new BrowserDeskLock(api, bench, page, desk);
		this.builtIn = new BrowserBuiltIns(page);
		this.desktop = new BrowserDesktops(page, attach);
		this.operatorShell = new BrowserOperatorShell(api, bench, page, desk);
		this.screenshot = new BrowserScreenshots(page, attach, this.builtIn);
		this.screen = new BrowserScreens(page, desk, api);
		this.show = new BrowserShows(api, bench, desk, initialShow, page);
		this.clock = new BrowserClock(bench, desk);
		this.command = new BrowserCommands(api, desk, page);
		this.keypad = new BrowserKeypad(desk, page);
		this.hardware = simulatedHardware(bench, api);
		this.hardwareSimulator = new BrowserHardwareSimulator(page);
		this.crossSurface = new BrowserCrossSurface(
			api,
			bench,
			page,
			desk,
			this.command,
			() => this.show.contractIdentity().workingId,
		);
		const coreSelection = new BrowserSelection(api);
		this.selection = new BrowserRoutedSelection(
			coreSelection,
			api,
			this.command,
			this.keypad,
			this.hardware,
			page,
			desk,
			`${this.routeSeed}:selection`,
		);
		this.encoder = new BrowserEncoders(
			api,
			coreSelection,
			page,
			desk,
			this.hardware,
			`${this.routeSeed}:encoder`,
		);
		this.attachedEncoder = new BrowserAttachedEncoders(api, bench, page);
		this.highlight = new BrowserHighlight(page, api, this.hardware);
		this.group = new BrowserGroups(
			api,
			page,
			desk,
			this.command,
			coreSelection,
			this.hardware,
			() => this.show.contractIdentity().workingId,
			`${this.routeSeed}:group`,
		);
		this.preset = new BrowserPresets(
			api,
			page,
			desk,
			this.command,
			this.hardware,
			() => this.show.contractIdentity().workingId,
			`${this.routeSeed}:preset`,
		);
		this.demo = new BrowserProductDemo(page, desk, bench, api, testInfo);
		this.record = new BrowserRecording(
			api,
			page,
			desk,
			this.command,
			() => this.show.contractIdentity().workingId,
		);
		this.cue = new BrowserCues(
			api,
			this.command,
			page,
			desk,
			() => this.show.contractIdentity().workingId,
		);
		this.playback = new BrowserPlaybacks(
			api,
			page,
			desk,
			this.hardware,
			() => this.show.contractIdentity().workingId,
		);
		this.virtualPlayback = new BrowserVirtualPlaybacks(
			api,
			page,
			desk,
			this.desktop,
			this.hardware,
			() => this.show.contractIdentity().workingId,
		);
		this.page = new BrowserPages(
			api,
			page,
			desk,
			() => this.show.contractIdentity().workingId,
		);
		this.preload = new BrowserPreload(
			api,
			page,
			desk,
			() => this.show.contractIdentity().workingId,
			this.playback,
		);
		this.moveInBlack = new BrowserMoveInBlack(
			api,
			page,
			desk,
			() => this.show.contractIdentity().workingId,
		);
		this.playbackConfiguration = new BrowserPlaybackConfiguration(
			api,
			page,
			desk,
			() => this.show.contractIdentity().workingId,
		);
		this.patch = new BrowserPatch(api, page, desk);
		this.systemIntegration = new BrowserSystemIntegrations(api, page);
		this.dmx = new BrowserDmx(api);
		this.output = new BrowserOutput(api, desk, page);
		this.programmerActionTiming = new BrowserProgrammerActionTiming(
			api,
			bench,
			page,
		);
		this.programmer = new BrowserProgrammer(api);
		this.special = new BrowserProgrammerSpecials(
			api,
			page,
			desk,
			coreSelection,
			this.hardware,
			() => this.show.contractIdentity().workingId,
		);
		this.timing = new BrowserTiming(
			api,
			page,
			desk,
			this.hardware,
			`${this.routeSeed}:timing`,
		);
		this.speedGroup = new BrowserSpeedGroups(
			api,
			page,
			desk,
			bench,
			`${this.routeSeed}:speed-group`,
		);
		this.recipe = new BrowserRecipes(this, (observer) =>
			desk.observeSemanticSteps(observer),
		);
		this.files = new BrowserFiles(api, page, desk, bench);
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
		const fixtureValues = new FixtureValueAssertions(api);
		this.expectFixtureValue = (target, expected) =>
			fixtureValues.expect(target, expected);
	}

	async finish(failure?: unknown): Promise<void> {
		this.stopObservingSteps();
		await this.output.network.close();
		const routes = {
			seed: this.routeSeed,
			selection: this.selection.routeReports,
			encoder: this.encoder.routeReports,
			group: this.group.routeReports,
			preset: this.preset.routeReports,
			programmerFade: this.timing.programmerFade.routeReports,
			speedGroup: this.speedGroup.reports,
			recipes: this.recipe.reports,
		};
		await this.testInfo.attach("bench-route-report.json", {
			body: Buffer.from(JSON.stringify(routes, null, 2)),
			contentType: "application/json",
		});
		console.info(
			`[bench routes] seed=${this.routeSeed} actions=${
				this.selection.routeReports.length +
				this.encoder.routeReports.length +
				this.group.routeReports.length +
				this.preset.routeReports.length
			}`,
		);
		if (failure === undefined) return;
		const evidence: Record<string, unknown> = {
			seed: this.routeSeed,
			error:
				failure instanceof Error
					? {
							name: failure.name,
							message: failure.message,
							stack: failure.stack,
						}
					: String(failure),
			semanticActions: this.semanticTrace,
		};
		try {
			evidence.show = this.show.contractIdentity();
		} catch (reason) {
			evidence.show = { unavailable: String(reason) };
		}
		try {
			evidence.desktop = await this.evidenceDesk.session();
		} catch (reason) {
			evidence.desktop = { unavailable: String(reason) };
		}
		try {
			evidence.selection = await this.selection.observe();
		} catch (reason) {
			evidence.selection = { unavailable: String(reason) };
		}
		try {
			evidence.programmingInteraction = await this.evidenceApi.request(
				"GET",
				"/api/v2/programming-interaction/snapshot",
			);
		} catch (reason) {
			evidence.programmingInteraction = { unavailable: String(reason) };
		}
		try {
			await this.testInfo.attach("bench-application-failure.png", {
				body: await this.evidencePage.screenshot(),
				contentType: "image/png",
			});
		} catch {
			// Playwright may already have closed the page; its configured failure
			// screenshot remains available in that case.
		}
		await this.testInfo.attach("bench-semantic-failure.json", {
			body: Buffer.from(JSON.stringify(evidence, null, 2)),
			contentType: "application/json",
		});
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
		const toggle = this.page.getByRole("button", {
			name: "Desktops / Built-ins",
			exact: true,
		});
		if ((await toggle.getAttribute("data-dock-mode")) !== "builtins")
			await toggle.click();
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
	readonly page: ScreenPageHandle;

	constructor(
		readonly name: string,
		private readonly runtimeId: string,
		private readonly browser: Page,
		api: ApiDriver,
		private readonly actions: readonly ControllableDesktopAction[],
	) {
		this.page = new ScreenPageHandle(api, runtimeId);
	}

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

	async expectFixedPane(type: FixedScreenPane["type"]): Promise<void> {
		const surface = await this.browser.context().newPage();
		try {
			await surface.goto(
				`${new URL(this.browser.url()).origin}/?screen=${encodeURIComponent(this.runtimeId)}`,
			);
			await expect(
				surface.locator(
					`[data-light-surface="fixed-screen-pane"][data-fixed-pane-type="${type}"]`,
				),
			).toBeVisible();
			await expect(surface.locator(".desk-pane")).toHaveCount(0);
			await expect(
				surface.getByRole("button", { name: /Pane Settings/ }),
			).toHaveCount(0);
		} finally {
			await surface.close();
		}
	}

	async expectBridgeAction(
		type: ControllableDesktopAction["type"],
	): Promise<void> {
		await expect
			.poll(() => this.actions.some((action) => action.type === type))
			.toBe(true);
	}

	private card(): Locator {
		return this.browser.locator(`[data-screen-id="${this.runtimeId}"]`);
	}
}

class ScreenPageHandle {
	constructor(
		private readonly api: ApiDriver,
		private readonly screenId: string,
	) {}

	async select(number: number): Promise<void> {
		if (!Number.isSafeInteger(number) || number < 1)
			throw new Error("Playback Page numbers start at 1");
		await this.api.request("POST", "/api/v2/screens/actions", {
			request_id: crypto.randomUUID(),
			action: { type: "set_page", screen_id: this.screenId, page: number },
		});
		await this.expectSelected(number);
	}

	async expectSelected(number: number): Promise<void> {
		await expect
			.poll(async () => {
				const snapshot = await this.api.request<{
					active_pages: Record<string, number>;
				}>("GET", "/api/v2/screens");
				return snapshot.active_pages[this.screenId];
			})
			.toBe(number);
	}
}

export class BrowserScreens {
	constructor(
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly api: ApiDriver,
	) {}

	async create(
		configuration: ScreenConfigurationIntent,
	): Promise<ScreenHandle> {
		const control = await this.desk.enableControllableDesktop();
		if (configuration.display) control.setDisplays([configuration.display]);
		await this.openSetup();
		// Only a real screen card carries a screen identity: the encoder-placement block beside
		// them shares the class without being a screen.
		const cards = this.page.locator(".screen-settings-card[data-screen-id]");
		const before = await cards.count();
		await this.page
			.getByRole("button", { name: "+ Add screen", exact: true })
			.click();
		const card = cards.nth(before);
		await expect(card).toBeVisible();
		const runtimeId = await card.getAttribute("data-screen-id");
		if (!runtimeId) throw new Error("Created screen has no runtime identity");
		await card.getByLabel("Screen name").fill(configuration.name);
		// A screen card is a summary row; everything else lives in its configuration modal, whose
		// Layout tab opens first and whose Placement tab owns the physical display.
		await card
			.getByRole("button", { name: "Configure screen", exact: true })
			.click();
		const settings = this.page.getByRole("dialog", {
			name: `Configure ${configuration.name}`,
		});
		await expect(settings).toBeVisible();
		if (configuration.fixedPane) {
			await chooseOption(this.page, settings, "Content", "Fixed full-screen pane");
			const fixedPaneLabels: Record<FixedScreenPane["type"], string> = {
				fixture_sheet: "Fixture Sheet",
				stage_2d: "Stage - 2D",
				stage_3d: "Stage - 3D",
				cues: "Cues - Cuelist",
				text: "Text",
			};
			await chooseOption(
				this.page,
				settings,
				"Pane",
				fixedPaneLabels[configuration.fixedPane],
			);
		} else if (configuration.desktop)
			await chooseOption(this.page, settings, "Desktop", configuration.desktop);
		await setSwitch(settings, "Dock", configuration.showDock);
		await setSwitch(settings, "Playbacks", configuration.showPlaybacks);
		await setSwitch(settings, "Page controls", configuration.showPageControls);
		await settings.getByRole("tab", { name: "Placement", exact: true }).click();
		if (configuration.display)
			await chooseOption(
				this.page,
				settings,
				"Physical Display",
				configuration.display.name,
			);
		await setSwitch(settings, "Window mode", configuration.fullscreen);
		if (configuration.bounds) {
			for (const [label, value] of [
				["Window X", configuration.bounds.x],
				["Window Y", configuration.bounds.y],
				["Window width", configuration.bounds.width],
				["Window height", configuration.bounds.height],
			] as const) {
				await settings.getByLabel(label).fill(String(value));
				await settings.getByLabel(label).blur();
			}
		}
		if (configuration.playbacks) {
			await settings
				.getByRole("tab", { name: "Playbacks", exact: true })
				.click();
			await this.configurePlaybacks(settings, runtimeId, configuration.playbacks);
		}
		await settings.getByRole("button", { name: /^Close/ }).first().click();
		await expect(settings).toBeHidden();
		const handle = new BrowserScreenHandle(
			configuration.name,
			runtimeId,
			this.page,
			this.api,
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
		screenId: string,
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
		await expect(dialog).toBeHidden();
		await expect
			.poll(async () => {
				const snapshot = await this.api.request<{
					screens: Array<{ id: string; page_mode: string }>;
				}>("GET", "/api/v2/screens");
				return snapshot.screens.find((screen) => screen.id === screenId)
					?.page_mode;
			})
			.toBe(playback.pageMode === "dedicated" ? "independent" : "follow_main");
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
