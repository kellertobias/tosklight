import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import resolver from "../../../../tools/artifact-paths.cjs";

const { artifactPaths, repositoryRoot } = resolver;
const storyIndex = JSON.parse(
	readFileSync(`${artifactPaths.storybook}/index.json`, "utf8"),
) as {
	entries: Record<
		string,
		{ id: string; name: string; title: string; type: string }
	>;
};
const stories = Object.values(storyIndex.entries)
	.filter((entry) => entry.type === "story")
	.sort((left, right) => left.id.localeCompare(right.id));
const storyIds = new Set(stories.map((entry) => entry.id));
const docs = Object.values(storyIndex.entries).filter(
	(entry) => entry.type === "docs",
);
const applicationBackground = readFileSync(
	`${repositoryRoot}/apps/light-desktop/src/styles/foundation.css`,
	"utf8",
).match(/--bg:\s*(#[0-9a-f]{6})/iu)?.[1];
const packageBackground = readFileSync(
	`${repositoryRoot}/apps/ui-library/src/styles/tokens.css`,
	"utf8",
).match(/--bg:\s*(#[0-9a-f]{6})/iu)?.[1];
const publicComponentStoryCoverage: Record<string, string> = {
	Button: "tosklight-controls-button--primary",
	CommandLine: "tosklight-command-line--interactive",
	CommandSection: "tosklight-command-section--configurable",
	ProgrammerKeypadView: "tosklight-command-section--programmer-software",
	PlaybackToolsView: "tosklight-command-section--playbacks-software",
	HardwareControlSummaryView:
		"tosklight-command-section--playbacks-hardware-connected",
	FormLayout: "tosklight-controls-form-layout--primary",
	FormField: "tosklight-controls-form-layout--primary",
	Field: "tosklight-controls-form-layout--primary",
	TextInput: "tosklight-controls-text-input--primary",
	TextField: "tosklight-controls-text-input--primary",
	NumberInput: "tosklight-controls-number-input--primary",
	NumberField: "tosklight-controls-number-input--primary",
	Input: "tosklight-controls-number-input--primary",
	TextArea: "tosklight-controls-multiline-text-input--primary",
	LargeTextInput: "tosklight-controls-multiline-text-input--primary",
	TextAreaField: "tosklight-controls-multiline-text-input--primary",
	LargeTextField: "tosklight-controls-multiline-text-input--primary",
	MultiValueToggle: "tosklight-controls-multi-value-toggle--primary",
	MultiValueToggleField: "tosklight-controls-multi-value-toggle--primary",
	CyclingValueToggle: "tosklight-controls-cycling-value-toggle--primary",
	CyclingValueToggleField: "tosklight-controls-cycling-value-toggle--primary",
	SelectField: "tosklight-controls-select--primary",
	Select: "tosklight-controls-select--primary",
	CheckboxField: "tosklight-controls-checkbox--primary",
	RadioField: "tosklight-controls-radio--primary",
	SwitchField: "tosklight-controls-switch--primary",
	IconPickerField: "tosklight-controls-icon-picker-field--primary",
	ColorPickerField: "tosklight-controls-color-picker-field--primary",
	FileDropField: "tosklight-controls-file-drop-field--primary",
	GroupedSelectionField: "tosklight-controls-grouped-selection-field--primary",
	SearchBar: "tosklight-controls-search-bar--primary",
	TouchSelect: "tosklight-controls-touch-select--primary",
	HorizontalFader: "tosklight-controls-horizontal-fader--primary",
	HorizontalFaderField: "tosklight-controls-horizontal-fader--primary",
	HorizontalTouchFader: "tosklight-controls-horizontal-fader--primary",
	InputModal: "tosklight-controls-text-input--primary",
	ModalNumberInput: "tosklight-controls-number-input--primary",
	ModalNumberValue: "tosklight-controls-number-input--primary",
	ModalNumberEditor: "tosklight-controls-number-input--primary",
	ModalTextKeyboard: "tosklight-controls-text-input--primary",
	ModalCaretValue: "tosklight-controls-text-input--primary",
	ModalPortal: "tosklight-window-system-modal-layer--portal-primitive",
	ModalTitleBar: "tosklight-window-system-modal-layer--title-bar-configuration",
	TitleChrome: "tosklight-window-system-title-chrome--window-chrome",
	ModalProvider: "tosklight-window-system-modal-layer--three-deep",
	ModalLayer: "tosklight-window-system-modal-layer--close-policies",
	ModalFrame: "tosklight-window-system-modal-layer--close-policies",
	ModalRegistration:
		"tosklight-window-system-modal-layer--application-registration",
	WindowDropdown: "tosklight-window-system-window-dropdown--primary",
	WindowHeader: "tosklight-window-system-window-header--primary",
	WindowSettings: "tosklight-window-system-window-settings--modal-and-anchored",
	WindowFrame: "tosklight-window-system-window-frame--primary",
	WindowScrollArea:
		"tosklight-window-system-window-scroll-area--populated-and-empty",
	DataTable: "tosklight-tables-data-table--primary",
	ButtonGrid: "tosklight-window-system-button-grid--primary",
	GridButton: "tosklight-window-system-button-grid--primary",
	SelectionList: "tosklight-window-system-selection-list--primary",
	SelectionTree: "tosklight-window-system-selection-tree--primary",
	FixtureSheetTableView: "tosklight-tables-fixture-sheet-table--step-selection",
	VerticalTouchFaderSurface: "controls-faders-vertical-touch-fader--software",
	TouchValueButton: "controls-faders-vertical-touch-fader--direct-value-button",
	FaderView: "controls-faders-vertical-touch-fader--fader-view-composition",
	TouchEncoder: "controls-encoders--individual-touch",
	HardwareEncoderDisplayView: "controls-encoders--individual-hardware",
	EncoderSection: "controls-encoders--configurable-family",
	GridDesktop: "window-system-desktop-grid-manager--constrained-placement",
	PaneView: "window-system-desktop-grid-manager--drag-and-resize",
	TouchPlaybackCardView: "controls-playbacks--configurable-playback",
	HardwarePlaybackCardView: "controls-playbacks--eight-by-two-hardware-bank",
	HardwareCueRowsView: "controls-playbacks--eight-by-two-hardware-bank",
	HardwarePlaybackFaderView: "controls-playbacks--eight-by-two-hardware-bank",
	PlaybackBankView: "controls-playbacks--eight-by-two-touch-bank",
	VirtualPlaybackGridView:
		"tables-and-grids-virtual-playback-grid--sparse-grid",
	PoolCard:
		"tables-and-grids-pools-production-pool-cards--scaling-and-every-state",
	PoolGrid: "tables-and-grids-pools-generic-pool-window--sparse",
	PoolWindow: "tables-and-grids-pools-generic-pool-window--sparse",
};

test.describe.configure({ mode: "serial" });

async function poolGeometry(
	page: import("@playwright/test").Page,
	cardSelector: string,
) {
	return page.locator(cardSelector).evaluateAll((cards) =>
		cards
			.filter((_, index) => index < 16 || index === cards.length - 1)
			.map((card) => {
				const bounds = card.getBoundingClientRect();
				return { width: bounds.width, height: bounds.height };
			}),
	);
}

async function expectSquarePool(
	page: import("@playwright/test").Page,
	cardSelector = ".pool-card",
) {
	await expect(page.locator(cardSelector).first()).toBeVisible();
	const geometry = await poolGeometry(page, cardSelector);
	expect(geometry.length).toBeGreaterThan(0);
	for (const bounds of geometry) {
		expect(Math.abs(bounds.width - bounds.height)).toBeLessThanOrEqual(1);
	}
	return geometry[0];
}

for (const story of stories) {
	test(`${story.title} / ${story.name} renders deterministically`, async ({
		page,
	}) => {
		const errors: string[] = [];
		const forbiddenRequests: string[] = [];
		page.on("console", (message) => {
			if (message.type() === "error") {
				const location = message.location();
				errors.push(
					`${message.text()} @ ${location.url}:${location.lineNumber}`,
				);
			}
		});
		page.on("pageerror", (error) => errors.push(error.message));
		page.on("response", (response) => {
			if (response.status() >= 400) {
				errors.push(`${response.status()} ${response.url()}`);
			}
		});
		page.on("request", (request) => {
			const url = request.url();
			if (/\/api(?:\/|\?|$)/u.test(url) || /^wss?:/u.test(url))
				forbiddenRequests.push(url);
		});

		await page.goto(
			`/iframe.html?id=${encodeURIComponent(story.id)}&viewMode=story`,
		);
		await page.evaluate(() => document.fonts.ready);
		const shot = page.locator("[data-documentation-shot]");
		await expect(shot).toHaveAttribute("data-documentation-ready", "true");
		await expect(shot).toBeVisible();
		const bounds = await shot.boundingBox();
		expect(bounds?.width).toBeGreaterThan(100);
		expect(bounds?.height).toBeGreaterThan(100);
		expect(await shot.locator("*").count()).toBeGreaterThan(0);
		expect((await shot.screenshot()).byteLength).toBeGreaterThan(2_000);
		await expect(page.locator("body")).toHaveCSS(
			"background-color",
			"rgb(7, 9, 12)",
		);
		await expect(shot).toHaveCSS("background-color", "rgb(7, 9, 12)");
		expect(forbiddenRequests).toEqual([]);
		expect(errors).toEqual([]);
	});
}

test("Media pane follows the three-column pool and settings contract", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1280, height: 720 });
	await page.goto(
		"/iframe.html?id=tosklight-windows-media-pane--full-built-in&viewMode=story",
	);
	await page.evaluate(() => document.fonts.ready);

	const layers = page.getByRole("list", { name: "Media layers" });
	await expect(layers.getByRole("button")).toHaveCount(8);
	await expect(
		page.getByRole("button", { name: "Master output" }),
	).toBeVisible();
	const masterBounds = await page
		.getByRole("button", { name: "Master output" })
		.boundingBox();
	expect((masterBounds?.width ?? 0) / (masterBounds?.height ?? 1)).toBeCloseTo(
		16 / 9,
		1,
	);
	const layerPreviewBounds = await layers
		.locator(".media-layer-thumbnail")
		.first()
		.boundingBox();
	expect(
		(layerPreviewBounds?.width ?? 0) / (layerPreviewBounds?.height ?? 1),
	).toBeCloseTo(16 / 9, 1);
	await page.getByRole("button", { name: "Master output" }).click();
	await expect(
		page.getByRole("button", { name: "Master output selected" }),
	).toHaveAttribute("aria-pressed", "true");

	const folderPool = page.getByRole("region", { name: "Media folders" });
	await expect(folderPool.locator(".pool-window-grid")).toHaveCount(1);
	await expect(folderPool.locator(".pool-card")).toHaveCount(10);
	await expect(folderPool.locator(".pool-card.selected")).toHaveAttribute(
		"data-pool-slot-id",
		"folder-city",
	);
	const folderOverflow = await folderPool.evaluate((element) => ({
		clientWidth: element.clientWidth,
		scrollWidth: element.scrollWidth,
		maskImage: getComputedStyle(element).maskImage,
	}));
	expect(folderOverflow.scrollWidth).toBeGreaterThan(
		folderOverflow.clientWidth,
	);
	expect(folderOverflow.maskImage).not.toBe("none");
	await folderPool.evaluate((element) => {
		element.scrollLeft = element.scrollWidth;
		element.dispatchEvent(new Event("scroll"));
	});
	await expect(folderPool).toHaveClass(/fade-left-none/u);

	await folderPool.locator('[data-pool-slot-id="folder-tour"]').click();
	await expect(folderPool.locator(".pool-card.selected")).toHaveAttribute(
		"data-pool-slot-id",
		"folder-tour",
	);
	const files = page.getByRole("region", { name: "Media files" });
	await expect(files.locator(".pool-window-grid")).toHaveCount(1);
	await expect(page.getByText("Media File", { exact: true })).toBeVisible();
	await expect(files.locator(".pool-card")).toHaveCount(40);
	await expect(files.locator(".pool-card.empty")).toHaveCount(38);
	await expect(files.locator(".pool-card.selected")).toHaveAttribute(
		"data-pool-slot-id",
		"file-tour-titles",
	);
	await expect(files.getByText("001", { exact: true })).toBeVisible();
	await expect(files.getByText("Tour Titles", { exact: true })).toBeVisible();
	await expect(files.locator("img").first()).toHaveCSS(
		"object-position",
		"50% 100%",
	);
	const mediaCardBounds = await files
		.locator(".pool-card")
		.first()
		.boundingBox();
	const mediaImageBounds = await files
		.locator(".pool-card-media")
		.first()
		.boundingBox();
	expect(
		(mediaImageBounds?.x ?? 0) - (mediaCardBounds?.x ?? 0),
	).toBeLessThanOrEqual(2);
	expect(
		(mediaCardBounds?.y ?? 0) +
			(mediaCardBounds?.height ?? 0) -
			((mediaImageBounds?.y ?? 0) + (mediaImageBounds?.height ?? 0)),
	).toBeLessThanOrEqual(2);
	await expect(page.getByText("BROWSING DRAFT", { exact: true })).toHaveCount(
		0,
	);

	await expect(
		page.getByRole("radiogroup", { name: "Media control section" }),
	).toBeVisible();
	await page.getByRole("radio", { name: "Position" }).click();
	await expect(page.getByRole("slider", { name: "X position" })).toBeVisible();
	await page.getByRole("radio", { name: "Frame" }).click();
	await expect(page.getByRole("slider", { name: "Keystone" })).toBeVisible();
	await page.getByRole("radio", { name: "Effects" }).click();
	await expect(page.getByRole("slider", { name: "Amount" })).toBeVisible();

	await page.getByRole("button", { name: "Settings" }).click();
	const settings = page.getByRole("dialog", { name: "Media pane settings" });
	await expect(settings.getByRole("switch")).toHaveCount(1);
	await settings.getByText("Visible", { exact: true }).click();
	await expect(
		page.getByRole("region", { name: "Media secondary controls" }),
	).toHaveCount(0);
	await expect(
		page.getByRole("radiogroup", { name: "Media control section" }),
	).toHaveCount(0);
	await expect(
		page.getByRole("radiogroup", { name: "Content or Mask browser" }),
	).toHaveCount(0);
	const unifiedSections = page.getByRole("radiogroup", {
		name: "Media window section",
	});
	await expect(unifiedSections).toBeVisible();
	await expect(unifiedSections.getByRole("radio")).toHaveCount(6);
	await expect(
		page.getByRole("button", { name: "Settings", exact: true }),
	).toBeVisible();
	await page.getByRole("button", { name: "Close settings" }).click();
	await unifiedSections.getByRole("radio", { name: "Position" }).click();
	await expect(
		page.getByRole("region", { name: "Media library browser" }),
	).toHaveCount(0);
	await expect(
		page.getByRole("region", { name: "Media secondary controls" }),
	).toBeVisible();
	await expect(page.getByRole("slider", { name: "X position" })).toBeVisible();
	await unifiedSections.getByRole("radio", { name: "Content" }).click();
	await expect(
		page.getByRole("region", { name: "Media library browser" }),
	).toBeVisible();
	await expect(
		page.getByRole("region", { name: "Media secondary controls" }),
	).toHaveCount(0);
});

test("Media desk preview uses the regular Media encoders", async ({ page }) => {
	await page.setViewportSize({ width: 1280, height: 720 });
	await page.goto(
		"/iframe.html?id=tosklight-windows-media-pane--full-desk-preview&viewMode=story",
	);
	await page.evaluate(() => document.fonts.ready);

	await expect(
		page.getByRole("application", { name: "ToskLight application" }),
	).toBeVisible();
	const applicationBounds = await page
		.getByRole("application", { name: "ToskLight application" })
		.boundingBox();
	expect(applicationBounds?.height).toBeGreaterThanOrEqual(860);
	const header = page.locator(".media-pane-surface > .ui-window-header");
	const headerTitleBounds = await header
		.locator(".ui-window-title")
		.boundingBox();
	const headerToolsBounds = await header
		.locator(".media-pane-header-tools")
		.boundingBox();
	expect(headerToolsBounds?.x ?? 0).toBeGreaterThan(
		(headerTitleBounds?.x ?? 0) + (headerTitleBounds?.width ?? 0) + 100,
	);
	await expect(
		header.getByText("DUMMY · LOCAL ONLY", { exact: true }),
	).toHaveCount(0);
	const firstLayer = page
		.getByRole("list", { name: "Media layers" })
		.getByRole("button")
		.first();
	await expect(
		firstLayer.locator(".media-layer-title > span").first(),
	).toHaveText("Layer 1");
	await expect(firstLayer.locator(".media-layer-name")).toHaveText("Main");
	const layerThumbnailBounds = await firstLayer
		.locator(".media-layer-thumbnail")
		.boundingBox();
	const layerCopyBounds = await firstLayer
		.locator(".media-layer-copy")
		.boundingBox();
	expect(layerCopyBounds?.x ?? 0).toBeGreaterThanOrEqual(
		(layerThumbnailBounds?.x ?? 0) + (layerThumbnailBounds?.width ?? 0),
	);
	await expect(
		page.getByRole("button", { name: "Media", exact: true }),
	).toBeVisible();
	await expect(
		page.getByRole("group", { name: "Enc 1 · Media Folder" }),
	).toBeVisible();
	await expect(
		page.getByRole("group", { name: "Enc 2 · Media File" }),
	).toBeVisible();

	await page
		.getByRole("button", { name: "Set Enc 1 · Media Folder value" })
		.click();
	await page.getByRole("slider", { name: "Enc 1 · Media Folder" }).fill("11");
	await expect(
		page.locator('[aria-label="Media folders"] .pool-card.selected'),
	).toHaveAttribute("data-pool-slot-id", "folder-tour");
	await page
		.getByRole("button", { name: "Close Enc 1 · Media Folder value" })
		.click();
	await page.getByRole("button", { name: "Save changes" }).click();

	await page
		.getByRole("button", { name: "Set Enc 2 · Media File value" })
		.click();
	await page.getByRole("slider", { name: "Enc 2 · Media File" }).fill("100");
	await expect(
		page.locator('[aria-label="Media files"] .pool-card.selected'),
	).toHaveAttribute("data-pool-slot-id", "file-night-forest");
});

test("Media layer titles stay visible in every primary composition", async ({
	page,
}) => {
	for (const story of [
		"full-built-in",
		"configurable-desktop-pane",
		"full-desk-preview",
	]) {
		await page.goto(
			`/iframe.html?id=tosklight-windows-media-pane--${story}&viewMode=story`,
		);
		await page.evaluate(() => document.fonts.ready);
		const titles = page.locator(".media-layer-title");
		await expect(titles).toHaveCount(8);
		for (let index = 0; index < 8; index += 1) {
			const title = titles.nth(index);
			await expect(title.locator("span").first()).toHaveText(
				`Layer ${index + 1}`,
			);
			await expect(title.locator(".media-layer-name")).not.toBeEmpty();
			const bounds = await title.boundingBox();
			expect(bounds?.width).toBeGreaterThan(30);
			expect(bounds?.height).toBeGreaterThan(8);
		}
	}

	await page.setViewportSize({ width: 1280, height: 720 });
	await page.goto(
		"/iframe.html?id=tosklight-windows-media-pane--full-desk-preview&viewMode=story",
	);
	await page.evaluate(() => document.fonts.ready);
	const compactFirstLayer = page
		.getByRole("list", { name: "Media layers" })
		.getByRole("button")
		.first();
	const compactThumbnailBounds = await compactFirstLayer
		.locator(".media-layer-thumbnail")
		.boundingBox();
	expect(
		(compactThumbnailBounds?.width ?? 0) /
			(compactThumbnailBounds?.height ?? 1),
	).toBeCloseTo(16 / 9, 2);
	await expect(
		compactFirstLayer.locator(".media-layer-title > span").first(),
	).toHaveText("Layer 1");
	await expect(compactFirstLayer.locator(".media-layer-source")).toBeVisible();
	await expect(compactFirstLayer.locator(".media-layer-opacity")).toBeVisible();
	await expect(compactFirstLayer.locator(".media-layer-name")).toBeHidden();
	await expect(compactFirstLayer.locator(".media-layer-mask")).toBeHidden();
	await expect(compactFirstLayer.locator(".media-layer-color")).toBeHidden();
	await expect(compactFirstLayer.locator(".media-layer-effect")).toBeHidden();

	await page.setViewportSize({ width: 1440, height: 1080 });
	await page.goto(
		"/iframe.html?id=tosklight-windows-media-pane--full-desk-preview&viewMode=story",
	);
	await page.evaluate(() => document.fonts.ready);
	const tallFirstLayer = page
		.getByRole("list", { name: "Media layers" })
		.getByRole("button")
		.first();
	const tallThumbnailBounds = await tallFirstLayer
		.locator(".media-layer-thumbnail")
		.boundingBox();
	const tallCopyBounds = await tallFirstLayer
		.locator(".media-layer-copy")
		.boundingBox();
	const tallStatusBounds = await tallFirstLayer
		.locator(":scope > i")
		.boundingBox();
	expect(
		(tallThumbnailBounds?.width ?? 0) / (tallThumbnailBounds?.height ?? 1),
	).toBeCloseTo(16 / 9, 2);
	expect(tallThumbnailBounds?.width).toBeLessThanOrEqual(116);
	expect(tallCopyBounds?.x ?? 0).toBeGreaterThan(
		(tallThumbnailBounds?.x ?? 0) + (tallThumbnailBounds?.width ?? 0),
	);
	expect(tallCopyBounds?.width).toBeGreaterThan(80);
	expect(
		(tallCopyBounds?.x ?? 0) + (tallCopyBounds?.width ?? 0),
	).toBeLessThanOrEqual(tallStatusBounds?.x ?? 0);
});

test("Dynamics lane layout preserves full-width geometry and isolated interactions", async ({
	page,
}, testInfo) => {
	test.setTimeout(60_000);
	const artifactDirectory = `${artifactPaths.visual}/dynamics-lane-regression`;
	mkdirSync(artifactDirectory, { recursive: true });
	const measurements: Array<Record<string, unknown>> = [];
	const storyUrl =
		"/iframe.html?id=tosklight-windows-dynamics--full-application-discussion&viewMode=story";
	const viewports = [
		{ width: 1280, height: 720, mode: "software" },
		{ width: 1920, height: 1080, mode: "software" },
		{ width: 900, height: 720, mode: "software" },
		{ width: 1280, height: 720, mode: "hardware" },
		{ width: 1920, height: 1080, mode: "hardware" },
	] as const;

	for (const viewport of viewports) {
		await page.setViewportSize(viewport);
		await page.goto(`${storyUrl}&globals=mode:${viewport.mode}`);
		await page.evaluate(() => document.fonts.ready);
		const shot = page.locator("[data-documentation-shot]");
		await expect(shot).toHaveAttribute("data-documentation-ready", "true");
		await page.addStyleTag({
			content:
				"*,*::before,*::after{animation:none!important;caret-color:transparent!important;transition:none!important}",
		});

		const geometry = await page
			.locator(".dynamic-lane-overview")
			.evaluateAll((rows) =>
				rows.map((row) => {
					const bounds = (selector: string) => {
						const element =
							selector === ":scope" ? row : row.querySelector(selector);
						if (!(element instanceof HTMLElement))
							throw new Error(`Missing ${selector}`);
						const box = element.getBoundingClientRect();
						return {
							x: box.x,
							y: box.y,
							width: box.width,
							height: box.height,
							right: box.right,
						};
					};
					return {
						row: bounds(":scope"),
						content: bounds(".dynamic-lane-content"),
						identity: bounds(".dynamic-lane-identity-select"),
						curve: bounds(".dynamic-lane-curve"),
						curveSelect: bounds(".dynamic-lane-curve-select"),
						action: bounds(".dynamic-lane-row-actions"),
					};
				}),
			);
		expect(geometry).toHaveLength(3);
		expect(
			await page.locator(".dynamic-lane-overview button button").count(),
		).toBe(0);

		const list = await page
			.locator(".dynamic-lane-overview-list")
			.boundingBox();
		expect(list).not.toBeNull();
		const listOverflow = await page
			.locator(".dynamic-lane-overview-list")
			.evaluate((element) => ({
				clientHeight: element.clientHeight,
				scrollHeight: element.scrollHeight,
			}));
		measurements.push({ viewport, list, lanes: geometry });
		for (const lane of geometry) {
			expect(lane.row.height).toBeGreaterThanOrEqual(120);
			expect(lane.row.width).toBeGreaterThan((list?.width ?? 0) - 20);
			expect(
				Math.abs(lane.content.width + lane.action.width - lane.row.width),
			).toBeLessThanOrEqual(3);
			expect(
				Math.abs(lane.identity.width + lane.curve.width - lane.content.width),
			).toBeLessThanOrEqual(1);
			expect(lane.curve.width).toBeGreaterThan(190);
			expect(lane.curve.width).toBeGreaterThan(lane.identity.width * 2);
			expect(lane.curveSelect.width).toBeCloseTo(lane.curve.width, 0);
			expect(lane.curveSelect.height).toBeCloseTo(lane.curve.height, 0);
			expect(lane.action.right).toBeLessThanOrEqual(viewport.width);
		}
		expect(geometry[1].row.height).toBeCloseTo(geometry[0].row.height, 0);
		expect(geometry[2].row.height).toBeCloseTo(geometry[0].row.height, 0);
		if (listOverflow.scrollHeight === listOverflow.clientHeight)
			expect(
				geometry.reduce((sum, lane) => sum + lane.row.height, 0),
			).toBeGreaterThan((list?.height ?? 0) - 24);
		for (const key of ["identity", "curve", "action"] as const) {
			expect(geometry[1][key].x).toBeCloseTo(geometry[0][key].x, 0);
			expect(geometry[2][key].x).toBeCloseTo(geometry[0][key].x, 0);
		}

		const screenshotPath = `${artifactDirectory}/${viewport.mode}-${viewport.width}x${viewport.height}.png`;
		await page.screenshot({ path: screenshotPath });
		await testInfo.attach(
			`Dynamics lanes ${viewport.mode} ${viewport.width}x${viewport.height}`,
			{ path: screenshotPath, contentType: "image/png" },
		);
	}
	writeFileSync(
		`${artifactDirectory}/geometry.json`,
		`${JSON.stringify(measurements, null, 2)}\n`,
	);

	await page.setViewportSize({ width: 1280, height: 720 });
	await page.goto(`${storyUrl}&globals=mode:software`);
	await page.evaluate(() => document.fonts.ready);
	await expect(page.locator("[data-documentation-shot]")).toHaveAttribute(
		"data-documentation-ready",
		"true",
	);

	const intensityIdentity = page.getByRole("button", {
		name: "Select lane 1, Intensity",
	});
	const blueIdentity = page.getByRole("button", {
		name: "Select lane 2, Blue",
	});
	const panCurve = page.getByRole("button", {
		name: "Select Pan lane from curve",
	});
	await blueIdentity.click();
	await expect(intensityIdentity).toHaveAttribute("aria-pressed", "false");
	await expect(blueIdentity).toHaveAttribute("aria-pressed", "true");

	await page.keyboard.down("Shift");
	await panCurve.click();
	await page.keyboard.up("Shift");
	await expect(blueIdentity).toHaveAttribute("aria-pressed", "true");
	await expect(panCurve).toHaveAttribute("aria-pressed", "true");

	const keyframe = page.getByRole("button", {
		name: "Intensity keyframe B",
	});
	const before = await keyframe.boundingBox();
	expect(before).not.toBeNull();
	const beforeLeft = await keyframe.evaluate(
		(element) => (element as HTMLElement).style.left,
	);
	const targetCenterX = (before?.x ?? 0) + (before?.width ?? 0) / 2 + 48;
	await page.mouse.move(
		(before?.x ?? 0) + (before?.width ?? 0) / 2,
		(before?.y ?? 0) + (before?.height ?? 0) / 2,
	);
	await page.mouse.down();
	await page.mouse.move(
		targetCenterX,
		(before?.y ?? 0) + (before?.height ?? 0) / 2,
		{ steps: 8 },
	);
	await page.mouse.up();
	await expect
		.poll(() =>
			keyframe.evaluate((element) => (element as HTMLElement).style.left),
		)
		.not.toBe(beforeLeft);
	await expect
		.poll(async () => {
			const after = await keyframe.boundingBox();
			return (after?.x ?? 0) + (after?.width ?? 0) / 2;
		})
		.toBeCloseTo(targetCenterX, 0);
	await expect(intensityIdentity).toHaveAttribute("aria-pressed", "true");
	await expect(blueIdentity).toHaveAttribute("aria-pressed", "false");
	await expect(panCurve).toHaveAttribute("aria-pressed", "false");

	const loopClose = page.locator(".dynamic-keyframe-marks i.loop-close");
	await expect(loopClose).toHaveCSS("border-top-style", "solid");
	await expect(loopClose).toHaveCSS("border-top-color", "rgb(122, 135, 145)");

	const laneRow = page.locator(".dynamic-lane-overview").first();
	const laneSettings = page.getByRole("button", {
		name: "Intensity lane settings",
	});
	const [closedSettingsBox, closedActionBox] = await Promise.all([
		laneSettings.boundingBox(),
		laneSettings.locator("..").boundingBox(),
	]);
	await expect(laneSettings).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
	await laneSettings.click();
	const laneMenu = page.getByRole("menu", { name: "Intensity lane menu" });
	const closeLaneSettings = page.getByRole("button", {
		name: "Close lane settings",
	});
	const [rowBox, menuBox, closeSettingsBox] = await Promise.all([
		laneRow.boundingBox(),
		laneMenu.boundingBox(),
		closeLaneSettings.boundingBox(),
	]);
	expect(Math.abs((menuBox?.x ?? 0) - (rowBox?.x ?? 0))).toBeLessThanOrEqual(2);
	expect(
		Math.abs((menuBox?.width ?? 0) - (rowBox?.width ?? 0)),
	).toBeLessThanOrEqual(2);
	expect(
		Math.abs((menuBox?.height ?? 0) - (rowBox?.height ?? 0)),
	).toBeLessThanOrEqual(2);
	expect(closeSettingsBox?.width).toBeGreaterThan(
		closedSettingsBox?.width ?? 0,
	);
	expect(closeSettingsBox?.x).toBeLessThan(closedSettingsBox?.x ?? 0);
	expect(closeSettingsBox?.x).toBeLessThan(closedActionBox?.x ?? 0);
	expect(
		Math.abs(
			(closeSettingsBox?.x ?? 0) -
				((rowBox?.x ?? 0) +
					(rowBox?.width ?? 0) -
					(closeSettingsBox?.width ?? 0)),
		),
	).toBeLessThanOrEqual(2);
	expect(
		Math.abs(
			(closeSettingsBox?.x ?? 0) +
				(closeSettingsBox?.width ?? 0) -
				((rowBox?.x ?? 0) + (rowBox?.width ?? 0)),
		),
	).toBeLessThanOrEqual(2);
	await expect(closeLaneSettings).toHaveText("×");
	await expect(closeLaneSettings).toHaveCSS(
		"background-color",
		"rgba(0, 0, 0, 0)",
	);

	await page.setViewportSize({ width: 900, height: 560 });
	await page.goto(`${storyUrl}&globals=mode:software`);
	await page.evaluate(() => document.fonts.ready);
	const compactOverflow = await page
		.locator(".dynamic-lane-overview-list")
		.evaluate((element) => ({
			clientHeight: element.clientHeight,
			scrollHeight: element.scrollHeight,
			rowHeights: Array.from(
				element.querySelectorAll(".dynamic-lane-overview"),
				(row) => row.getBoundingClientRect().height,
			),
		}));
	expect(compactOverflow.rowHeights.every((height) => height >= 120)).toBe(
		true,
	);
	expect(compactOverflow.scrollHeight).toBeGreaterThan(
		compactOverflow.clientHeight,
	);
});

test("Dynamics full application discussion keeps the selection preview across tabs and exposes shared Phase controls", async ({
	page,
}) => {
	const storyUrl =
		"/iframe.html?id=tosklight-windows-dynamics--full-application-discussion&viewMode=story";
	const screenshotPath = `${artifactPaths.visual}/dynamics-phase/full-application-phase-1440x900.png`;
	mkdirSync(`${artifactPaths.visual}/dynamics-phase`, { recursive: true });
	await page.setViewportSize({ width: 1440, height: 900 });
	await page.goto(`${storyUrl}&globals=mode:software`);
	await page.evaluate(() => document.fonts.ready);

	const editor = page.locator(".dynamic-full-discussion-editor");
	const preview = page.getByRole("complementary", {
		name: "Selection preview",
	});
	const fixtureField = page.getByRole("img", {
		name: "Top-down preview of 12 selected fixtures",
	});
	await expect(preview).toBeVisible();
	await expect(fixtureField).toBeVisible();
	await editor.getByRole("button", { name: "+ Add Lane" }).click();
	const laneAttributeDialog = page.getByRole("dialog", {
		name: "Select lane attribute",
	});
	await expect(laneAttributeDialog).toBeVisible();
	await expect(
		laneAttributeDialog.getByText(
			"Choose the attribute controlled by the new lane",
		),
	).toBeVisible();
	await expect(
		laneAttributeDialog.getByRole("button", { name: "Intensity" }),
	).toBeVisible();
	await laneAttributeDialog
		.getByRole("button", { name: "Close modal" })
		.click();
	const [windowBox, headerBox, composerBox, composerControlBox, previewBox] =
		await Promise.all([
			editor.locator(".dynamics-window").boundingBox(),
			editor.locator(".ui-window-header").boundingBox(),
			editor.locator(".dynamic-lane-bottom-editor").boundingBox(),
			editor.locator(".dynamic-curve-method-cycle").boundingBox(),
			preview.boundingBox(),
		]);
	for (const fullWidthBox of [headerBox, composerBox]) {
		expect(
			Math.abs((fullWidthBox?.x ?? 0) - (windowBox?.x ?? 0)),
		).toBeLessThanOrEqual(1);
		expect(
			Math.abs((fullWidthBox?.width ?? 0) - (windowBox?.width ?? 0)),
		).toBeLessThanOrEqual(1);
	}
	expect(previewBox?.y).toBeGreaterThanOrEqual(
		(headerBox?.y ?? 0) + (headerBox?.height ?? 0) - 1,
	);
	expect((previewBox?.y ?? 0) + (previewBox?.height ?? 0)).toBeLessThanOrEqual(
		(composerBox?.y ?? 0) + 1,
	);
	expect(await fixtureField.locator(".dynamic-face-fixture").count()).toBe(12);
	const fieldBox = await fixtureField.boundingBox();
	expect(fieldBox?.width).toBeCloseTo(fieldBox?.height ?? 0, 0);
	await expect(fixtureField).not.toHaveCSS("border-radius", "50%");
	const fixtureStyles = await fixtureField
		.locator(".dynamic-face-fixture")
		.evaluateAll((fixtures) =>
			fixtures.slice(0, 6).map((fixture) => {
				const style = getComputedStyle(fixture);
				return {
					background: style.backgroundColor,
					border: style.borderColor,
					opacity: style.opacity,
					shadow: style.boxShadow,
				};
			}),
		);
	expect(
		new Set(fixtureStyles.map(({ background }) => background)).size,
	).toBeGreaterThan(1);
	expect(
		new Set(fixtureStyles.map(({ opacity }) => opacity)).size,
	).toBeGreaterThan(1);
	expect(fixtureStyles.every(({ shadow }) => shadow === "none")).toBe(true);

	await editor.getByRole("button", { name: "Speed", exact: true }).click();
	await expect(preview).toBeVisible();
	const speedSourceFields = editor.locator(".dynamic-speed-source-fields");
	const [speedSourceBox, speedGroupBox, beatsPerCycleBox] = await Promise.all([
		speedSourceFields.boundingBox(),
		speedSourceFields.locator(".ui-form-field").nth(0).boundingBox(),
		speedSourceFields.locator(".ui-form-field").nth(1).boundingBox(),
	]);
	expect(speedGroupBox?.width).toBeCloseTo(beatsPerCycleBox?.width ?? 0, 0);
	expect(
		(speedGroupBox?.width ?? 0) + (beatsPerCycleBox?.width ?? 0),
	).toBeGreaterThan((speedSourceBox?.width ?? 0) - 12);
	const beatGrid = editor.getByRole("img", {
		name: "Speed transport beat grid",
	});
	const groupTap = editor.getByRole("button", {
		name: "Tap Speed Group A tempo, 120 BPM",
	});
	await expect(beatGrid).toContainText("Phase 38%");
	await expect(groupTap).toBeVisible();
	await expect(groupTap).toContainText("120 BPM");
	await expect(groupTap).toContainText("TAP GROUP A");
	const [beatGridBox, groupTapBox] = await Promise.all([
		beatGrid.boundingBox(),
		groupTap.boundingBox(),
	]);
	const speedSourceToggle = editor.getByRole("radiogroup", {
		name: "Speed source",
	});
	const speedSourceToggleBox = await speedSourceToggle.boundingBox();
	expect(groupTapBox?.width).toBeCloseTo(beatGridBox?.width ?? 0, 0);
	expect(groupTapBox?.height).toBeCloseTo(beatGridBox?.height ?? 0, 0);
	expect(beatGridBox?.y).toBeLessThan(speedSourceToggleBox?.y ?? 0);
	expect(groupTapBox?.y).toBeGreaterThan(
		(beatGridBox?.y ?? 0) + (beatGridBox?.height ?? 0) - 1,
	);
	const firstFixture = fixtureField.locator(".dynamic-face-fixture").first();
	const stoppedOpacity = await firstFixture.evaluate(
		(element) => getComputedStyle(element).opacity,
	);
	await editor.getByRole("button", { name: "▶ Preview", exact: true }).click();
	await expect
		.poll(() =>
			firstFixture.evaluate((element) => getComputedStyle(element).opacity),
		)
		.not.toBe(stoppedOpacity);
	await editor.getByRole("radio", { name: "Fixed BPM", exact: true }).click();
	const fixedTap = editor.getByRole("button", {
		name: "Tap fixed tempo, 120 BPM",
	});
	await expect(fixedTap).toBeVisible();
	await expect(fixedTap).toContainText("120 BPM");
	await expect(fixedTap).toContainText("TAP");
	expect((await fixedTap.boundingBox())?.width).toBeCloseTo(
		(await beatGrid.boundingBox())?.width ?? 0,
		0,
	);
	for (const field of await editor
		.locator(".dynamic-speed-controls .ui-form-field")
		.all()) {
		const [fieldWidth, controlWidth] = await Promise.all([
			field.evaluate((element) => element.getBoundingClientRect().width),
			field
				.locator(".ui-form-control")
				.evaluate((element) => element.getBoundingClientRect().width),
		]);
		expect(controlWidth).toBeCloseTo(fieldWidth, 0);
	}

	await editor.getByRole("button", { name: "Phase", exact: true }).click();
	await expect(preview).toBeVisible();
	await expect(editor.getByText("2D phase distribution")).toHaveCount(0);
	await expect(
		editor.getByText("Fill shows color · outline glow shows intensity"),
	).toHaveCount(0);
	await expect(editor.locator(".dynamic-phase-position-map")).toHaveCount(0);
	await expect(
		editor.getByRole("radiogroup", { name: "Phase mode" }),
	).toHaveCount(0);
	await expect(
		editor
			.locator(".dynamic-phase-controls")
			.getByRole("radiogroup", { name: "Ordering mode" }),
	).toBeHidden();
	await expect(
		editor.getByText(
			"Ordering stays centered · hold or right-click to choose any method",
		),
	).toHaveCount(0);

	const phaseControls = page.getByRole("group", {
		name: "Phase quick controls",
	});
	await expect(phaseControls).toBeVisible();
	await expect(
		phaseControls.getByRole("radiogroup", { name: "Ordering mode" }),
	).toBeVisible();
	await expect(
		phaseControls.getByRole("radio", { name: "Linear", exact: true }),
	).toBeVisible();
	await expect(
		phaseControls.getByRole("radio", { name: "Selection", exact: true }),
	).toHaveCount(0);
	await expect(
		phaseControls.getByText("Ordering mode", { exact: true }),
	).toHaveCount(0);
	await expect(
		phaseControls.getByRole("button", { name: "Take Selection" }),
	).toBeVisible();
	await expect(
		phaseControls.getByRole("button", { name: "Clear Selection" }),
	).toBeVisible();
	const [phaseControlsBox, orderingBox, takeSelectionBox, clearSelectionBox] =
		await Promise.all([
			phaseControls.boundingBox(),
			phaseControls
				.getByRole("radiogroup", { name: "Ordering mode" })
				.boundingBox(),
			phaseControls
				.getByRole("button", { name: "Take Selection" })
				.boundingBox(),
			phaseControls
				.getByRole("button", { name: "Clear Selection" })
				.boundingBox(),
		]);
	expect(phaseControlsBox?.height).toBeCloseTo(composerBox?.height ?? 0, 0);
	expect(orderingBox?.height).toBeCloseTo(composerControlBox?.height ?? 0, 0);
	const orderingCenter = (orderingBox?.y ?? 0) + (orderingBox?.height ?? 0) / 2;
	expect(
		(takeSelectionBox?.x ?? 0) -
			((orderingBox?.x ?? 0) + (orderingBox?.width ?? 0)),
	).toBeGreaterThanOrEqual(8);
	for (const actionBox of [takeSelectionBox, clearSelectionBox]) {
		const actionCenter = (actionBox?.y ?? 0) + (actionBox?.height ?? 0) / 2;
		expect(actionCenter).toBeCloseTo(orderingCenter, 0);
		expect(actionBox?.height).toBeCloseTo(orderingBox?.height ?? 0, 0);
	}
	expect(
		await editor.getByRole("button", { name: "Take Selection" }).count(),
	).toBe(1);
	expect(
		await editor.getByRole("button", { name: "Clear Selection" }).count(),
	).toBe(1);
	for (const label of ["Offset", "Span", "Blocks", "Repeats"]) {
		await expect(
			editor
				.locator(".dynamic-phase-shared-fields")
				.getByLabel(label, { exact: true }),
		).toBeVisible();
		await expect(phaseControls.getByLabel(label, { exact: true })).toHaveCount(
			0,
		);
	}
	await expect(
		editor
			.locator(".dynamic-phase-controls")
			.getByRole("switch", { name: "Wings" }),
	).toBeVisible();
	await expect(
		editor.getByLabel("Explicit anchors", { exact: true }),
	).toBeHidden();
	const [blocksBox, repeatsBox, wingsBox, offsetBox, spanBox] =
		await Promise.all([
			editor.locator(".dynamic-phase-blocks-field").boundingBox(),
			editor.locator(".dynamic-phase-repeats-field").boundingBox(),
			editor.locator(".dynamic-phase-wings-field").boundingBox(),
			editor.locator(".dynamic-phase-offset-field").boundingBox(),
			editor.locator(".dynamic-phase-span-field").boundingBox(),
		]);
	expect(blocksBox?.y).toBeCloseTo(repeatsBox?.y ?? 0, 0);
	expect(blocksBox?.y).toBeCloseTo(wingsBox?.y ?? 0, 0);
	expect(offsetBox?.y).toBeCloseTo(spanBox?.y ?? 0, 0);
	expect(offsetBox?.y ?? 0).toBeGreaterThan(blocksBox?.y ?? 0);
	await expect(
		page.getByRole("combobox", { name: "Dynamic lane" }),
	).toHaveCount(0);
	const radialOrdering = phaseControls.getByRole("radio", {
		name: "Radial",
		exact: true,
	});
	await radialOrdering.click();
	await expect(radialOrdering).toHaveAttribute("aria-checked", "true");
	await expect(radialOrdering).toHaveClass(/is-active/);
	const gridOrdering = phaseControls.getByRole("radio", {
		name: "Grid",
		exact: true,
	});
	await expect(gridOrdering).toHaveAttribute("aria-checked", "false");
	await expect(gridOrdering).not.toHaveClass(/is-active/);
	await expect(editor.getByLabel("Direction", { exact: true })).toHaveCount(0);
	await expect(editor.getByLabel("Center X", { exact: true })).toBeVisible();
	await expect(editor.getByLabel("Center Z", { exact: true })).toBeVisible();
	await expect(
		phaseControls.getByRole("radio", { name: "Radial in", exact: true }),
	).toHaveCount(0);
	await page.screenshot({ path: screenshotPath });

	await page.setViewportSize({ width: 1199, height: 800 });
	await expect(preview).toBeHidden();
});

test("every story family has autodocs source preview", () => {
	const documentedTitles = new Set(docs.map((entry) => entry.title));
	expect(docs.length).toBeGreaterThan(0);
	for (const title of new Set(stories.map((entry) => entry.title))) {
		expect(
			documentedTitles,
			`${title} is missing an autodocs source preview`,
		).toContain(title);
	}
});

test("every public production component has a tracked representative story", () => {
	for (const [component, storyId] of Object.entries(
		publicComponentStoryCoverage,
	)) {
		expect(
			storyIds,
			`${component} is missing representative story ${storyId}`,
		).toContain(storyId);
	}
});

test("component-owned catalog stories keep their documented interactions operable", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-controls-text-input--primary&viewMode=story",
	);
	await page.getByRole("button", { name: "Open keyboard" }).first().click();
	await expect(
		page.getByRole("dialog", { name: "Fixture name" }),
	).toBeVisible();
	await expect(
		page.getByRole("button", { name: "Enter · Confirm" }),
	).toBeVisible();

	await page.goto(
		"/iframe.html?id=tosklight-controls-number-input--primary&viewMode=story",
	);
	await page.getByRole("button", { name: "Open number pad" }).first().click();
	await expect(page.getByRole("dialog", { name: "Level" })).toBeVisible();
	await expect(page.getByRole("button", { name: "7" })).toBeVisible();

	await page.goto(
		"/iframe.html?id=tosklight-controls-checkbox--primary&viewMode=story",
	);
	const checkbox = page.getByRole("checkbox", { name: /Desktop lock/u });
	await expect(checkbox).toBeChecked();
	await checkbox.click();
	await expect(checkbox).not.toBeChecked();

	await page.goto(
		"/iframe.html?id=tosklight-window-system-window-dropdown--primary&viewMode=story",
	);
	await page.getByRole("button", { name: "Stage view" }).click();
	await page.getByRole("menuitem", { name: "3D" }).click();
	await expect(page.getByLabel("Selected stage view")).toHaveText("3D");
});

test("Storybook uses the exact application background token", () => {
	expect(applicationBackground).toBe("#07090c");
	expect(packageBackground).toBe(applicationBackground);
});

test("configured search keeps the standard magnifier size and adds width only for its caret", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-controls-search-bar--primary&viewMode=story",
	);
	const plainIcon = page.locator(
		".console-search:not(.has-options) .console-search-icon > svg",
	);
	const optionsTrigger = page.getByRole("button", { name: "Search settings" });
	const optionsIcon = optionsTrigger.locator(
		":scope > svg:not(.console-search-chevron)",
	);
	const caret = optionsTrigger.locator(".console-search-chevron");
	const [plainIconBox, optionsTriggerBox, optionsIconBox, caretBox] =
		await Promise.all([
			plainIcon.boundingBox(),
			optionsTrigger.boundingBox(),
			optionsIcon.boundingBox(),
			caret.boundingBox(),
		]);
	expect(plainIconBox).not.toBeNull();
	expect(optionsTriggerBox).not.toBeNull();
	expect(optionsIconBox).not.toBeNull();
	expect(caretBox).not.toBeNull();
	expect(optionsIconBox?.width).toBeCloseTo(plainIconBox?.width ?? 18, 0);
	expect(optionsIconBox?.height).toBeCloseTo(plainIconBox?.height ?? 18, 0);
	expect(optionsTriggerBox?.width).toBeGreaterThan(optionsIconBox?.width ?? 18);
	expect(caretBox?.x).toBeGreaterThan(
		(optionsIconBox?.x ?? 0) + (optionsIconBox?.width ?? 0),
	);
	expect(
		(caretBox?.x ?? 0) -
			((optionsIconBox?.x ?? 0) + (optionsIconBox?.width ?? 0)),
	).toBeLessThanOrEqual(3);
	const combinedCenter =
		((optionsIconBox?.x ?? 0) + (caretBox?.x ?? 0) + (caretBox?.width ?? 0)) /
		2;
	expect(combinedCenter).toBeCloseTo(
		(optionsTriggerBox?.x ?? 0) + (optionsTriggerBox?.width ?? 0) / 2,
		0,
	);
	const plainSearch = page.locator(".console-search:not(.has-options)");
	for (const [buttonName, iconSelector] of [
		["Clear search", ".ui-input-clear-icon"],
		["Open keyboard", ".ui-keyboard-icon"],
	] as const) {
		const button = plainSearch.getByRole("button", { name: buttonName });
		const icon = button.locator(iconSelector);
		const [buttonBox, iconBox] = await Promise.all([
			button.boundingBox(),
			icon.boundingBox(),
		]);
		expect(buttonBox).not.toBeNull();
		expect(iconBox).not.toBeNull();
		expect(
			Math.abs(
				(iconBox?.x ?? 0) +
					(iconBox?.width ?? 0) / 2 -
					((buttonBox?.x ?? 0) + (buttonBox?.width ?? 0) / 2),
			),
		).toBeLessThanOrEqual(0.5);
	}
});

test("DMX application stories render the production matrix, inspector, and source mutations", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1496, height: 761 });
	await page.goto(
		"/iframe.html?id=tosklight-windows-dmx--values-output-summary&viewMode=story",
	);
	await expect(page.getByText("DMX Output", { exact: true })).toBeVisible();
	await expect(
		page.getByRole("button", { name: "Values as dots" }),
	).toBeVisible();
	await expect(page.getByRole("button", { name: "Sources" })).toBeVisible();
	await expect(page.locator(".dmx-universe")).toHaveCount(4);
	await expect(page.locator(".dmx-universe button")).toHaveCount(2_048);
	await expect(page.locator(".dmx-info-pane")).toContainText("Output summary");
	await expect(page.locator(".ui-data-table")).toHaveCount(0);
	await expect(page.locator(".ui-selection-tree")).toHaveCount(0);

	await page.goto(
		"/iframe.html?id=tosklight-windows-dmx--selected-patched-channel&viewMode=story",
	);
	await expect(page.locator(".dmx-row button.selected")).toHaveCount(1, {
		timeout: 15_000,
	});
	await expect(page.locator(".dmx-dip-switches > span")).toHaveCount(9);
	await expect(page.locator(".dmx-fixture-card")).toContainText("Stage Hazer");
	await expect(page.getByRole("button", { name: /Raw value/u })).toBeVisible();
	await expect(
		page.getByRole("button", { name: "Release override" }),
	).toBeVisible();

	await page.goto(
		"/iframe.html?id=tosklight-windows-dmx--sources-with-overrides&viewMode=story",
	);
	const source = page.locator(".dmx-detail-list article").first();
	await source.getByRole("button", { name: "Release" }).click();
	await expect(
		page.locator('output[aria-label="Last DMX mutation"]'),
	).toHaveText("1.13:released");

	await page.goto(
		"/iframe.html?id=tosklight-windows-dmx--sources-empty&viewMode=story",
	);
	await expect(
		page.getByText("No raw DMX overrides are active."),
	).toBeVisible();
});

test("Help application stories render real navigation, Markdown, search, and status states", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1496, height: 761 });
	await page.goto(
		"/iframe.html?id=tosklight-windows-help--quick-start&viewMode=story",
	);
	await expect(page.getByText("Help", { exact: true })).toBeVisible();
	await expect(page.getByText("Live documentation")).toBeVisible();
	await expect(
		page.getByRole("navigation", { name: "Help topics" }),
	).toBeVisible();
	await expect(page.locator(".help-content h1")).toHaveText("Quick Start");
	await expect(page.locator(".help-content .desk-key")).toHaveCount(4);
	await expect(page.locator(".help-content .keyboard-key")).toHaveCount(2);
	await expect(page.locator(".help-content img")).toHaveJSProperty(
		"complete",
		true,
	);
	await expect(page.locator(".ui-selection-tree")).toHaveCount(0);
	await expect(
		page.getByRole("textbox", { name: "Search Help" }),
	).toBeVisible();

	await page.getByRole("textbox", { name: "Search Help" }).fill("Command Line");
	await expect(page.getByRole("button", { name: "Quick Start" })).toHaveCount(
		0,
	);
	await expect(
		page.getByRole("button", { name: "Command Line" }),
	).toBeVisible();
	await page
		.getByRole("textbox", { name: "Search Help" })
		.fill("No such topic");
	await expect(page.getByText("No matching help topics.")).toBeVisible();

	await page.goto(
		"/iframe.html?id=tosklight-windows-help--loading&viewMode=story",
	);
	const loadingPane = page.locator(".help-topic-pane");
	const loadingState = loadingPane.locator(".ui-window-empty-state");
	await expect(
		loadingState.getByText("Loading help", { exact: true }),
	).toBeVisible();
	const loadingIcon = loadingState.locator(".help-state-icon");
	await expect(loadingIcon).toBeVisible();
	await expect(loadingState.locator(".icon")).toHaveCSS(
		"color",
		"rgb(132, 145, 155)",
	);
	await expect(loadingState).toHaveCSS("color", "rgb(132, 145, 155)");
	const loadingPaneBox = await loadingPane.boundingBox();
	const loadingStateBox = await loadingState.boundingBox();
	expect(
		(loadingStateBox?.x ?? 0) + (loadingStateBox?.width ?? 0) / 2,
	).toBeCloseTo((loadingPaneBox?.x ?? 0) + (loadingPaneBox?.width ?? 0) / 2, 0);
	expect(
		(loadingStateBox?.y ?? 0) + (loadingStateBox?.height ?? 0) / 2,
	).toBeCloseTo(
		(loadingPaneBox?.y ?? 0) + (loadingPaneBox?.height ?? 0) / 2,
		0,
	);
	await page.goto(
		"/iframe.html?id=tosklight-windows-help--empty-catalog&viewMode=story",
	);
	const emptyState = page.locator(".help-topic-pane .ui-window-empty-state");
	await expect(
		emptyState.getByText("No help topics found", { exact: true }),
	).toBeVisible();
	await expect(emptyState.locator(".help-state-icon")).toBeVisible();
	await page.goto(
		"/iframe.html?id=tosklight-windows-help--catalog-error&viewMode=story",
	);
	await expect(
		page.getByText("Unable to load help: Catalog request failed"),
	).toBeVisible();
	await page.goto(
		"/iframe.html?id=tosklight-windows-help--catalog-warning&viewMode=story",
	);
	await expect(
		page.getByText("One optional help topic could not be indexed."),
	).toBeVisible();
	const warning = page.locator(".help-catalog-warning");
	await expect(warning.locator("svg")).toBeVisible();
	await expect(warning).toHaveCSS("border-top-color", "rgb(217, 133, 37)");

	await page.goto(
		"/iframe.html?id=tosklight-windows-help--search-results&viewMode=story",
	);
	await expect(page.getByRole("textbox", { name: "Search Help" })).toHaveValue(
		"Command Line",
	);
	await expect(page.getByRole("button", { name: "Quick Start" })).toHaveCount(
		0,
	);
	await expect(
		page.getByRole("button", { name: "Command Line" }),
	).toBeVisible();
	await expect(page.locator(".help-content h1")).toHaveText("Command Line");

	await page.goto(
		"/iframe.html?id=tosklight-windows-help--search-no-results&viewMode=story",
	);
	await expect(page.getByRole("textbox", { name: "Search Help" })).toHaveValue(
		"No such topic",
	);
	await expect(page.getByText("No matching help topics.")).toBeVisible();
});

test("application shell stories preserve Dock and software or hardware control modes", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1496, height: 761 });
	await page.goto(
		"/iframe.html?id=tosklight-shell-and-control--dock-desktops&viewMode=story",
	);
	await expect(page.locator(".left-dock")).toBeVisible();
	const dockModeToggle = page.getByRole("button", {
		name: "Desktops / Built-ins",
	});
	await expect(dockModeToggle).toHaveAttribute("data-dock-mode", "desks");
	const desktopModeLabel = dockModeToggle.getByText("Desktops", {
		exact: true,
	});
	const builtInModeLabel = dockModeToggle.getByText("Built-ins", {
		exact: true,
	});
	await expect(desktopModeLabel).toHaveClass(/active/u);
	await expect(builtInModeLabel).not.toHaveClass(/active/u);
	expect(
		await desktopModeLabel.evaluate(
			(element) => getComputedStyle(element).color,
		),
	).not.toBe(
		await builtInModeLabel.evaluate(
			(element) => getComputedStyle(element).color,
		),
	);
	await expect(
		page.getByRole("button", { name: /New desktop/u }),
	).toBeVisible();
	const newDesktopIcon = page
		.getByRole("button", { name: "New desktop" })
		.locator(".dock-entry-icon");
	const newDesktopLabel = page
		.getByRole("button", { name: "New desktop" })
		.locator(".dock-entry-label");
	const [newDesktopIconBounds, newDesktopLabelBounds] = await Promise.all([
		newDesktopIcon.boundingBox(),
		newDesktopLabel.boundingBox(),
	]);
	expect(newDesktopIconBounds).not.toBeNull();
	expect(newDesktopLabelBounds).not.toBeNull();
	expect(newDesktopLabelBounds?.y ?? 0).toBeGreaterThanOrEqual(
		(newDesktopIconBounds?.y ?? 0) + (newDesktopIconBounds?.height ?? 0),
	);

	await dockModeToggle.click();
	await expect(dockModeToggle).toHaveAttribute("data-dock-mode", "builtins");
	await expect(page.getByRole("button", { name: "Stage" })).toBeVisible();
	await expect(page.getByRole("button", { name: "New desktop" })).toHaveCount(
		0,
	);
	await expect(page.locator(".dock-list-swap-builtins")).toBeVisible();
	expect(
		await page
			.locator(".dock-list-swap-builtins")
			.evaluate((element) => getComputedStyle(element).animationName),
	).toContain("dock-list-swap-builtins");

	await page.goto(
		"/iframe.html?id=tosklight-shell-and-control--dock-built-ins&viewMode=story",
	);
	await expect(page.getByRole("button", { name: "Stage" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Fixtures" })).toBeVisible();

	await page.goto(
		"/iframe.html?id=tosklight-shell-and-control--programmer-software&viewMode=story",
	);
	await expect(
		page.locator(".control-section.programmer.touch-connected"),
	).toBeVisible();
	await expect(page.getByRole("textbox", { name: "Command line" })).toHaveValue(
		"FIXTURE 1 THRU 12 AT 68",
	);
	await expect(page.locator(".touch-encoder")).toHaveCount(4);
	await expect(page.locator(".numeric-pad")).toBeVisible();

	await page.goto(
		"/iframe.html?id=tosklight-shell-and-control--playbacks-software&viewMode=story",
	);
	await expect(page.locator(".playback-tools")).toBeVisible();
	await expect(page.locator(".numeric-pad")).toHaveCount(0);
	await expect(page.locator(".speed-group-stack button")).toHaveCount(5);
	await expect(page.locator('[data-playback-bank-mode="touch"]')).toBeVisible();
	await expect(page.locator(".playback-card")).toHaveCount(16);

	await page.goto(
		"/iframe.html?id=tosklight-shell-and-control--playbacks-hardware-connected&viewMode=story",
	);
	await expect(
		page.locator(".control-section.playbacks.hardware-connected"),
	).toBeVisible();
	await expect(
		page.locator('[data-playback-bank-mode="hardware"]'),
	).toBeVisible();
	await expect(page.locator(".hardware-encoder-display")).toHaveCount(0);
	await expect(page.locator(".hardware-control-summary")).toBeVisible();
	await expect(page.locator(".hardware-speed-groups button")).toHaveCount(5);
});

test("application control stories cover parameter families, playback banks, and keypad actions", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1496, height: 761 });
	await page.goto(
		"/iframe.html?id=tosklight-shell-and-control--parameter-families-and-touch-encoders&viewMode=story",
	);
	for (const family of [
		"Intensity",
		"Color",
		"Position",
		"Beam",
		"Shapers",
		"Focus",
		"Control",
		"Media",
	]) {
		await expect(
			page.getByRole("button", { name: family, exact: true }),
		).toBeVisible();
	}
	await expect(page.locator(".touch-encoder")).toHaveCount(4);

	await page.goto(
		"/iframe.html?id=tosklight-shell-and-control--playback-bank-touch&viewMode=story",
	);
	await expect(page.locator('[data-playback-bank-mode="touch"]')).toBeVisible();
	await expect(
		page.locator(
			'[data-playback-bank-mode="touch"] > [data-ui-component="touch-playback-card"]',
		),
	).toHaveCount(4);

	await page.goto(
		"/iframe.html?id=tosklight-shell-and-control--playback-bank-hardware&viewMode=story",
	);
	await expect(
		page.locator(
			'[data-playback-bank-mode="hardware"] > [data-ui-component="hardware-playback-card"]',
		),
	).toHaveCount(4);

	await page.goto(
		"/iframe.html?id=tosklight-shell-and-control--keypad-programmer-fade-preload-highlight-and-step&viewMode=story",
	);
	await expect(page.getByText("Prog. Fade", { exact: true })).toBeVisible();
	await expect(page.getByRole("button", { name: "PRELOAD GO" })).toBeVisible();
	await expect(page.getByRole("button", { name: "HIGH" })).toBeVisible();
	await expect(page.getByRole("button", { name: "PREV" })).toBeVisible();
	await expect(page.getByRole("button", { name: "NEXT" })).toBeVisible();
});

test("the serverless command line is interactive and used as the Storybook command surface", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-command-line--interactive&viewMode=story",
	);
	const command = page.getByRole("textbox", { name: "Command line" });
	await expect(command).toHaveValue("FIXTURE 1 AT 68");
	await command.click();
	const history = page.getByRole("dialog", { name: "Command line history" });
	await expect(history).toBeVisible();
	await history
		.getByRole("article")
		.filter({ hasText: "GROUP 99 AT FULL" })
		.getByRole("button", { name: "Reuse" })
		.click();
	await expect(command).toHaveValue("GROUP 99 AT FULL");
	await expect(history).toHaveCount(0);

	await command.fill("FIXTURE 2 AT 50");
	await command.press("Enter");
	await expect(
		page.getByRole("img", { name: "Command applied" }),
	).toBeVisible();
	await expect(page.getByLabel("Command line event")).toContainText(
		"Executed FIXTURE 2 AT 50",
	);

	const mode = page.getByRole("button", { name: /PROG/u });
	await mode.click();
	await expect(page.locator(".command-line-bar")).toHaveClass(/playback-mode/u);
	const record = page.getByRole("button", { name: "REC" });
	await record.click();
	await expect(page.getByRole("button", { name: "REC ARMED" })).toHaveAttribute(
		"aria-pressed",
		"true",
	);
	await page.getByRole("button", { name: "PRELOAD" }).click();
	await expect(page.getByRole("button", { name: "PRELOAD GO" })).toHaveText(
		"PRELOAD GO",
	);
	await page
		.getByRole("button", { name: /Open running and output controls/u })
		.click();
	await expect(page.getByLabel("Command line event")).toHaveText(
		"Opened running and output controls",
	);
});

test("Command Section follows the global hardware context and production control geometry", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1496, height: 761 });
	await page.goto(
		"/iframe.html?id=tosklight-command-section--programmer-software&viewMode=story&globals=mode:software",
	);
	await expect(
		page.locator(".control-section.programmer.touch-connected"),
	).toBeVisible();
	await expect(page.getByText("Intensity encoders")).toHaveCount(0);
	await expect(page.locator(".encoder-section-header")).toHaveCount(0);
	await expect(page.locator(".touch-encoder")).toHaveCount(4);
	const highlightKeys = await page
		.locator(".highlight-controls [data-keypad-key]")
		.evaluateAll((buttons) =>
			buttons.map((button) => button.getAttribute("data-keypad-key")),
		);
	expect(highlightKeys).toEqual(["HIGH", "PREV", "NEXT", "ALL"]);
	for (const key of [
		"DEL",
		"MOV",
		"CPY",
		"SET",
		"GRP",
		"CUE",
		"TIME",
		"DIV",
		"-",
		"+",
		"TRU",
		"AT",
	])
		await expect(
			page.locator(`[data-keypad-key="${key}"]`).first(),
		).toHaveClass(/action/u);
	await expect(page.locator('[data-keypad-key="DEL"]').first()).toHaveCSS(
		"color",
		"rgb(255, 179, 15)",
	);
	await expect(page.locator('[data-keypad-key="ENT"]').first()).toHaveClass(
		/enter/u,
	);
	await expect(page.locator('[data-keypad-key="ENT"]').first()).toHaveCSS(
		"color",
		"rgb(27, 214, 236)",
	);
	await expect(page.locator(".mode-toggle")).toHaveCSS("margin-left", "0px");
	await expect(page.locator(".mode-toggle")).toHaveCSS("width", "116px");
	await expect(page.getByRole("button", { name: "Dynamics" })).toHaveCSS(
		"color",
		"rgb(27, 214, 236)",
	);
	await expect(page.getByRole("button", { name: "Dynamics" })).not.toHaveCSS(
		"border-color",
		"rgb(27, 214, 236)",
	);
	await page.getByRole("button", { name: "Dynamics" }).click();
	await expect(page.getByRole("button", { name: "Dynamics" })).toHaveCSS(
		"border-color",
		"rgb(27, 214, 236)",
	);

	for (const key of ["SET", "SHIFT"]) {
		const button = page.locator(`[data-keypad-key="${key}"]`).first();
		await button.click();
		await expect(button).toHaveClass(/active/u);
		await expect(button).toHaveAttribute("aria-pressed", "true");
		await button.click();
		await expect(button).not.toHaveClass(/active/u);
	}

	await page.goto(
		"/iframe.html?id=tosklight-command-section--configurable&viewMode=story&globals=mode:software&args=clearState:selection;previousEnabled:false;nextEnabled:false;preloadArmed:true",
	);
	await expect(page.locator('[data-keypad-key="CLR"]').first()).toHaveClass(
		/clear-active/u,
	);
	await expect(page.locator('[data-keypad-key="CLR"]').first()).toHaveCSS(
		"background-color",
		"rgb(214, 166, 0)",
	);
	await expect(page.locator('[data-keypad-key="CLR"]').first()).toHaveCSS(
		"color",
		"rgb(48, 56, 61)",
	);
	await expect(page.locator('[data-keypad-key="PREV"]')).toBeDisabled();
	await expect(page.locator('[data-keypad-key="NEXT"]')).toBeDisabled();
	await expect(page.getByRole("button", { name: /PRELOAD GO/u })).toBeVisible();
	await expect(page.getByRole("textbox", { name: "Command line" })).toHaveClass(
		/blind/u,
	);
	await expect(page.getByRole("textbox", { name: "Command line" })).toHaveCSS(
		"color",
		"rgb(255, 179, 15)",
	);

	await page.goto(
		"/iframe.html?id=tosklight-command-section--configurable&viewMode=story&globals=mode:software&args=clearState:active-values",
	);
	await expect(page.locator('[data-keypad-key="CLR"]').first()).toHaveClass(
		/clear-warning/u,
	);
	await expect(page.locator('[data-keypad-key="CLR"]').first()).toHaveCSS(
		"animation-name",
		"clear-blink",
	);

	await page.goto(
		"/iframe.html?id=tosklight-command-section--playbacks-software&viewMode=story&globals=mode:software",
	);
	const playbackCards = page.locator(".playback-card");
	await expect(playbackCards).toHaveCount(16);
	await expect(
		page.locator(
			".playback-card.loaded, .playback-card.selected, .playback-card.running",
		),
	).toHaveCount(0);
	await expect(playbackCards.first()).toHaveCSS("outline-style", "none");
	const playbackRows = await playbackCards.evaluateAll((cards) =>
		[...new Set(cards.map((card) => card.getBoundingClientRect().y))].sort(
			(left, right) => left - right,
		),
	);
	expect(playbackRows).toHaveLength(2);
	for (const name of ["Previous playback page", "Next playback page"]) {
		const button = page.getByRole("button", { name });
		await expect(button.locator("svg")).toBeVisible();
		const bounds = await button.boundingBox();
		expect(bounds?.height).toBeGreaterThanOrEqual(60);
	}

	await page.goto(
		"/iframe.html?id=tosklight-command-section--configurable&viewMode=story&globals=mode:hardware",
	);
	await expect(
		page.locator(".control-section.programmer.hardware-connected"),
	).toBeVisible();
	await expect(page.locator(".touch-encoder")).toHaveCount(0);
	await expect(page.locator(".hardware-encoder-display")).toHaveCount(4);
	await expect(
		page.locator(".hardware-encoder-target.hardware-encoder-primary").first(),
	).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
	await expect(page.locator(".encoder-section")).toHaveCount(0);
	await expect(page.locator(".hardware-control-summary")).toBeVisible();

	await page.goto(
		"/iframe.html?id=tosklight-command-line--interactive&viewMode=story&globals=mode:hardware",
	);
	await expect(page.locator(".command-line-bar.hardware-mode")).toBeVisible();
	await expect(page.getByRole("button", { name: "ESC" })).toHaveCount(0);
});

test("touch and hardware encoder stories exercise continuous input, modal entry, and release", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=controls-encoders--individual-touch&viewMode=story",
	);
	const encoder = page.getByRole("group", { name: "Enc 1 · Dimmer" });
	const value = encoder.locator(".touch-encoder-value");
	await expect(encoder.locator(".touch-encoder-labels b")).toHaveText("Dimmer");
	await expect(encoder.locator(".touch-encoder-labels small")).toHaveText(
		"Enc 1",
	);
	await expect(encoder.locator(".touch-encoder-set")).toHaveCount(0);
	await expect(encoder.locator(".touch-encoder-legend")).toHaveText(
		"Increase•••Set•••Decrease",
	);
	const [encoderGeometry, surfaceGeometry, valueGeometry, legendGeometry] =
		await Promise.all([
			encoder.boundingBox(),
			encoder.locator(".touch-encoder-surface").boundingBox(),
			value.boundingBox(),
			encoder.locator(".touch-encoder-legend").boundingBox(),
		]);
	expect(encoderGeometry).not.toBeNull();
	expect(surfaceGeometry).not.toBeNull();
	expect(valueGeometry).not.toBeNull();
	expect(legendGeometry).not.toBeNull();
	expect(
		(valueGeometry?.x ?? 0) +
			(valueGeometry?.width ?? 0) / 2 -
			((encoderGeometry?.x ?? 0) + (encoderGeometry?.width ?? 0) / 2),
	).toBeCloseTo(0, 5);
	expect(
		(valueGeometry?.y ?? 0) +
			(valueGeometry?.height ?? 0) / 2 -
			((encoderGeometry?.y ?? 0) + (encoderGeometry?.height ?? 0) / 2),
	).toBeCloseTo(0, 5);
	expect(valueGeometry?.width).toBeCloseTo(surfaceGeometry?.width ?? 0, 0);
	await expect(value).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
	await expect(value).toHaveCSS("border-top-width", "0px");
	expect(legendGeometry?.x).toBeGreaterThan(valueGeometry?.x ?? 0);
	expect(legendGeometry?.height).toBeCloseTo(
		(surfaceGeometry?.height ?? 0) * 0.44,
		0,
	);
	expect(legendGeometry?.width).toBeCloseTo(18, 0);
	expect(
		(surfaceGeometry?.x ?? 0) +
			(surfaceGeometry?.width ?? 0) -
			((legendGeometry?.x ?? 0) + (legendGeometry?.width ?? 0)),
	).toBeCloseTo(1, 0);
	expect(
		(legendGeometry?.y ?? 0) +
			(legendGeometry?.height ?? 0) / 2 -
			((surfaceGeometry?.y ?? 0) + (surfaceGeometry?.height ?? 0) / 2),
	).toBeCloseTo(0, 0);
	const ridgesGeometry = await encoder
		.locator(".touch-encoder-ridges")
		.boundingBox();
	expect(ridgesGeometry?.width).toBeCloseTo(
		(surfaceGeometry?.width ?? 0) - 32,
		0,
	);
	for (const zone of [
		encoder.locator(".touch-encoder-tap-positive"),
		value,
		encoder.locator(".touch-encoder-tap-negative"),
	]) {
		const zoneGeometry = await zone.boundingBox();
		expect(zoneGeometry?.width).toBeCloseTo(surfaceGeometry?.width ?? 0, 0);
		expect(zoneGeometry?.height).toBeCloseTo(
			(surfaceGeometry?.height ?? 0) / 3,
			0,
		);
	}
	await expect(value).toHaveText("52.0%");
	await encoder.locator(".touch-encoder-tap-positive").click();
	await expect(value).toHaveText("52.1%");
	await encoder.locator(".touch-encoder-tap-negative").click();
	await expect(value).toHaveText("52.0%");
	await encoder.hover();
	await page.mouse.wheel(0, -100);
	await expect(value).toHaveText("52.1%");
	await page.keyboard.down("Shift");
	await page.mouse.wheel(0, 100);
	await page.keyboard.up("Shift");
	await expect(value).toHaveText("51.1%");

	const box = await encoder.boundingBox();
	expect(box).not.toBeNull();
	await page.mouse.move((box?.x ?? 0) + 30, (box?.y ?? 0) + 180);
	await page.mouse.down();
	await page.waitForTimeout(120);
	await page.mouse.move((box?.x ?? 0) + 30, (box?.y ?? 0) + 160);
	await expect(encoder.locator(".touch-encoder-drag-feedback")).toHaveCount(0);
	await expect(encoder).toHaveAttribute("data-motion", "up");
	const slowMotionSpeed = Number.parseFloat(
		await encoder.evaluate((node) =>
			node.style.getPropertyValue("--encoder-motion-speed"),
		),
	);
	await page.waitForTimeout(10);
	await page.mouse.move((box?.x ?? 0) + 30, (box?.y ?? 0) + 110);
	const fastMotionSpeed = Number.parseFloat(
		await encoder.evaluate((node) =>
			node.style.getPropertyValue("--encoder-motion-speed"),
		),
	);
	expect(fastMotionSpeed).toBeGreaterThan(slowMotionSpeed);
	await page.waitForTimeout(50);
	const offsetBeforeReturn = Number.parseFloat(
		await encoder.evaluate((node) =>
			node.style.getPropertyValue("--encoder-ridge-offset"),
		),
	);
	await page.mouse.move((box?.x ?? 0) + 30, (box?.y ?? 0) + 140);
	await expect(encoder).toHaveAttribute("data-motion", "up");
	const returningMotionSpeed = Number.parseFloat(
		await encoder.evaluate((node) =>
			node.style.getPropertyValue("--encoder-motion-speed"),
		),
	);
	expect(returningMotionSpeed).toBeLessThan(fastMotionSpeed);
	expect(returningMotionSpeed).toBeGreaterThan(slowMotionSpeed);
	await page.waitForTimeout(50);
	const offsetAfterReturn = Number.parseFloat(
		await encoder.evaluate((node) =>
			node.style.getPropertyValue("--encoder-ridge-offset"),
		),
	);
	expect(offsetAfterReturn).toBeLessThan(offsetBeforeReturn);
	await page.waitForTimeout(90);
	await page.mouse.up();
	await expect(value).not.toHaveText("51.1%");

	await encoder.click({
		position: {
			x: (box?.width ?? 0) / 2,
			y: (box?.height ?? 0) / 2,
		},
	});
	const touchEditor = page.getByRole("dialog", {
		name: "Enc 1 · Dimmer value",
	});
	await touchEditor.getByRole("button", { name: "7" }).click();
	await touchEditor.getByRole("button", { name: "5" }).click();
	await touchEditor.getByRole("button", { name: "ENTER" }).click();
	await expect(value).toHaveText("75.0%");
	await encoder
		.getByRole("button", { name: "Set Enc 1 · Dimmer value" })
		.click();
	await touchEditor.getByRole("button", { name: "Release" }).click();
	await expect(value).toHaveText("Released");

	for (const constrained of [
		"individual-touch-disabled",
		"individual-touch-indexed",
	]) {
		await page.goto(
			`/iframe.html?id=controls-encoders--${constrained}&viewMode=story`,
		);
		const constrainedEncoder = page.getByRole("group");
		await expect(constrainedEncoder).toHaveAttribute("aria-disabled", "true");
		await expect(
			constrainedEncoder.getByRole("button", {
				name: /Set Enc 1 · Dimmer value/u,
			}),
		).toBeDisabled();
	}

	await page.goto(
		"/iframe.html?id=controls-encoders--individual-hardware&viewMode=story",
	);
	const hardwareEncoder = page.getByRole("button", {
		name: "Encoder 1: Pan, 80° ... 100°",
	});
	const primaryName = hardwareEncoder.locator(
		".hardware-encoder-primary-labels b",
	);
	const encoderNumber = hardwareEncoder.locator(
		".hardware-encoder-primary-labels small",
	);
	const primaryValue = hardwareEncoder.locator(
		".hardware-encoder-target.hardware-encoder-primary strong",
	);
	const secondaryName = hardwareEncoder.locator(
		".hardware-encoder-secondary-labels b",
	);
	const pushTurn = hardwareEncoder.locator(
		".hardware-encoder-secondary-labels small",
	);
	const secondaryValue = hardwareEncoder.locator(
		".hardware-encoder-target.hardware-encoder-secondary strong",
	);
	await expect(primaryName).toHaveText("Pan");
	await expect(encoderNumber).toHaveText("Enc 1");
	await expect(primaryValue).toHaveText("80° ... 100°");
	await expect(secondaryName).toHaveText("Tilt");
	await expect(pushTurn).toHaveText("Push-turn");
	await expect(secondaryValue).toHaveText("30°");
	await expect(primaryName).toHaveCSS("word-break", "break-all");
	const [
		hardwareGeometry,
		primaryNameGeometry,
		encoderNumberGeometry,
		primaryValueGeometry,
		dividerGeometry,
		secondaryNameGeometry,
		pushTurnGeometry,
		secondaryValueGeometry,
	] = await Promise.all([
		hardwareEncoder.boundingBox(),
		primaryName.boundingBox(),
		encoderNumber.boundingBox(),
		primaryValue.boundingBox(),
		hardwareEncoder.locator(".hardware-encoder-divider").boundingBox(),
		secondaryName.boundingBox(),
		pushTurn.boundingBox(),
		secondaryValue.boundingBox(),
	]);
	expect(primaryNameGeometry?.x).toBeCloseTo((hardwareGeometry?.x ?? 0) + 6, 0);
	expect(
		(primaryNameGeometry?.x ?? 0) + (primaryNameGeometry?.width ?? 0),
	).toBeLessThanOrEqual(encoderNumberGeometry?.x ?? 0);
	expect(
		(encoderNumberGeometry?.x ?? 0) + (encoderNumberGeometry?.width ?? 0),
	).toBeCloseTo(
		(hardwareGeometry?.x ?? 0) + (hardwareGeometry?.width ?? 0) - 6,
		0,
	);
	expect(
		(primaryValueGeometry?.x ?? 0) + (primaryValueGeometry?.width ?? 0) / 2,
	).toBeCloseTo(
		(hardwareGeometry?.x ?? 0) + (hardwareGeometry?.width ?? 0) / 2,
		0,
	);
	expect(
		Math.abs(
			(primaryValueGeometry?.y ?? 0) +
				(primaryValueGeometry?.height ?? 0) / 2 -
				((hardwareGeometry?.y ?? 0) + (hardwareGeometry?.height ?? 0) * 0.25),
		),
	).toBeLessThanOrEqual(1);
	expect(dividerGeometry?.y).toBeCloseTo(
		(hardwareGeometry?.y ?? 0) + (hardwareGeometry?.height ?? 0) / 2,
		0,
	);
	expect(secondaryNameGeometry?.x).toBeCloseTo(
		(hardwareGeometry?.x ?? 0) + 6,
		0,
	);
	expect(
		(pushTurnGeometry?.x ?? 0) + (pushTurnGeometry?.width ?? 0),
	).toBeCloseTo(
		(hardwareGeometry?.x ?? 0) + (hardwareGeometry?.width ?? 0) - 6,
		0,
	);
	expect(
		(secondaryValueGeometry?.x ?? 0) + (secondaryValueGeometry?.width ?? 0) / 2,
	).toBeCloseTo(
		(hardwareGeometry?.x ?? 0) + (hardwareGeometry?.width ?? 0) / 2,
		0,
	);
	expect(
		Math.abs(
			(secondaryValueGeometry?.y ?? 0) +
				(secondaryValueGeometry?.height ?? 0) / 2 -
				((hardwareGeometry?.y ?? 0) + (hardwareGeometry?.height ?? 0) * 0.75),
		),
	).toBeLessThanOrEqual(1);
	await hardwareEncoder.focus();
	await page.keyboard.press("Enter");
	const hardwareEditor = page.getByRole("dialog", {
		name: "Encoder 1 value",
	});
	await expect(
		hardwareEditor.locator(".hardware-encoder-target-selector"),
	).toContainText("PanTilt");
	await expect(
		hardwareEditor.getByRole("button", { name: "Pan", exact: true }),
	).toHaveAttribute("aria-pressed", "true");
	await hardwareEditor
		.getByRole("button", { name: "Tilt", exact: true })
		.click();
	await expect(
		hardwareEditor.getByRole("button", { name: "Tilt", exact: true }),
	).toHaveAttribute("aria-pressed", "true");
	await hardwareEditor
		.getByRole("button", { name: "Close Encoder 1 value" })
		.click();

	await hardwareEncoder.click({
		position: {
			x: (hardwareGeometry?.width ?? 0) / 2,
			y: (hardwareGeometry?.height ?? 0) * 0.75,
		},
	});
	await expect(
		hardwareEditor.locator(".hardware-encoder-target-selector"),
	).toHaveCount(0);
	await expect(hardwareEditor).toContainText("Tilt");
	await hardwareEditor.getByRole("button", { name: "4" }).click();
	await hardwareEditor.getByRole("button", { name: "ENTER" }).click();
	await expect(secondaryValue).toHaveText("4°");

	await hardwareEncoder.click({
		position: {
			x: (hardwareGeometry?.width ?? 0) / 2,
			y: (hardwareGeometry?.height ?? 0) * 0.25,
		},
	});
	await expect(hardwareEditor).toContainText("Pan");
	await hardwareEditor.getByRole("button", { name: "Release Pan" }).click();
	await expect(
		page.getByRole("button", { name: "Encoder 1: Pan, Released" }),
	).toBeVisible();

	await page.goto(
		"/iframe.html?id=controls-encoders--configurable-family&viewMode=story",
	);
	await expect(page.getByText("RGBW · AUV", { exact: true })).toBeVisible();
	await expect(page.locator(".touch-encoder")).toHaveCount(6);
	for (const [slot, attribute, color] of [
		[1, "Red", "#ff4d57"],
		[2, "Green", "#43d66f"],
		[3, "Blue", "#4f8cff"],
		[4, "White", "#edf4f6"],
		[5, "Amber", "#ffb52e"],
		[6, "UV", "#a56cff"],
	] as const) {
		const configurableEncoder = page.getByRole("group", {
			name: `Enc ${slot} · ${attribute}`,
		});
		await expect(
			configurableEncoder.locator(".touch-encoder-labels b"),
		).toHaveText(attribute);
		await expect(
			configurableEncoder.locator(".touch-encoder-labels small"),
		).toHaveText(`Enc ${slot}`);
		await expect(configurableEncoder).toHaveCSS("--encoder-color", color);
		if (attribute === "White")
			await expect(
				configurableEncoder.locator(".touch-encoder-value.range-value span"),
			).toHaveText(["0%", "100%"]);
		if (attribute === "White")
			await expect(
				configurableEncoder.locator(".touch-encoder-value.range-value i"),
			).toHaveText("...");
		const [configurableGeometry, legendGeometry] = await Promise.all([
			configurableEncoder.boundingBox(),
			configurableEncoder.locator(".touch-encoder-legend").boundingBox(),
		]);
		expect(
			(legendGeometry?.y ?? 0) +
				(legendGeometry?.height ?? 0) / 2 -
				((configurableGeometry?.y ?? 0) +
					(configurableGeometry?.height ?? 0) / 2),
		).toBeCloseTo(0, 0);
	}
	await page
		.getByRole("group", { name: "Enc 1 · Red" })
		.getByRole("button", { name: "Set Enc 1 · Red value" })
		.click();
	const configurableEditor = page.getByRole("dialog", {
		name: "Enc 1 · Red value",
	});
	await expect(
		configurableEditor.getByRole("button", { name: "THRU" }),
	).toBeVisible();
	await configurableEditor
		.getByRole("button", { name: "Show presets" })
		.click();
	for (const preset of [
		"Off",
		"Quarter",
		"Half",
		"Full",
		"House preset",
		"Show level",
	])
		await expect(
			configurableEditor.getByRole("button", { name: new RegExp(preset) }),
		).toBeVisible();
	await configurableEditor
		.getByRole("button", { name: "Close Enc 1 · Red value" })
		.click();
});

test("the consolidated playback stories cover controls, touch, hardware, loaded, held, and pickup states", async ({
	page,
}) => {
	const storyIndex = await page.request.get("/index.json");
	expect(storyIndex.ok()).toBe(true);
	const entries = Object.values(
		(
			(await storyIndex.json()) as {
				entries: Record<string, { title: string; type: string }>;
			}
		).entries,
	).filter(
		(entry) => entry.type === "story" && entry.title === "Controls/Playbacks",
	);
	expect(entries).toHaveLength(3);

	await page.goto(
		"/iframe.html?id=controls-playbacks--configurable-playback&viewMode=story",
	);
	const configurable = page.locator(".playback-card");
	await expect(configurable).toHaveCount(1);
	await expect(configurable).toHaveAttribute("data-playback-kind", "cue-list");
	await expect(configurable).toHaveAttribute("data-button-count", "3");
	await expect(configurable).toHaveAttribute("data-has-fader", "true");

	for (const mode of ["touch", "hardware"] as const) {
		await page.goto(
			`/iframe.html?id=controls-playbacks--eight-by-two-${mode}-bank&viewMode=story`,
		);
		const bank = page.locator(".playback-fader-bank");
		const cards = bank.locator(".playback-card");
		await expect(cards).toHaveCount(16);
		const contract = await cards.evaluateAll((elements) =>
			elements.map((card) => ({
				buttons: card.getAttribute("data-button-count"),
				fader: card.getAttribute("data-has-fader"),
				kind: card.getAttribute("data-playback-kind"),
			})),
		);
		expect(contract.slice(0, 7).every((card) => card.buttons === "1")).toBe(
			true,
		);
		expect(contract.slice(0, 8).every((card) => card.fader === "false")).toBe(
			true,
		);
		expect(contract.slice(8, 15).every((card) => card.buttons === "3")).toBe(
			true,
		);
		expect(contract.slice(8, 15).every((card) => card.fader === "true")).toBe(
			true,
		);
		expect(contract[7]).toMatchObject({
			buttons: "0",
			fader: "false",
			kind: "empty",
		});
		expect(contract[15]).toMatchObject({
			buttons: "0",
			fader: "false",
			kind: "empty",
		});
		await expect(page.locator(".playback-button-active")).toHaveCount(1);
	}

	await page.goto(
		"/iframe.html?id=controls-playbacks--eight-by-two-touch-bank&viewMode=story",
	);
	const loadedStatus = page.locator(".playback-summary-loaded");
	await expect(loadedStatus).toHaveCount(1);
	await expect(loadedStatus).toHaveText("LOADED");
	await expect(loadedStatus).toHaveCSS("border-top-color", "rgb(255, 179, 15)");
	await expect(
		loadedStatus.locator("xpath=..").getByText("3.2s", { exact: true }),
	).toHaveCount(0);
	const [loadedWidgetBox, loadedBadgeBox] = await Promise.all([
		loadedStatus.locator("xpath=..").boundingBox(),
		loadedStatus.boundingBox(),
	]);
	expect(
		(loadedWidgetBox?.x ?? 0) +
			(loadedWidgetBox?.width ?? 0) -
			((loadedBadgeBox?.x ?? 0) + (loadedBadgeBox?.width ?? 0)),
	).toBeCloseTo(0, 0);
	await expect(page.locator(".hardware-fader-pickup-difference")).toHaveCount(
		0,
	);
	await expect(page.getByText("FLASH HELD", { exact: true })).toHaveCount(0);
	const touchCard = page
		.locator(".touch-playback-card[data-playback-row='1']")
		.first();
	const touchStack = page
		.locator(".touch-playback-card[data-playback-row='1']")
		.first()
		.locator(".vertical-touch-fader-stack");
	const [
		stackBox,
		faderBox,
		actionsBox,
		lastButtonBox,
		identityBox,
		summaryBox,
	] = await Promise.all([
		touchStack.boundingBox(),
		touchStack.locator(".vertical-touch-fader").boundingBox(),
		touchStack.locator(".vertical-touch-fader-actions").boundingBox(),
		touchStack
			.locator(".vertical-touch-fader-actions .ui-button")
			.last()
			.boundingBox(),
		touchCard.locator(".playback-identity").boundingBox(),
		touchCard.locator(".playback-top-widget").boundingBox(),
	]);
	const faderValueBox = await touchStack
		.locator(".vertical-touch-fader strong")
		.boundingBox();
	const faderFillStyle = await touchStack
		.locator(".vertical-touch-fader")
		.evaluate((element) => {
			const fill = getComputedStyle(element, "::before");
			return {
				borderRadius: fill.borderRadius,
				left: fill.left,
				right: fill.right,
			};
		});
	expect(stackBox).not.toBeNull();
	expect(faderBox).not.toBeNull();
	expect(actionsBox).not.toBeNull();
	expect(lastButtonBox).not.toBeNull();
	await expect(touchStack.locator(".vertical-touch-fader > span")).toHaveText(
		"Master",
	);
	expect(faderFillStyle).toEqual({
		borderRadius: "2px",
		left: "1px",
		right: "1px",
	});
	expect(faderValueBox?.x).toBeCloseTo(faderBox?.x ?? 0, 0);
	expect((faderValueBox?.y ?? 0) + (faderValueBox?.height ?? 0)).toBeCloseTo(
		(faderBox?.y ?? 0) + (faderBox?.height ?? 0),
		0,
	);
	expect(identityBox?.height).toBeCloseTo(20, 0);
	expect(summaryBox?.height).toBeCloseTo(16, 0);
	expect(faderBox!.height).toBeGreaterThan(actionsBox!.height);
	expect(actionsBox!.height).toBeCloseTo(44, 0);
	expect(Math.abs(faderBox!.y - stackBox!.y)).toBeLessThanOrEqual(1);
	expect(
		Math.abs(actionsBox!.y - (faderBox!.y + faderBox!.height + 2)),
	).toBeLessThanOrEqual(1);
	expect(
		Math.abs(
			actionsBox!.y + actionsBox!.height - (stackBox!.y + stackBox!.height),
		),
	).toBeLessThanOrEqual(1);
	expect(
		Math.abs(
			actionsBox!.y + actionsBox!.height - (stackBox!.y + stackBox!.height),
		),
	).toBeLessThanOrEqual(2);
	expect(
		Math.abs(
			lastButtonBox!.y +
				lastButtonBox!.height -
				(stackBox!.y + stackBox!.height),
		),
	).toBeLessThanOrEqual(2);

	const beatCard = page.locator(
		".touch-playback-card[data-playback-row='1'][data-playback-kind='dynamic']",
	);
	const [beatWidgetBox, beatTrackBox, bpmBox] = await Promise.all([
		beatCard.locator(".playback-top-widget").boundingBox(),
		beatCard.locator(".playback-beat-track").boundingBox(),
		beatCard.getByText("120 BPM", { exact: true }).boundingBox(),
	]);
	expect(beatTrackBox?.height).toBeCloseTo(beatWidgetBox?.height ?? 0, 0);
	expect(beatTrackBox?.x).toBeCloseTo(beatWidgetBox?.x ?? 0, 0);
	expect(beatTrackBox?.x).toBeLessThan(bpmBox?.x ?? 0);

	const emptyCard = page
		.locator(".touch-playback-card[data-playback-row='1'].empty")
		.first();
	await expect(emptyCard).toHaveCSS("border-top-style", "dashed");
	await expect(emptyCard.locator(".playback-empty-body")).toHaveCSS(
		"border-top-style",
		"none",
	);
	expect(
		await emptyCard
			.locator(".playback-identity > b")
			.evaluate((element) => getComputedStyle(element).color),
	).toMatch(/0\.6\)?$/u);
	const selectionOverlay = await page
		.locator(".touch-playback-card.selected")
		.evaluate((card) => {
			const overlay = getComputedStyle(card, "::after");
			return {
				borderWidth: overlay.borderTopWidth,
				pointerEvents: overlay.pointerEvents,
				position: overlay.position,
				zIndex: overlay.zIndex,
			};
		});
	expect(selectionOverlay).toEqual({
		borderWidth: "2px",
		pointerEvents: "none",
		position: "absolute",
		zIndex: "20",
	});

	await page.goto(
		"/iframe.html?id=controls-playbacks--eight-by-two-hardware-bank&viewMode=story",
	);
	const compactCuelist = page.locator(
		".hardware-playback-card[data-playback-row='0'][data-playback-kind='cue-list']",
	);
	const fullCuelist = page.locator(
		".hardware-playback-card[data-playback-row='1'][data-playback-kind='cue-list']",
	);
	await expect(compactCuelist.locator(".hardware-cue-row:visible")).toHaveCount(
		1,
	);
	await expect(fullCuelist.locator(".hardware-cue-row:visible")).toHaveCount(3);
	await expect(fullCuelist).toContainText("House Open");
	await expect(fullCuelist).toContainText("Mephisto Stage Center");
	await expect(fullCuelist).toContainText("Stage Blackout");
	const cueColumnGeometry = await fullCuelist
		.locator(".hardware-cue-row:visible")
		.evaluateAll((rows) =>
			rows.map((row) => {
				const number = row.querySelector<HTMLElement>("span");
				const detail = row.querySelector<HTMLElement>("small");
				const name = row.querySelector<HTMLElement>("b");
				return {
					numberWidth: number?.getBoundingClientRect().width ?? 0,
					numberTextWidth: number?.scrollWidth ?? 0,
					numberFontFamily: number ? getComputedStyle(number).fontFamily : "",
					numberFontSize: number ? getComputedStyle(number).fontSize : "",
					numberFits: (number?.scrollWidth ?? 0) <= (number?.clientWidth ?? 0),
					detailFits: (detail?.scrollWidth ?? 0) <= (detail?.clientWidth ?? 0),
					nameFontSize: name ? getComputedStyle(name).fontSize : "",
					nameFontFamily: name ? getComputedStyle(name).fontFamily : "",
					nameWidth: name?.getBoundingClientRect().width ?? 0,
				};
			}),
		);
	expect(cueColumnGeometry.every(({ numberFits }) => numberFits)).toBe(true);
	expect(cueColumnGeometry.every(({ detailFits }) => detailFits)).toBe(true);
	expect(cueColumnGeometry.every(({ numberWidth }) => numberWidth < 22)).toBe(
		true,
	);
	expect(
		cueColumnGeometry.every(
			({ numberTextWidth, numberWidth }) =>
				Math.abs(numberWidth - numberTextWidth) <= 1,
		),
	).toBe(true);
	expect(
		cueColumnGeometry.every(
			({ nameFontFamily, nameFontSize, numberFontFamily, numberFontSize }) =>
				nameFontSize === "8px" &&
				numberFontSize === "8px" &&
				nameFontFamily.includes("-apple-system") &&
				numberFontFamily.includes("SFMono-Regular"),
		),
	).toBe(true);
	expect(cueColumnGeometry.every(({ nameWidth }) => nameWidth > 0)).toBe(true);

	const compactGroup = page.locator(
		".hardware-playback-card[data-playback-row='0'][data-playback-kind='group-master']",
	);
	const [compactInfoBox, compactSummaryBox, compactButtonBox] =
		await Promise.all([
			compactGroup.locator(".hardware-cue-list").boundingBox(),
			compactGroup.locator(".playback-top-widget").boundingBox(),
			compactGroup
				.locator(".hardware-playback-controls .ui-button")
				.boundingBox(),
		]);
	expect(compactInfoBox?.height).toBeGreaterThan(0);
	expect(compactSummaryBox?.y).toBeGreaterThanOrEqual(compactInfoBox?.y ?? 0);
	expect(
		(compactSummaryBox?.y ?? 0) + (compactSummaryBox?.height ?? 0),
	).toBeLessThanOrEqual(
		(compactInfoBox?.y ?? 0) + (compactInfoBox?.height ?? 0),
	);
	expect(compactButtonBox?.height).toBeLessThanOrEqual(15);

	await expect(page.locator(".hardware-fader-pickup-difference")).toHaveCount(
		2,
	);
	await expect(
		page.locator(".hardware-fader[data-pickup-direction='lower']"),
	).toHaveCount(1);
	await expect(
		page.locator(".hardware-fader[data-pickup-direction='raise']"),
	).toHaveCount(1);
	await expect(page.locator(".hardware-fader-target-marker")).toHaveCount(0);
	await expect(page.getByText(/Physical \d/u)).toHaveCount(0);
	await expect(page.getByText(/Target \d/u)).toHaveCount(0);
	expect(
		await page.locator(".hardware-fader.pickup-required > b").allTextContents(),
	).toEqual(["62%", "62%"]);
	const pickupGeometry = await page
		.locator(".hardware-fader.pickup-required")
		.evaluateAll((faders) =>
			faders.map((fader) => {
				const track = fader.querySelector<HTMLElement>(".hardware-fader-track");
				const fill = fader.querySelector<HTMLElement>(".hardware-fader-fill");
				const difference = fader.querySelector<HTMLElement>(
					".hardware-fader-pickup-difference",
				);
				const physical = fader.querySelector<HTMLElement>(
					".hardware-fader-physical-marker",
				);
				const value = fader.querySelector<HTMLElement>(":scope > b");
				const trackBox = track?.getBoundingClientRect();
				const valueBox = value?.getBoundingClientRect();
				const center = (element: HTMLElement | null) => {
					const box = element?.getBoundingClientRect();
					return box ? box.y + box.height / 2 : 0;
				};
				return {
					direction: fader.getAttribute("data-pickup-direction"),
					trackBottom: (trackBox?.y ?? 0) + (trackBox?.height ?? 0),
					trackHeight: trackBox?.height ?? 0,
					trackWidth: trackBox?.width ?? 0,
					fillTop: fill?.getBoundingClientRect().y ?? 0,
					differenceHeight: difference?.getBoundingClientRect().height ?? 0,
					physicalCenter: center(physical),
					physicalWidth: physical?.getBoundingClientRect().width ?? 0,
					physicalHeight: physical?.getBoundingClientRect().height ?? 0,
					valueRight: (valueBox?.x ?? 0) + (valueBox?.width ?? 0),
					valueBottom: (valueBox?.y ?? 0) + (valueBox?.height ?? 0),
					trackRight: (trackBox?.x ?? 0) + (trackBox?.width ?? 0),
					valueFontSize: value ? getComputedStyle(value).fontSize : "",
				};
			}),
		);
	const boundary = (
		geometry: (typeof pickupGeometry)[number],
		fraction: number,
	) => geometry.trackBottom - geometry.trackHeight * fraction;
	const lower = pickupGeometry.find(({ direction }) => direction === "lower")!;
	const raise = pickupGeometry.find(({ direction }) => direction === "raise")!;
	expect(Math.abs(lower.fillTop - boundary(lower, 0.75))).toBeLessThanOrEqual(
		1,
	);
	expect(
		Math.abs(lower.physicalCenter - boundary(lower, 0.75)),
	).toBeLessThanOrEqual(1);
	expect(lower.differenceHeight).toBeCloseTo(lower.trackHeight * 0.25, 0);
	expect(Math.abs(raise.fillTop - boundary(raise, 0.5))).toBeLessThanOrEqual(1);
	expect(
		Math.abs(raise.physicalCenter - boundary(raise, 0.5)),
	).toBeLessThanOrEqual(1);
	expect(raise.differenceHeight).toBeCloseTo(raise.trackHeight * 0.25, 0);
	for (const geometry of pickupGeometry) {
		expect(geometry.physicalWidth).toBeGreaterThanOrEqual(geometry.trackWidth);
		expect(geometry.physicalHeight).toBeCloseTo(1, 0);
		expect(geometry.trackRight - geometry.valueRight).toBeLessThanOrEqual(3);
		expect(geometry.trackBottom - geometry.valueBottom).toBeLessThanOrEqual(2);
		expect(geometry.valueFontSize).toBe("9px");
	}
	await expect(page.getByText("LOADED NEXT", { exact: true })).toHaveCount(1);
});

test("playback group controls enforce row action rules and touch or hardware height envelopes", async ({
	page,
}) => {
	for (const mode of ["touch", "hardware"] as const) {
		const story = `controls-playbacks--eight-by-two-${mode}-bank`;
		const expectedHeight = mode === "touch" ? 280 : 140;
		await page.goto(
			`/iframe.html?id=${story}&viewMode=story&args=playbacksWide:8;playbacksHigh:2;availableWidth:640`,
		);
		const widthSlider = page.getByRole("slider", {
			name: `${mode} playback group available width`,
		});
		const resizableFrame = page.locator(
			`[data-playback-group-frame="${mode}"]`,
		);
		await expect(resizableFrame).toHaveCSS("width", "640px");
		await widthSlider.fill("880");
		await expect(resizableFrame).toHaveCSS("width", "880px");

		for (const rows of [1, 2, 3, 4, 5, 6]) {
			await page.goto(
				`/iframe.html?id=${story}&viewMode=story&args=playbacksWide:4;playbacksHigh:${rows};availableWidth:640`,
			);
			const frame = page.locator(`[data-playback-group-frame="${mode}"]`);
			const bank = frame.locator(".playback-fader-bank");
			await expect(bank).toHaveAttribute("data-playback-rows", String(rows));
			await expect(bank).toHaveCSS("column-gap", "3px");
			await expect(bank).toHaveCSS("row-gap", "3px");
			await expect(bank.locator(".playback-card")).toHaveCount(4 * rows);
			const frameBox = await frame.boundingBox();
			expect(frameBox?.width).toBe(640);
			expect(frameBox?.height).toBe(expectedHeight);
			const cards = await bank
				.locator(".playback-card")
				.evaluateAll((elements) =>
					elements.map((card) => ({
						buttons: Number(card.getAttribute("data-button-count")),
						fader: card.getAttribute("data-has-fader") === "true",
						kind: card.getAttribute("data-playback-kind"),
						row: Number(card.getAttribute("data-playback-row")),
					})),
				);
			const assigned = cards.filter((card) => card.kind !== "empty");
			if (rows === 1) {
				expect(assigned.every((card) => card.fader && card.buttons === 3)).toBe(
					true,
				);
			} else if (rows === 2) {
				expect(
					assigned
						.filter((card) => card.row === 0)
						.every((card) => !card.fader && card.buttons === 1),
				).toBe(true);
				expect(
					assigned
						.filter((card) => card.row === 1)
						.every((card) => card.fader && card.buttons === 3),
				).toBe(true);
			} else if (mode === "touch") {
				expect(
					assigned.every(
						(card) => !card.fader && (card.buttons === 1 || card.buttons === 2),
					),
				).toBe(true);
			} else if (rows === 3 || rows === 4) {
				expect(
					assigned.every((card) => !card.fader && card.buttons === 1),
				).toBe(true);
				if (rows === 3) {
					expect(
						await bank
							.locator(".playback-card:not(.empty) .hardware-cue-list")
							.evaluateAll((lists) =>
								lists.every((list) => list.getBoundingClientRect().height > 0),
							),
					).toBe(true);
				} else {
					await expect(
						bank.locator(
							".playback-card:not(.empty) .hardware-cue-list:visible",
						),
					).toHaveCount(0);
				}
			} else {
				expect(
					assigned.every((card) => !card.fader && card.buttons === 0),
				).toBe(true);
				await expect(bank.locator(".hardware-playback-controls")).toHaveCount(
					0,
				);
				await expect(bank.locator("button")).toHaveCount(0);
				await expect(
					bank.locator(
						".playback-card:not(.empty) .hardware-playback-button-label",
					),
				).toHaveCount(assigned.length);
				const labelsFitBottomRight = await bank
					.locator(".playback-card:not(.empty)")
					.evaluateAll((cards) =>
						cards.every((card) => {
							const cardBox = card.getBoundingClientRect();
							const labelBox = card
								.querySelector<HTMLElement>(".hardware-playback-button-label")
								?.getBoundingClientRect();
							return (
								Boolean(labelBox) &&
								cardBox.right - (labelBox?.right ?? 0) <= 3 &&
								cardBox.bottom - (labelBox?.bottom ?? 0) <= 2
							);
						}),
					);
				expect(labelsFitBottomRight).toBe(true);
			}
			if (rows >= 3) {
				await expect(bank).toHaveClass(/compact-rows/u);
				const controlsFit = await bank
					.locator(".playback-card")
					.evaluateAll((elements) =>
						elements.every((card) => {
							const bounds = card.getBoundingClientRect();
							return [...card.querySelectorAll<HTMLElement>("button")].every(
								(button) => {
									const buttonBounds = button.getBoundingClientRect();
									return (
										buttonBounds.right <= bounds.right + 1 &&
										buttonBounds.bottom <= bounds.bottom + 1
									);
								},
							);
						}),
					);
				expect(controlsFit).toBe(true);
			}
		}
	}
});

test("application Stage stories render deterministic 2D fixtures and the real 3D canvas", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1496, height: 761 });
	await page.goto(
		"/iframe.html?id=tosklight-windows-stage--stage-2-d&viewMode=story",
	);
	await expect(page.locator(".stage-fixture")).toHaveCount(5);
	await expect(page.locator(".stage-fixture.selected")).toHaveCount(1);

	await page.goto(
		"/iframe.html?id=tosklight-windows-stage--stage-3-d&viewMode=story",
	);
	await expect(page.locator(".stage-3d-canvas")).toBeVisible();
	await expect(page.locator(".stage-3d-canvas canvas")).toBeVisible();
});

test("generic and application-owned pool stories preserve their slot contracts", async ({
	page,
}) => {
	for (const storyId of [
		"tables-and-grids-pools-generic-pool-window--empty",
		"tables-and-grids-pools-generic-pool-window--sparse",
	]) {
		await page.goto(`/iframe.html?id=${storyId}&viewMode=story`);
		await expect(page.locator(".pool-card")).toHaveCount(200);
	}
	await page.goto(
		"/iframe.html?id=tosklight-windows-pools--groups&viewMode=story",
	);
	await expect(page.locator(".group-card")).toHaveCount(200);
	await expect(page.locator(".group-card").nth(3)).toHaveAttribute(
		"data-pool-slot-id",
		"4",
	);
	await expect(page.locator(".group-card").nth(3)).toHaveClass(/selected/u);
	await page.locator(".group-card").nth(3).click({ button: "right" });
	await expect(page.getByLabel("Group pool interaction")).toHaveText(
		"Context Group 4",
	);

	await page.goto(
		"/iframe.html?id=tosklight-windows-pools--presets&viewMode=story",
	);
	await expect(page.locator(".preset-card")).toHaveCount(200);
	await expect(page.locator(".preset-card").nth(4)).toHaveAttribute(
		"data-pool-slot-id",
		"2.5",
	);
	await expect(page.locator(".preset-card.store-target")).toHaveCount(0);
	await page.locator(".preset-card").nth(4).click();
	await expect(page.getByLabel("Preset pool interaction")).toHaveText(
		"Activated Color 5",
	);

	await page.goto(
		"/iframe.html?id=tosklight-windows-cuelists-and-cues--pool&viewMode=story",
	);
	await expect(page.locator(".cuelist-card")).toHaveCount(1000);
	await expect(page.locator(".cuelist-card").first()).toHaveAttribute(
		"data-pool-slot-id",
		"1",
	);
	await page.getByRole("textbox", { name: "Search Cuelists" }).fill("Main");
	await expect(page.locator(".cuelist-card")).toHaveCount(1);
	await page.getByRole("textbox", { name: "Search Cuelists" }).fill("Side");
	const filteredCuelist = page.locator(".cuelist-card").first();
	await expect(page.locator(".cuelist-card")).toHaveCount(1);
	await expect(filteredCuelist.locator(".number")).toHaveText("4");
	await expect(filteredCuelist).toHaveAttribute("data-pool-slot-id", "4");
	await expect(filteredCuelist).toHaveAttribute("data-pool-position", "0");
	await page.getByRole("textbox", { name: "Search Cuelists" }).fill("");
	await expect(page.locator(".cuelist-card")).toHaveCount(1000);

	await page.goto(
		"/iframe.html?id=tables-and-grids-pools-generic-pool-window--extended&viewMode=story",
	);
	await expect(page.locator(".pool-card")).toHaveCount(260);
});

test("the narrow generic pool keeps touch-sized boxes in a scrollable window", async ({
	page,
}) => {
	await page.setViewportSize({ width: 430, height: 844 });
	await page.goto(
		"/iframe.html?id=tables-and-grids-pools-generic-pool-window--narrow&viewMode=story",
	);
	const scroller = page.locator(".ui-window-scroller");
	const firstCard = page.locator(".pool-card").first();
	const bounds = await firstCard.boundingBox();

	expect(bounds?.width).toBeGreaterThanOrEqual(88);
	expect(bounds?.height).toBeGreaterThanOrEqual(88);
	expect(
		await scroller.evaluate((node) => node.scrollHeight > node.clientHeight),
	).toBe(true);
	await scroller.evaluate((node) => {
		node.scrollTop = 300;
	});
	await expect
		.poll(() => scroller.evaluate((node) => node.scrollTop))
		.toBeGreaterThan(0);
});

test("pool cards stay square across width, height, resize, overflow, and application CSS", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1496, height: 900 });
	const genericStories = [
		"tables-and-grids-pools-generic-pool-window--empty",
		"tables-and-grids-pools-generic-pool-window--sparse",
		"tables-and-grids-pools-generic-pool-window--narrow-tall",
		"tables-and-grids-pools-generic-pool-window--narrow-short",
		"tables-and-grids-pools-generic-pool-window--wide-tall",
		"tables-and-grids-pools-generic-pool-window--wide-short",
		"tables-and-grids-pools-generic-pool-window--extended",
	];
	const widths = new Map<string, number>();
	for (const storyId of genericStories) {
		await page.goto(`/iframe.html?id=${storyId}&viewMode=story`);
		const first = await expectSquarePool(page);
		widths.set(storyId, first.width);
		const scroller = page.locator(".ui-window-scroller");
		expect(
			await scroller.evaluate(
				(node) => node.scrollWidth <= node.clientWidth + 1,
			),
		).toBe(true);
	}
	expect(
		Math.abs(
			(widths.get("tables-and-grids-pools-generic-pool-window--narrow-tall") ??
				0) -
				(widths.get(
					"tables-and-grids-pools-generic-pool-window--narrow-short",
				) ?? 0),
		),
	).toBeLessThanOrEqual(1);
	expect(
		Math.abs(
			(widths.get("tables-and-grids-pools-generic-pool-window--wide-tall") ??
				0) -
				(widths.get("tables-and-grids-pools-generic-pool-window--wide-short") ??
					0),
		),
	).toBeLessThanOrEqual(1);

	await page.goto(
		"/iframe.html?id=tables-and-grids-pools-generic-pool-window--extended&viewMode=story",
	);
	const extendedScroller = page.locator(".ui-window-scroller");
	expect(
		await extendedScroller.evaluate(
			(node) => node.scrollHeight > node.clientHeight,
		),
	).toBe(true);

	await page.goto(
		"/iframe.html?id=tables-and-grids-pools-generic-pool-window--live-resize&viewMode=story",
	);
	const before = await expectSquarePool(page);
	await page.getByRole("button", { name: "Resize pool viewport" }).click();
	await expect
		.poll(async () => (await poolGeometry(page, ".pool-card"))[0]?.width)
		.not.toBe(before.width);
	await expectSquarePool(page);

	for (const [storyId, selector] of [
		["tosklight-windows-pools--groups-narrow-short", ".group-card"],
		["tosklight-windows-pools--groups-wide-tall", ".group-card"],
		["tosklight-windows-pools--presets-narrow-short", ".preset-card"],
		["tosklight-windows-pools--presets-wide-tall", ".preset-card"],
		["tosklight-windows-cuelists-and-cues--pool-narrow-short", ".cuelist-card"],
		["tosklight-windows-cuelists-and-cues--pool-wide-tall", ".cuelist-card"],
	] as const) {
		await page.goto(`/iframe.html?id=${storyId}&viewMode=story`);
		await expectSquarePool(page, selector);
	}
});

test("the Cuelist pool uses its target minimum and fills the available width", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1280, height: 760 });
	await page.goto(
		"/iframe.html?id=tosklight-windows-cuelists-and-cues--pool-wide-tall&viewMode=story",
	);
	const grid = page.locator(".cuelist-pool-grid");
	const firstCard = page.locator(".cuelist-card").first();
	await expect(grid).toHaveCSS("--grid-cell-min", "100px");
	await expect(firstCard).toBeVisible();
	const geometry = await grid.evaluate((node) => {
		const style = getComputedStyle(node);
		const first = node.firstElementChild?.getBoundingClientRect();
		const contentWidth =
			node.clientWidth -
			Number.parseFloat(style.paddingLeft) -
			Number.parseFloat(style.paddingRight);
		const gap = Number.parseFloat(style.columnGap);
		const minimum = Number.parseFloat(
			style.getPropertyValue("--grid-cell-min"),
		);
		const columns = first
			? Array.from(node.children).filter(
					(child) =>
						Math.abs(child.getBoundingClientRect().top - first.top) < 1,
				).length
			: 0;
		return {
			cardWidth: first?.width ?? 0,
			columns,
			contentWidth,
			gap,
			minimum,
		};
	});
	expect(geometry.cardWidth).toBeGreaterThanOrEqual(geometry.minimum - 1);
	expect(
		geometry.columns * geometry.cardWidth +
			(geometry.columns - 1) * geometry.gap,
	).toBeCloseTo(geometry.contentWidth, 0);
	expect(
		(geometry.columns + 1) * geometry.minimum + geometry.columns * geometry.gap,
	).toBeGreaterThan(geometry.contentWidth);
});

test("configured colors and derived or frozen states use complete outlines and readable markers", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tables-and-grids-pools-generic-pool-window--every-card-state&viewMode=story",
	);
	const configured = page
		.getByRole("button", { name: /Configured color/u })
		.first();
	const borders = await configured.evaluate((card) => {
		const style = getComputedStyle(card);
		return [
			style.borderTopColor,
			style.borderRightColor,
			style.borderBottomColor,
			style.borderLeftColor,
		];
	});
	expect(new Set(borders).size).toBe(1);
	await expect(configured.getByLabel(/Configured color/u)).toBeVisible();
	await expect(page.getByLabel("Derived state")).toHaveText("Derived");
	await expect(page.getByLabel("Frozen state")).toHaveText("Frozen");
	for (const card of await page
		.locator(".pool-card.derived, .pool-card.frozen")
		.all()) {
		const widths = await card.evaluate((element) => {
			const style = getComputedStyle(element);
			return [
				style.borderTopWidth,
				style.borderRightWidth,
				style.borderBottomWidth,
				style.borderLeftWidth,
			];
		});
		expect(new Set(widths).size).toBe(1);
	}

	await page.goto(
		"/iframe.html?id=tosklight-windows-pools--groups-status-markers&viewMode=story",
	);
	await expect(page.getByLabel(/Derived state/u)).toBeVisible();
	await expect(page.getByLabel(/Frozen state/u)).toBeVisible();
});

test("production pool cards use dashed empty cards and semantic workflow colors", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tables-and-grids-pools-production-pool-cards--scaling-and-every-state&viewMode=story",
	);
	const empty = page.getByRole("button", { name: /Empty/u });
	const recordTarget = page.getByRole("button", { name: /Record here/u });
	const updateTarget = page.locator(".pool-card.update-target").first();
	const setTarget = page.locator(".pool-card.set-target").first();
	await expect(empty).toBeEnabled();
	await expect(recordTarget).toBeEnabled();
	await expect(empty).toHaveCSS("border-top-style", "dashed");
	await expect(empty).toHaveCSS("opacity", "1");
	await expect(recordTarget.locator(".pool-card-workflow")).toHaveText(
		"Record",
	);
	await expect(updateTarget.locator(".pool-card-workflow")).toHaveText(
		"Update",
	);
	await expect(setTarget.locator(".pool-card-workflow")).toHaveText("Set");
	const [recordColor, updateColor, setColor] = await Promise.all(
		[recordTarget, updateTarget, setTarget].map((card) =>
			card.evaluate((element) => getComputedStyle(element).borderTopColor),
		),
	);
	expect(new Set([recordColor, updateColor, setColor]).size).toBe(3);
	await expect(page.getByRole("button", { name: /Disabled/u })).toHaveCount(0);
});

test("pool names wrap inside the top-left region and both filled treatments are available", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tables-and-grids-pools-production-pool-cards--scaling-and-every-state&viewMode=story",
	);
	const longName = page.getByText(
		"Front Wash With A Deliberately Long Operator Name",
	);
	await expect(longName).toBeVisible();
	const wrapping = await longName.evaluate((element) => {
		const style = getComputedStyle(element);
		const card = element.closest(".pool-card")?.getBoundingClientRect();
		const name = element.getBoundingClientRect();
		return {
			hyphens: style.hyphens,
			overflowWrap: style.overflowWrap,
			inside:
				Boolean(card) &&
				name.left >= (card?.left ?? 0) &&
				name.right <= (card?.right ?? 0),
		};
	});
	expect(wrapping.hyphens).toBe("auto");
	expect(wrapping.overflowWrap).toBe("break-word");
	expect(wrapping.inside).toBe(true);

	const tintedBackground = await page
		.getByText("Blue", { exact: true })
		.locator("xpath=ancestor::button[contains(@class, 'pool-card')]")
		.evaluate((element) => getComputedStyle(element).backgroundColor);
	await page.goto(
		"/iframe.html?id=tables-and-grids-pools-production-pool-cards--outline-only-filled-cards&viewMode=story",
	);
	const outlineBackground = await page
		.getByText("Blue", { exact: true })
		.locator("xpath=ancestor::button[contains(@class, 'pool-card')]")
		.evaluate((element) => getComputedStyle(element).backgroundColor);
	expect(outlineBackground).not.toBe(tintedBackground);
});

test("legacy application CSS cannot impose rectangular full-pool dimensions", () => {
	const css = [
		"apps/light-desktop/src/styles/control-surface.css",
		"apps/light-desktop/src/styles/shared-controls.css",
		"apps/light-desktop/src/window-kit.css",
	]
		.map((file) => readFileSync(`${repositoryRoot}/${file}`, "utf8"))
		.join("\n");
	for (const selector of [
		"group-pool-window",
		"preset-pool-window",
		"cuelist-window",
	]) {
		expect(css).not.toMatch(
			new RegExp(
				`${selector}[^\\{]*\\{[^\\}]*(?:grid-auto-rows:\\s*94px|(?:min-)?(?:width|height):\\s*(?:132|142)px)`,
				"u",
			),
		);
	}
	expect(css).not.toMatch(
		/\.card-pool\s*\{[^}]*(?:grid-auto-rows:\s*94px|(?:min-)?(?:width|height):\s*(?:132|142)px)/u,
	);
	expect(css).toMatch(
		/\.group-strip\s+\.group-card\s*\{[^}]*(?:width|height)\s*:\s*(?:124|142|94)px/u,
	);
});

test("the application Virtual Playbacks adapter uses a stable faderless pool grid across pages", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-virtual-playbacks--sparse-grid&viewMode=story",
	);
	await expect(page.locator(".virtual-playback-box")).toHaveCount(12);
	await expect(page.locator(".playback-fader-bank")).toHaveCount(0);
	await expect(page.locator('input[type="range"]')).toHaveCount(0);
	await expect(
		page.getByRole("button", { name: /cell 2 empty/u }),
	).toBeVisible();

	await page.goto(
		"/iframe.html?id=tosklight-virtual-playbacks--page-switching&viewMode=story",
	);
	const first = page.locator('[data-grid-position="0"]');
	await expect(first).toContainText("Main");
	await expect(first).toHaveAttribute("data-page", "1");
	await page.getByRole("button", { name: "Next page" }).click();
	await expect(first).toContainText("House");
	await expect(first).toHaveAttribute("data-page", "2");
	await expect(first).toHaveAttribute("data-grid-position", "0");
});

test("Virtual Playback GO and held actions retain click versus press-release semantics", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-virtual-playbacks--sparse-grid&viewMode=story",
	);
	await page.getByRole("button", { name: /cell 1 Main/u }).click();
	await expect(page.getByRole("status")).toHaveText("Action 1001");

	await page.goto(
		"/iframe.html?id=tosklight-virtual-playbacks--held-flash-and-swap&viewMode=story",
	);
	const flash = page.getByRole("button", { name: /cell 1 Bump/u });
	const bounds = await flash.boundingBox();
	expect(bounds).not.toBeNull();
	await page.mouse.move(
		bounds!.x + bounds!.width / 2,
		bounds!.y + bounds!.height / 2,
	);
	await page.mouse.down();
	await expect(page.getByRole("status")).toHaveText("Pressed 1001");
	await page.mouse.up();
	await expect(page.getByRole("status")).toHaveText("Released 1001");
});

test("the application Virtual Playbacks stories cover adapter-owned targeting and availability states", async ({
	page,
}) => {
	for (const [story, selector, className] of [
		["configuration-state", '[data-grid-position="0"]', "configuration-armed"],
		["update-state", '[data-grid-position="0"]', "update-target"],
		["exclusion-zone-state", '[data-grid-position="3"]', "exclusion-selected"],
	] as const) {
		await page.goto(
			`/iframe.html?id=tosklight-virtual-playbacks--${story}&viewMode=story`,
		);
		await expect(page.locator(selector)).toHaveClass(
			new RegExp(className, "u"),
		);
	}

	await expect(page.locator('[data-grid-position="3"]')).toHaveAttribute(
		"data-exclusion-zones",
		"Front alternates",
	);
	await page.goto(
		"/iframe.html?id=tosklight-virtual-playbacks--sparse-large-grid&viewMode=story",
	);
	const largeGrid = page.locator(".virtual-playback-grid");
	await expect(largeGrid).toHaveAttribute("data-logical-cells", "300");
	expect(await largeGrid.locator(".virtual-playback-box").count()).toBeLessThan(
		200,
	);
	await expect(
		page.getByRole("button", { name: /cell 1 Main/u }),
	).toBeEnabled();

	await page.goto(
		"/iframe.html?id=tosklight-virtual-playbacks--pinned-page&viewMode=story",
	);
	await expect(
		page.getByRole("button", { name: /playback 1301 page 2 cell 1 House/u }),
	).toBeVisible();
	await page.goto(
		"/iframe.html?id=tosklight-virtual-playbacks--overlapping-zones&viewMode=story",
	);
	await expect(page.locator('[data-grid-position="3"]')).toHaveAttribute(
		"data-exclusion-zones",
		"Front alternates, Bump alternates",
	);
	await page.goto(
		"/iframe.html?id=tosklight-virtual-playbacks--hidden-membership&viewMode=story",
	);
	await expect(
		page.getByText("Virtual Playback 1301 remains a saved member on page 2.", {
			exact: true,
		}),
	).toHaveText("Virtual Playback 1301 remains a saved member on page 2.");
	await page.goto(
		"/iframe.html?id=tosklight-virtual-playbacks--zone-error-state&viewMode=story",
	);
	await expect(page.getByRole("alert")).toHaveText(
		"Zone revision changed on another screen",
	);

	await page.goto(
		"/iframe.html?id=tables-and-grids-virtual-playback-grid--every-state&viewMode=story",
	);
	const empty = page.locator('[data-grid-position="1"]');
	const unavailable = page.locator('[data-grid-position="2"]');
	await expect(empty).toHaveAttribute("data-availability", "empty");
	await expect(unavailable).toHaveAttribute("data-availability", "unavailable");
	await expect(empty).toBeEnabled();
	await expect(unavailable).toBeDisabled();
	const vacantAppearance = async (
		element: import("@playwright/test").Locator,
	) =>
		element.evaluate((node) => {
			const style = getComputedStyle(node);
			return {
				background: style.backgroundColor,
				borderStyle: style.borderStyle,
				color: style.color,
				filter: style.filter,
				opacity: style.opacity,
			};
		});
	expect(await vacantAppearance(unavailable)).toEqual(
		await vacantAppearance(empty),
	);
});

test("Virtual Playback cards use outline, full-fill, edge-status, and artwork hierarchy", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-virtual-playbacks--running-transition&viewMode=story",
	);
	const card = page.locator('[data-grid-position="0"]');
	const inactive = await card.evaluate((element) => {
		const style = getComputedStyle(element);
		return {
			background: style.backgroundColor,
			left: style.borderLeftColor,
			top: style.borderTopColor,
		};
	});
	expect(inactive.left).toBe(inactive.top);
	await page.getByRole("button", { name: "Toggle running" }).click();
	await expect(card).toHaveClass(/running/u);
	await expect(card.locator(".pool-card-status")).toHaveText("Running");
	await expect
		.poll(() =>
			card.evaluate((element) => getComputedStyle(element).backgroundColor),
		)
		.not.toBe(inactive.background);
	const running = await card.evaluate((element) => {
		const style = getComputedStyle(element);
		return {
			background: style.backgroundColor,
			border: style.borderTopColor,
		};
	});
	expect(running.border).toBe(inactive.top);
	await page.getByRole("button", { name: "Toggle running" }).click();
	await expect(card).not.toHaveClass(/running/u);
	expect(
		await card.evaluate((element) => getComputedStyle(element).backgroundColor),
	).not.toBe(running.background);

	await page.goto(
		"/iframe.html?id=tosklight-virtual-playbacks--icon-and-image-artwork&viewMode=story",
	);
	const icon = page.locator('[data-grid-position="0"] .pool-card-icon');
	const image = page.locator('[data-grid-position="1"] .pool-card-image');
	await expect(icon).toBeVisible();
	await expect(image).toBeVisible();
	const imageCard = page.locator('[data-grid-position="1"]');
	const [imageBounds, cardBounds] = await Promise.all([
		image.boundingBox(),
		imageCard.boundingBox(),
	]);
	expect(imageBounds).not.toBeNull();
	expect(cardBounds).not.toBeNull();
	expect((imageBounds?.x ?? 0) + (imageBounds?.width ?? 0)).toBeLessThanOrEqual(
		(cardBounds?.x ?? 0) + (cardBounds?.width ?? 0),
	);
	expect(
		(imageBounds?.y ?? 0) + (imageBounds?.height ?? 0),
	).toBeLessThanOrEqual((cardBounds?.y ?? 0) + (cardBounds?.height ?? 0));

	for (const [story, label] of [
		["configuration-state", "Configure Playback"],
		["update-state", "Update"],
	] as const) {
		await page.goto(
			`/iframe.html?id=tosklight-virtual-playbacks--${story}&viewMode=story`,
		);
		await expect(page.locator(".pool-card-workflow").first()).toHaveText(label);
	}
});

test("nested modal story keeps deterministic stack order and top-only Escape", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-window-system-modal-layer--three-deep&viewMode=story",
	);
	await page.getByRole("button", { name: "Open nested modal" }).click();
	await page.getByRole("button", { name: "Open third modal" }).click();
	await expect(page.locator('[data-modal-id="window-modal"]')).toHaveCSS(
		"z-index",
		"3000",
	);
	await expect(page.locator('[data-modal-id="nested-modal"]')).toHaveCSS(
		"z-index",
		"3010",
	);
	await expect(page.locator('[data-modal-id="third-modal"]')).toHaveCSS(
		"z-index",
		"3020",
	);
	await page.keyboard.press("Escape");
	await expect(page.getByRole("dialog", { name: "Third modal" })).toBeHidden();
	await expect(
		page.getByRole("dialog", { name: "Nested modal" }),
	).toBeVisible();
});

test("modal close policies, title configuration, and programmatic close are interactive", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-window-system-modal-layer--close-policies&viewMode=story",
	);
	const policyDialog = page.getByRole("dialog", { name: "Policy modal" });
	await expect(policyDialog).toBeVisible();
	await expect(page.getByRole("button", { name: "Close modal" })).toHaveCount(
		0,
	);
	await page.keyboard.press("Escape");
	await expect(policyDialog).toBeVisible();
	await page
		.locator('[data-modal-id="policy-modal"]')
		.click({ position: { x: 2, y: 2 } });
	await expect(policyDialog).toBeVisible();

	await page.goto(
		"/iframe.html?id=tosklight-window-system-modal-layer--title-bar-configuration&viewMode=story",
	);
	await page.getByRole("tab", { name: "Advanced" }).click();
	await expect(page.getByLabel("Active modal tab")).toHaveText("advanced");
	await expect(
		page.getByRole("button", { name: "Close patch fixtures" }),
	).toBeVisible();
	const search = page.getByRole("textbox", { name: "Search Patch fixtures" });
	await expect(search).toBeVisible();
	await search.fill("spot");
	await page.getByRole("button", { name: "Search settings" }).click();
	const settings = page.getByRole("dialog", {
		name: "Patch fixtures search settings",
	});
	await expect(settings).toBeVisible();
	await settings.getByText("Favorites only", { exact: true }).click();
	await settings.getByRole("button", { name: "Clear settings" }).click();
	await expect(search).toHaveValue("spot");
	await page.keyboard.press("Escape");
	await expect(settings).toBeHidden();
	await expect(
		page.getByRole("dialog", { name: "Configured title bar" }),
	).toBeVisible();
	await page.getByRole("button", { name: "Clear search" }).click();
	await expect(search).toHaveValue("");

	await page.goto(
		"/iframe.html?id=tosklight-window-system-modal-layer--programmatic-close&viewMode=story",
	);
	await page.getByRole("button", { name: "Close target by ID" }).click();
	await expect(
		page.getByRole("dialog", { name: "Programmatic target" }),
	).toBeHidden();
	await expect(page.getByLabel("Programmatic modal state")).toHaveText(
		"Closed",
	);
});

test("configured search children share stack order, focus, form geometry, and dividers", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1280, height: 800 });
	await page.goto(
		"/iframe.html?id=tosklight-window-system-modal-layer--title-bar-configuration&viewMode=story",
	);
	const owner = page.locator('[data-modal-id="configured-title"]');
	const keyboardTrigger = page.getByRole("button", { name: "Open keyboard" });
	await keyboardTrigger.click();
	const inputLayer = page.locator(".ui-input-modal-layer");
	await expect(owner).toHaveCSS("z-index", "3000");
	await expect(inputLayer).toHaveCSS("z-index", "3010");
	await expect(owner).toHaveAttribute("data-modal-top", "false");
	await page.keyboard.press("Escape");
	await expect(inputLayer).toBeHidden();
	await expect(keyboardTrigger).toBeFocused();

	const settingsTrigger = page.getByRole("button", { name: "Search settings" });
	await settingsTrigger.click();
	const settings = page.getByRole("dialog", {
		name: "Patch fixtures search settings",
	});
	await expect(page.locator(".search-options-layer")).toHaveCSS(
		"z-index",
		"3010",
	);
	await expect(
		settings.getByRole("button", { name: /^(Apply|Save)$/u }),
	).toHaveCount(0);
	const switchField = settings.locator(".ui-form-field").filter({
		hasText: "Favorites only",
	});
	const [labelBox, switchBox, statesBox] = await Promise.all([
		switchField.locator(":scope > label").boundingBox(),
		switchField.locator(".ui-switch-track").boundingBox(),
		switchField.locator(".ui-switch-states").boundingBox(),
	]);
	expect(labelBox).not.toBeNull();
	expect(switchBox).not.toBeNull();
	expect(statesBox).not.toBeNull();
	await expect(switchField.locator(".ui-switch-state")).toHaveCount(2);
	await expect(switchField.locator(".ui-switch-state-off")).toHaveText(
		"All fixtures",
	);
	await expect(switchField.locator(".ui-switch-state-on")).toHaveText(
		"Favorites",
	);
	expect(Math.abs(labelBox!.y - switchBox!.y)).toBeLessThan(24);
	expect(switchBox!.x + switchBox!.width).toBeLessThanOrEqual(statesBox!.x);

	const nestedKeyboardTrigger = settings.getByRole("button", {
		name: "Open keyboard",
	});
	await nestedKeyboardTrigger.click();
	await expect(page.locator(".ui-input-modal-layer")).toHaveCSS(
		"z-index",
		"3020",
	);
	await page.keyboard.press("Escape");
	await expect(nestedKeyboardTrigger).toBeFocused();
	await page.keyboard.press("Escape");
	await expect(settings).toBeHidden();
	await expect(settingsTrigger).toBeFocused();

	for (let cycle = 0; cycle < 3; cycle += 1) {
		await keyboardTrigger.click();
		await expect(page.locator(".ui-input-modal-layer")).toHaveCSS(
			"z-index",
			"3010",
		);
		await page.keyboard.press("Escape");
		await expect(keyboardTrigger).toBeFocused();
	}

	const dividers = owner.locator(".ui-titlebar-search-divider");
	await expect(dividers).toHaveCount(2);
	for (const divider of await dividers.all()) {
		const geometry = await divider.evaluate((element) => {
			const bar = getComputedStyle(element);
			const line = getComputedStyle(element, "::after");
			return {
				width: Math.round(element.getBoundingClientRect().width),
				background: bar.backgroundColor,
				lineWidth: Number.parseFloat(line.width),
				lineColor: line.backgroundColor,
			};
		});
		expect(geometry.width).toBe(6);
		expect(geometry.background).not.toBe("rgba(0, 0, 0, 0)");
		expect(geometry.lineWidth).toBe(2);
		expect(geometry.lineColor).toContain("27");
		expect(geometry.lineColor).toContain("214");
		expect(geometry.lineColor).toContain("236");
	}

	await page.goto(
		"/iframe.html?id=tosklight-window-system-modal-layer--search-without-adjacent-buttons&viewMode=story",
	);
	await expect(page.locator(".ui-titlebar-search-divider")).toHaveCount(0);

	await page.goto(
		"/iframe.html?id=tosklight-window-system-modal-layer--window-title-bar-search&viewMode=story",
	);
	await expect(page.locator(".ui-titlebar-search-divider")).toHaveCount(1);
	await page.setViewportSize({ width: 620, height: 760 });
	await expect(page.locator(".ui-titlebar-search-divider")).toBeHidden();
	const searchBox = await page
		.locator(".ui-window-header-search")
		.boundingBox();
	const actionBox = await page
		.getByRole("button", { name: "Add fixture" })
		.boundingBox();
	expect(searchBox).not.toBeNull();
	expect(actionBox).not.toBeNull();
	expect(searchBox!.y).toBeGreaterThanOrEqual(actionBox!.y + actionBox!.height);
});

test("title-bar search dividers remain prominently two CSS pixels at DPR 1 and 2", async ({
	browser,
}) => {
	for (const deviceScaleFactor of [1, 2]) {
		const context = await browser.newContext({
			deviceScaleFactor,
			viewport: { width: 1280, height: 800 },
		});
		const dprPage = await context.newPage();
		await dprPage.goto(
			"/iframe.html?id=tosklight-window-system-modal-layer--title-bar-configuration&viewMode=story",
		);
		const divider = dprPage.locator(".ui-titlebar-search-divider").first();
		await expect(divider).toBeVisible();
		const cssWidth = await divider.evaluate((element) =>
			Number.parseFloat(getComputedStyle(element, "::after").width),
		);
		expect(cssWidth).toBeCloseTo(2, 5);
		await context.close();
	}
});

test("generic and Fixture Sheet tables retain a single row-separator owner at DPR 1 and 2", async ({
	browser,
}) => {
	for (const deviceScaleFactor of [1, 2]) {
		const context = await browser.newContext({
			deviceScaleFactor,
			viewport: { width: 1280, height: 800 },
		});
		const dprPage = await context.newPage();
		for (const story of [
			"tosklight-tables-data-table--primary",
			"tosklight-tables-fixture-sheet-table--step-selection",
		]) {
			await dprPage.goto(`/iframe.html?id=${story}&viewMode=story`);
			const table = dprPage.locator(".ui-data-table").first();
			await expect(table).toBeVisible();
			const geometry = await table.evaluate((element) => {
				const tableStyle = getComputedStyle(element);
				const rows = [
					...element.querySelectorAll<HTMLElement>(".ui-data-table-row"),
				].slice(0, 8);
				return {
					devicePixelRatio: window.devicePixelRatio,
					tableBackgroundImage: tableStyle.backgroundImage,
					tableBorderTop: tableStyle.borderTopWidth,
					tableBorderBottom: tableStyle.borderBottomWidth,
					rows: rows.map((row, index) => {
						const rowStyle = getComputedStyle(row);
						const cells = [
							...row.querySelectorAll<HTMLElement>(":scope > span"),
						];
						const bounds = row.getBoundingClientRect();
						const nextBounds = rows[index + 1]?.getBoundingClientRect();
						return {
							borderTop: rowStyle.borderTopWidth,
							borderBottom: rowStyle.borderBottomWidth,
							bottom: bounds.bottom,
							nextTop: nextBounds?.top,
							cellBorders: cells.map((cell) => {
								const cellStyle = getComputedStyle(cell);
								return [cellStyle.borderTopWidth, cellStyle.borderBottomWidth];
							}),
						};
					}),
				};
			});
			expect(geometry.devicePixelRatio).toBe(deviceScaleFactor);
			expect(geometry.tableBackgroundImage).toBe("none");
			expect(geometry.tableBorderTop).toBe("0px");
			expect(geometry.tableBorderBottom).toBe("0px");
			for (const [index, row] of geometry.rows.entries()) {
				expect(row.borderTop).toBe("0px");
				expect(row.borderBottom).toBe("1px");
				expect(
					row.cellBorders.every(
						([top, bottom]) => top === "0px" && bottom === "0px",
					),
				).toBe(true);
				if (index < geometry.rows.length - 1)
					expect(row.bottom).toBeCloseTo(row.nextTop ?? Number.NaN, 5);
			}
			expect((await table.screenshot()).byteLength).toBeGreaterThan(2_000);
		}
		await context.close();
	}
});

test("window and modal chrome embed borderless searches at their title-bar heights", async ({
	page,
}) => {
	const measure = async () =>
		page.locator(".console-search").evaluate((element) => {
			const control = element.querySelector(".ui-text-control");
			const controlStyle = control ? getComputedStyle(control) : null;
			const clear = element.querySelector(
				".ui-input-clear",
			) as HTMLElement | null;
			return {
				controlWidth: Math.round(control?.getBoundingClientRect().width ?? 0),
				controlHeight: Math.round(control?.getBoundingClientRect().height ?? 0),
				borderWidths: controlStyle
					? [
							controlStyle.borderTopWidth,
							controlStyle.borderRightWidth,
							controlStyle.borderBottomWidth,
							controlStyle.borderLeftWidth,
						]
					: [],
				borderRadius: controlStyle?.borderRadius,
				inputHeight: Math.round(
					element.querySelector("input")?.getBoundingClientRect().height ?? 0,
				),
				clearHeight: Math.round(clear?.getBoundingClientRect().height ?? 0),
				clearCenterOffset: clear
					? Math.round(
							clear.getBoundingClientRect().top +
								clear.getBoundingClientRect().height / 2 -
								(control?.getBoundingClientRect().top ?? 0) -
								(control?.getBoundingClientRect().height ?? 0) / 2,
						)
					: null,
				usesStandardClass: element.classList.contains("console-search"),
			};
		});
	await page.goto(
		"/iframe.html?id=tosklight-window-system-window-frame--primary&viewMode=story",
	);
	const windowSearch = await measure();
	await page.goto(
		"/iframe.html?id=tosklight-window-system-modal-layer--title-bar-configuration&viewMode=story",
	);
	const modalSearch = await measure();
	expect(windowSearch.borderWidths).toEqual(["0px", "0px", "0px", "0px"]);
	expect(modalSearch.borderWidths).toEqual(["0px", "0px", "0px", "0px"]);
	expect(windowSearch.borderRadius).toBe("0px");
	expect(modalSearch.borderRadius).toBe("0px");
	expect(windowSearch.inputHeight).toBe(windowSearch.controlHeight);
	expect(windowSearch.clearHeight).toBe(windowSearch.controlHeight);
	expect(modalSearch.inputHeight).toBe(modalSearch.controlHeight);
	expect(modalSearch.clearHeight).toBe(modalSearch.controlHeight);
	expect(modalSearch.controlHeight).toBeGreaterThan(windowSearch.controlHeight);
	expect(windowSearch.clearCenterOffset).toBe(0);
	expect(modalSearch.clearCenterOffset).toBe(0);
	expect(windowSearch.controlWidth).toBeGreaterThan(0);
	expect(modalSearch.controlWidth).toBeGreaterThan(0);
	const search = page.getByRole("textbox", { name: "Search Patch fixtures" });
	await search.fill("");
	const emptyWidth = (await measure()).controlWidth;
	await search.fill("spot");
	expect((await measure()).controlWidth).toBe(emptyWidth);
	await page.getByRole("button", { name: "Open keyboard" }).click();
	const inputDialog = page.getByRole("dialog", {
		name: "Search Patch fixtures",
	});
	await expect(
		inputDialog.locator(".modal-value-leading-icon svg"),
	).toBeVisible();
	const [iconBox, valueBox] = await Promise.all([
		inputDialog.locator(".modal-value-leading-icon").boundingBox(),
		inputDialog.locator(".modal-caret-value").boundingBox(),
	]);
	expect(iconBox).not.toBeNull();
	expect(valueBox).not.toBeNull();
	expect(iconBox?.x).toBeLessThan(valueBox?.x ?? 0);
});

test("modal title details stay grouped and vertically centered with their heading", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-window-system-modal-layer--close-policies&viewMode=story",
	);
	const titleCopy = page.locator(".ui-modal-title-copy");
	await expect(titleCopy).toHaveCSS("justify-content", "center");
	const [copyBox, headingBox, detailsBox] = await Promise.all([
		titleCopy.boundingBox(),
		titleCopy.locator(".ui-modal-title-heading").boundingBox(),
		titleCopy.locator(".ui-modal-title-details").boundingBox(),
	]);
	expect(copyBox).not.toBeNull();
	expect(headingBox).not.toBeNull();
	expect(detailsBox).not.toBeNull();
	expect(detailsBox?.y).toBeGreaterThan(
		(headingBox?.y ?? 0) + (headingBox?.height ?? 0),
	);
	const contentTop = headingBox?.y ?? 0;
	const contentBottom = (detailsBox?.y ?? 0) + (detailsBox?.height ?? 0);
	expect(
		Math.abs(
			contentTop -
				(copyBox?.y ?? 0) -
				((copyBox?.height ?? 0) - (contentBottom - contentTop)) / 2,
		),
	).toBeLessThanOrEqual(1);
});

test("desktop story uses the real 24 × 18 non-overlapping geometry", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=window-system-desktop-grid-manager--constrained-placement&viewMode=story",
	);
	const paneLocator = page.locator(".desk-pane");
	await expect(paneLocator).toHaveCount(3);
	const panes = await paneLocator.evaluateAll((elements) =>
		elements.map((element) => {
			const pane = element as HTMLElement;
			return {
				id: pane.dataset.paneId,
				x: Number(pane.dataset.gridColumn),
				y: Number(pane.dataset.gridRow),
				width: Number(pane.dataset.gridWidth),
				height: Number(pane.dataset.gridHeight),
			};
		}),
	);
	expect(panes).toHaveLength(3);
	for (const pane of panes) {
		expect(pane.x).toBeGreaterThanOrEqual(1);
		expect(pane.y).toBeGreaterThanOrEqual(1);
		expect(pane.x + pane.width - 1).toBeLessThanOrEqual(24);
		expect(pane.y + pane.height - 1).toBeLessThanOrEqual(18);
	}
	for (let left = 0; left < panes.length; left += 1) {
		for (let right = left + 1; right < panes.length; right += 1) {
			expect(
				panes[left].x + panes[left].width <= panes[right].x ||
					panes[right].x + panes[right].width <= panes[left].x ||
					panes[left].y + panes[left].height <= panes[right].y ||
					panes[right].y + panes[right].height <= panes[left].y,
			).toBe(true);
		}
	}
});

test("desktop panes drag, resize, maximize, and request an empty-grid placement", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1600, height: 1000 });
	await page.goto(
		"/iframe.html?id=window-system-desktop-grid-manager--drag-and-resize&viewMode=story",
	);
	const grid = page.locator(".desk-grid");
	const group = page.locator('[data-pane-id="groups"]');
	await expect(group).toBeVisible();
	const groupHeader = group.locator(".pane-drag-handle");
	const gridBounds = await grid.boundingBox();
	const headerBounds = await groupHeader.boundingBox();
	expect(gridBounds).not.toBeNull();
	expect(headerBounds).not.toBeNull();
	await page.mouse.move(
		headerBounds!.x + headerBounds!.width / 2,
		headerBounds!.y + headerBounds!.height / 2,
	);
	await page.mouse.down();
	await page.mouse.move(
		gridBounds!.x + gridBounds!.width * (8.5 / 24),
		gridBounds!.y + gridBounds!.height * (10.5 / 18),
		{ steps: 4 },
	);
	await page.mouse.up();
	await expect(group).toHaveAttribute("data-grid-column", "9");

	const resize = group.locator(".pane-resize-handle");
	const resizeBounds = await resize.boundingBox();
	expect(resizeBounds).not.toBeNull();
	await page.mouse.move(
		resizeBounds!.x + resizeBounds!.width - 4,
		resizeBounds!.y + resizeBounds!.height - 4,
	);
	await page.mouse.down();
	await page.mouse.move(
		gridBounds!.x + gridBounds!.width * (18 / 24),
		gridBounds!.y + gridBounds!.height * (18 / 18) - 2,
		{ steps: 4 },
	);
	await page.mouse.up();
	await expect(group).toHaveAttribute("data-grid-width", "10");

	await page.goto(
		"/iframe.html?id=window-system-desktop-grid-manager--maximized&viewMode=story",
	);
	const maximized = page.getByRole("region", { name: "Fixture Sheet pane" });
	await expect(maximized).toHaveAttribute("aria-expanded", "true");
	await expect(maximized.locator(".pane-resize-handle")).toHaveCount(0);

	await page.goto(
		"/iframe.html?id=window-system-desktop-grid-manager--empty-grid&viewMode=story",
	);
	await page.getByRole("button", { name: /24 × 18 desktop grid/u }).click({
		position: { x: 760, y: 380 },
	});
	await expect(page.getByLabel("Requested grid rectangle")).not.toHaveText(
		"No window requested",
	);
});

test("button variants expose animated loading, larger icon-only controls, icons, and left alignment", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-controls-button--primary&viewMode=story",
	);
	const loading = page.getByRole("button", { name: "Loading" });
	await expect(loading).toHaveAttribute("aria-busy", "true");
	await expect(loading.locator(".ui-spinner")).toHaveCSS(
		"animation-name",
		"ui-spin",
	);
	const iconOnly = page.getByRole("button", { name: "Settings" });
	const iconOnlySize = await iconOnly.evaluate((button) =>
		Number.parseFloat(getComputedStyle(button).fontSize),
	);
	const regularSize = await page
		.getByRole("button", { name: "Secondary", exact: true })
		.evaluate((button) => Number.parseFloat(getComputedStyle(button).fontSize));
	expect(iconOnlySize).toBeGreaterThan(regularSize);

	const iconSection = page
		.getByRole("heading", { name: "Label and icon content" })
		.locator("..");
	await expect(iconSection.locator(".ui-button")).toHaveCount(6);
	await expect(iconSection.locator(".ui-button-icon")).toHaveCount(6);
	for (const variant of [
		"primary",
		"secondary",
		"ghost",
		"danger",
		"success",
		"warning",
	]) {
		await expect(iconSection.locator(`.ui-button.ui-${variant}`)).toHaveCount(
			1,
		);
	}

	const alignedSection = page
		.getByRole("heading", { name: "Left-aligned full-width controls" })
		.locator("..");
	await expect(
		alignedSection.locator(".ui-button.is-left-aligned"),
	).toHaveCount(6);
	for (const button of await alignedSection.locator(".ui-button").all()) {
		await expect(button).toHaveCSS("justify-content", "flex-start");
	}
});

test("direct value opens a modal with the touch fader left of the number pad", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=controls-faders-vertical-touch-fader--direct-value-button&viewMode=story",
	);
	const trigger = page.locator(".touch-value-button > .ui-button");
	await expect(trigger).toHaveAttribute("aria-haspopup", "dialog");
	await expect(trigger).toHaveAttribute("aria-expanded", "false");
	await expect(page.getByRole("dialog")).toHaveCount(0);
	await trigger.click();
	await expect(trigger).toHaveAttribute("aria-expanded", "true");
	const dialog = page.getByRole("dialog", { name: "Grand Master value" });
	await expect(dialog).toBeVisible();
	await expect(dialog.locator("..")).toHaveCSS("position", "fixed");
	const fader = dialog.locator(".modal-number-editor-fader");
	await expect(fader.locator(".vertical-touch-fader")).toHaveCount(1);
	await expect(fader.locator(".vertical-touch-fader-actions")).toHaveCount(0);
	const valueRow = dialog.locator(".modal-number-value");
	const keypad = dialog.getByLabel("Number input keypad");
	const firstKey = keypad.locator(".ui-button").first();
	const faderBox = await fader.boundingBox();
	const valueBox = await valueRow.boundingBox();
	const keypadBox = await keypad.boundingBox();
	const firstKeyBox = await firstKey.boundingBox();
	expect(faderBox).not.toBeNull();
	expect(valueBox).not.toBeNull();
	expect(keypadBox).not.toBeNull();
	expect(
		(faderBox?.x ?? Number.POSITIVE_INFINITY) + (faderBox?.width ?? 0),
	).toBeLessThan(keypadBox?.x ?? Number.NEGATIVE_INFINITY);
	expect(faderBox?.y).toBeCloseTo(valueBox?.y ?? Number.NaN, 0);
	expect((faderBox?.y ?? 0) + (faderBox?.height ?? 0)).toBeCloseTo(
		(keypadBox?.y ?? 0) + (keypadBox?.height ?? 0),
		0,
	);
	expect(faderBox?.width).toBeCloseTo((firstKeyBox?.width ?? 64) * 2 + 8, 0);
	expect(valueBox?.width).toBeCloseTo(keypadBox?.width ?? Number.NaN, 0);
	const backspaceBox = await keypad
		.getByRole("button", { name: "⌫" })
		.boundingBox();
	const enterBox = await keypad
		.getByRole("button", { name: "ENTER" })
		.boundingBox();
	expect(backspaceBox?.width).toBeGreaterThan(firstKeyBox?.width ?? Infinity);
	expect(enterBox?.width).toBeGreaterThan(firstKeyBox?.width ?? Infinity);
	await page.keyboard.press("7");
	await expect(keypad.getByRole("button", { name: "7" })).toHaveAttribute(
		"data-keyboard-pressed",
		"true",
	);
	const slider = dialog.getByRole("slider", { name: "Grand Master fader" });
	const sliderBox = await slider.boundingBox();
	expect(sliderBox).not.toBeNull();
	if (!sliderBox) return;

	await page.mouse.click(sliderBox.x + sliderBox.width / 2, sliderBox.y + 6);
	await expect(slider).toHaveValue("100");
	await page.mouse.click(
		sliderBox.x + sliderBox.width / 2,
		sliderBox.y + sliderBox.height / 2,
	);
	await expect(slider).toHaveValue("50");

	await page.mouse.move(sliderBox.x + 6, sliderBox.y + sliderBox.height / 2);
	await page.mouse.down();
	await page.mouse.move(
		sliderBox.x + sliderBox.width - 6,
		sliderBox.y + sliderBox.height / 2,
	);
	await page.mouse.up();
	await expect(slider).toHaveValue("50");

	await page.mouse.click(
		sliderBox.x + sliderBox.width / 2,
		sliderBox.y + sliderBox.height - 6,
	);
	await expect(slider).toHaveValue("0");
	await dialog
		.getByRole("button", { name: "Close Grand Master value" })
		.click();
	let unsaved = page.getByRole("dialog", {
		name: "Unsaved Grand Master value changes",
	});
	await expect(unsaved).toBeVisible();
	await unsaved.getByText("Stay in modal", { exact: true }).click();
	await expect(unsaved).toHaveCount(0);
	await expect(dialog).toBeVisible();
	await page.keyboard.press("Escape");
	unsaved = page.getByRole("dialog", {
		name: "Unsaved Grand Master value changes",
	});
	await expect(unsaved).toBeVisible();
	await unsaved.getByRole("button", { name: "Discard changes" }).click();
	await expect(dialog).toHaveCount(0);
	await expect(trigger).toContainText("42.0%");
});

for (const storyId of [
	"tosklight-controls-button--primary",
	"tosklight-integration-input-modal-surfaces--number-pad",
	"controls-faders-vertical-touch-fader--software",
	"controls-encoders--individual-touch",
	"tosklight-virtual-playbacks--narrow-touch",
	"tables-and-grids-pools-generic-pool-window--narrow",
	"tosklight-window-system-modal-layer--close-policies",
	"tosklight-window-system-modal-layer--title-bar-configuration",
]) {
	test(`${storyId} renders at a touch-oriented viewport`, async ({ page }) => {
		await page.setViewportSize({ width: 430, height: 844 });
		await page.goto(`/iframe.html?id=${storyId}&viewMode=story`);
		await page.evaluate(() => document.fonts.ready);
		const shot = page.locator("[data-documentation-shot]");
		await expect(shot).toBeVisible();
		expect((await shot.screenshot()).byteLength).toBeGreaterThan(2_000);
	});
}

test("named application windows use production Fixture, Cuelist, Patch, and Setup views", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-windows-fixture-sheet--selected-and-active-steps&viewMode=story",
	);
	await expect(page.getByText("Fixture Sheet", { exact: true })).toBeVisible();
	await expect(page.locator(".ui-data-table-row:not(.empty)")).toHaveCount(5);
	await expect(
		page.getByText("Stage Left Mover · Master", { exact: true }),
	).toBeVisible();
	await expect(page.locator(".ui-data-table-row.selected")).toHaveCount(2);
	await expect(page.locator(".fixture-sheet-icon img")).toHaveCount(4);
	const fixtureSheetIconSources = await page
		.locator(".fixture-sheet-icon img")
		.evaluateAll((icons) =>
			icons.map((icon) => decodeURIComponent((icon as HTMLImageElement).src)),
		);
	expect(
		fixtureSheetIconSources.some((source) =>
			source.includes("fixture type profile dimmer lamp"),
		),
	).toBe(true);
	expect(
		fixtureSheetIconSources.some((source) =>
			source.includes("fixture type led wash moving light lenses"),
		),
	).toBe(true);
	await expect(page.locator(".vertical-meter")).toHaveCount(4);
	await expect(page.locator(".color-dot")).toHaveCount(5);
	await expect(page.locator(".position-glyph")).toHaveCount(4);
	expect(await page.getByRole("columnheader").allTextContents()).toEqual([
		"ID",
		"Icon",
		"Name / type",
		"Intensity",
		"Color",
		"Position",
		"Beam",
		"Shapers",
		"Focus",
		"Control",
		"Media",
	]);

	await page.goto(
		"/iframe.html?id=tosklight-windows-cuelists-and-cues--pool&viewMode=story",
	);
	await expect(page.getByText("Cuelist Pool", { exact: true })).toBeVisible();
	await expect(page.getByText("Main Sequence", { exact: true })).toBeVisible();
	await page.goto(
		"/iframe.html?id=tosklight-windows-cuelists-and-cues--cues-with-properties&viewMode=story",
	);
	await expect(
		page.getByText(/Cuelist View · Cuelist 1 · Main Sequence/u),
	).toBeVisible();
	await expect(page.getByRole("row")).toHaveCount(9);
	await expect(page.getByText("Opening Look", { exact: true })).toBeVisible();
	await expect(page.locator(".cue-properties")).toBeVisible();
	await page.goto(
		"/iframe.html?id=tosklight-windows-cuelists-and-cues--fixed-cues-unavailable&viewMode=story",
	);
	await expect(
		page.getByText("Fixed Cuelist is unavailable", { exact: true }),
	).toBeVisible();

	await page.goto(
		"/iframe.html?id=tosklight-windows-patch--empty-patch&viewMode=story",
	);
	await expect(page.getByText("Show Patch", { exact: true })).toBeVisible();
	await expect(page.locator(".patch-table")).toBeVisible();
	await expect(
		page.getByText("No fixtures in this layer.", { exact: true }),
	).toBeVisible();
	await page.goto(
		"/iframe.html?id=tosklight-windows-patch--filled-patch&viewMode=story",
	);
	await expect(
		page.getByText("10 fixtures · 3 layers", { exact: true }),
	).toBeVisible();
	await expect(
		page.getByText("Front Fresnel 1", { exact: true }),
	).toBeVisible();
	await expect(page.getByText("Front Wash 4", { exact: true })).toBeVisible();
	await expect(
		page.getByText("Front Blinder 1", { exact: true }),
	).toBeVisible();
	await expect(
		page.getByText("Front Blinder 4", { exact: true }),
	).toBeVisible();
	await expect(page.getByText("Stage ACL 1", { exact: true })).toBeVisible();
	await expect(page.getByText("Stage ACL 8", { exact: true })).toBeVisible();
	await expect(page.locator(".multipatch-row")).toHaveCount(10);
	await expect(
		page.locator(".multipatch-row").getByText("Unpatched", { exact: true }),
	).toHaveCount(7);
	await expect(page.locator(".fixture-type-icon img")).toHaveCount(10);
	const patchIconSources = await page
		.locator(".fixture-type-icon img")
		.evaluateAll((icons) =>
			icons.map((icon) => decodeURIComponent((icon as HTMLImageElement).src)),
		);
	expect(
		patchIconSources.some((source) =>
			source.includes("fixture type fresnel barn doors"),
		),
	).toBe(true);
	expect(
		patchIconSources.some((source) =>
			source.includes("fixture type led wash moving light lenses"),
		),
	).toBe(true);
	expect(
		patchIconSources.some((source) => source.includes("fixture type blinder")),
	).toBe(true);
	expect(
		patchIconSources.some((source) => source.includes("fixture type acl set")),
	).toBe(true);

	await page.goto(
		"/iframe.html?id=tosklight-windows-setup--timecode&viewMode=story",
	);
	await expect(page.getByText("Desk Setup", { exact: true })).toBeVisible();
	await expect(page.getByRole("heading", { name: "Timecode" })).toBeVisible();
	await expect(page.getByText("ltc:", { exact: true })).toBeVisible();
	await expect(
		page.getByText("Fallback allowed", { exact: true }),
	).toBeVisible();
});

test("FIXTURE-SHEET-002-004 › compact modes increase density without dropping configured values", async ({
	page,
}) => {
	await page.setViewportSize({ width: 430, height: 844 });
	const inspect = async (
		story: "off-small-screen" | "icon-only" | "text-only",
	) => {
		await page.goto(
			`/iframe.html?id=tosklight-windows-fixture-sheet--${story}&viewMode=story`,
		);
		await page.evaluate(() => document.fonts.ready);
		const sheet = page.locator(".fixture-window");
		await expect(sheet).toHaveAttribute(
			"data-fixture-sheet-compact-mode",
			story === "off-small-screen" ? "off" : story,
		);
		const table = page.locator(".ui-data-table");
		const rows = page.locator(".ui-data-table-row:not(.header)");
		await expect(rows).toHaveCount(24);
		const geometry = await page
			.locator(".fixture-table .ui-window-scroller")
			.evaluate((host) => {
				const viewport = host.getBoundingClientRect();
				const bodyRows = [
					...host.querySelectorAll<HTMLElement>(
						".ui-data-table-row:not(.header)",
					),
				];
				return {
					clientWidth: host.clientWidth,
					scrollWidth: host.scrollWidth,
					rowHeights: bodyRows.map((row) => row.getBoundingClientRect().height),
					visibleRows: bodyRows.filter((row) => {
						const bounds = row.getBoundingClientRect();
						return (
							bounds.top >= viewport.top && bounds.bottom <= viewport.bottom
						);
					}).length,
				};
			});
		expect(geometry.scrollWidth).toBeGreaterThan(geometry.clientWidth);
		expect(new Set(geometry.rowHeights)).toEqual(
			new Set([story === "off-small-screen" ? 43 : 32]),
		);
		await expect(table.getByText("102.0", { exact: true })).toBeVisible();
		await expect(table.getByText("102.1", { exact: true })).toBeVisible();
		const boundedMarkers = rows.locator(
			".fixture-step-marker, .fixture-sheet-preload-marker, .preload-value, .source-value",
		);
		expect(
			await boundedMarkers.evaluateAll((markers) =>
				markers.every((marker) => {
					const bounds = marker.getBoundingClientRect();
					const cell = marker.closest('[role="cell"]')?.getBoundingClientRect();
					return Boolean(
						cell &&
							bounds.left >= cell.left &&
							bounds.right <= cell.right &&
							bounds.top >= cell.top &&
							bounds.bottom <= cell.bottom,
					);
				}),
			),
		).toBe(true);
		const firstRowDynamics = rows.first().locator(".fixture-dynamic-stack");
		await expect(firstRowDynamics).toHaveCount(2);
		expect(
			await firstRowDynamics.evaluateAll((indicators) =>
				indicators.every((indicator) => {
					const bounds = indicator.getBoundingClientRect();
					const cell = indicator
						.closest('[role="cell"]')
						?.getBoundingClientRect();
					return Boolean(
						cell &&
							bounds.left >= cell.left &&
							bounds.right <= cell.right &&
							bounds.top >= cell.top &&
							bounds.bottom <= cell.bottom,
					);
				}),
			),
		).toBe(true);
		const groupStatuses = rows.locator(".fixture-group-master-status");
		await expect(groupStatuses.first()).toHaveAttribute(
			"data-group-master-state",
			"highlight-bypass",
		);
		await expect(groupStatuses.nth(1)).toHaveAttribute(
			"data-group-master-state",
			"flash",
		);
		expect(
			await groupStatuses.evaluateAll((statuses) =>
				statuses.every((status) => {
					const bounds = status.getBoundingClientRect();
					const cell = status.closest('[role="cell"]')?.getBoundingClientRect();
					return Boolean(
						cell && bounds.top >= cell.top && bounds.bottom <= cell.bottom,
					);
				}),
			),
		).toBe(true);
		expect(await table.locator(".source-programmer").count()).toBeGreaterThan(
			0,
		);
		return geometry;
	};

	const off = await inspect("off-small-screen");
	await expect(page.locator(".vertical-meter").first()).toBeVisible();
	await expect(page.locator(".fixture-sheet-value-text").first()).toBeVisible();
	const offMedia = page
		.getByText("Media Folder Folder 2", { exact: true })
		.first();
	await offMedia.scrollIntoViewIfNeeded();
	await expect(offMedia).toBeVisible();
	await expect(page.getByRole("columnheader", { name: "Media" })).toBeVisible();
	const unavailable = page.locator('[title="Focus unavailable"]').first();
	await unavailable.scrollIntoViewIfNeeded();
	await expect(unavailable).toBeVisible();
	const iconOnly = await inspect("icon-only");
	await expect(page.locator(".vertical-meter").first()).toBeVisible();
	await expect(page.locator(".fixture-sheet-value-text").first()).toBeHidden();
	for (const marker of ["MeF", "Me", "MaF", "Ma"]) {
		const glyph = page.getByText(marker, { exact: true }).first();
		await glyph.scrollIntoViewIfNeeded();
		await expect(glyph).toBeVisible();
	}
	const legacyWide = page.getByText("Legacy Wide", { exact: true }).first();
	await expect(legacyWide).toHaveCount(1);
	expect(
		await legacyWide.evaluate((element) => getComputedStyle(element).display),
	).toBe("none");
	const textOnly = await inspect("text-only");
	await expect(page.locator(".vertical-meter").first()).toBeHidden();
	await expect(page.locator(".fixture-sheet-value-text").first()).toBeVisible();
	const textMedia = page
		.getByText("Media Folder Folder 2", { exact: true })
		.first();
	await textMedia.scrollIntoViewIfNeeded();
	await expect(textMedia).toBeVisible();
	expect(iconOnly.visibleRows).toBeGreaterThan(off.visibleRows);
	expect(textOnly.visibleRows).toBe(iconOnly.visibleRows);
});

test("Form stories keep inputs, scrolling, fader, pickers, grouped selections, and file drop interactive", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1100, height: 720 });
	await page.goto(
		"/iframe.html?id=tosklight-integration-form-controls--input-fields&viewMode=story",
	);
	const canvas = page.locator(".forms-story-canvas");
	await expect(canvas).toBeVisible();
	const scrolling = await canvas.evaluate((element) => ({
		clientHeight: element.clientHeight,
		scrollHeight: element.scrollHeight,
	}));
	expect(scrolling.scrollHeight).toBeGreaterThan(scrolling.clientHeight);
	await canvas.hover();
	await page.mouse.wheel(0, 420);
	await expect
		.poll(() => canvas.evaluate((element) => element.scrollTop))
		.toBeGreaterThan(0);

	const valueWithFader = page.getByRole("textbox", {
		name: "Value with fader",
	});
	await valueWithFader
		.locator("..")
		.getByRole("button", { name: "Open number pad" })
		.click();
	const faderDialog = page.getByRole("dialog", { name: "Value with fader" });
	await expect(
		faderDialog.getByRole("slider", { name: "Value with fader fader" }),
	).toHaveCount(1);
	await expect(faderDialog).toHaveClass(/with-value-fader/u);
	await faderDialog
		.getByRole("button", { name: "Close Value with fader" })
		.click();

	const valueWithPresets = page.getByRole("textbox", {
		name: "Value with presets",
	});
	await valueWithPresets
		.locator("..")
		.getByRole("button", { name: "Open number pad" })
		.click();
	let presetDialog = page.getByRole("dialog", { name: "Value with presets" });
	const modeToggle = presetDialog.getByRole("button", {
		name: "Show presets",
	});
	await expect(modeToggle.locator('[data-active="true"]')).toHaveText("Value");
	await expect(
		presetDialog.getByRole("button", { name: "Release value" }),
	).toBeVisible();
	await modeToggle.click();
	await expect(
		presetDialog.getByText("Intensity", { exact: true }),
	).toBeVisible();
	await expect(
		presetDialog.getByText("Operator defaults", { exact: true }),
	).toBeVisible();
	await presetDialog.getByRole("button", { name: /Full/ }).click();
	await expect(valueWithPresets).toHaveValue("100");
	await expect(presetDialog).toHaveCount(0);

	await valueWithPresets
		.locator("..")
		.getByRole("button", { name: "Open number pad" })
		.click();
	presetDialog = page.getByRole("dialog", { name: "Value with presets" });
	await presetDialog.getByRole("button", { name: "Release value" }).click();
	await expect(valueWithPresets).toHaveValue("");

	const notes = page.getByRole("textbox", { name: "Notes" });
	const noteValue = await notes.inputValue();
	expect(noteValue.split("\n").length).toBeGreaterThanOrEqual(10);
	await notes.evaluate((element: HTMLTextAreaElement) => {
		element.setSelectionRange(8, 28, "forward");
		element.dataset.identity = "preserved";
	});
	await notes
		.locator("xpath=..")
		.getByRole("button", { name: "Scroll text down" })
		.click();
	await expect(notes).toHaveAttribute("data-identity", "preserved");
	await expect(notes).toHaveValue(noteValue);
	expect(
		await notes.evaluate((element: HTMLTextAreaElement) => [
			element.selectionStart,
			element.selectionEnd,
		]),
	).toEqual([8, 28]);

	await page.goto(
		"/iframe.html?id=tosklight-integration-form-controls--form-components&viewMode=story",
	);
	await expect(canvas).toBeVisible();
	await page.getByRole("radio", { name: "3D" }).click();
	await expect(page.getByRole("radio", { name: "3D" })).toHaveAttribute(
		"aria-checked",
		"true",
	);

	await expect(page.locator('input[type="file"]')).toHaveCount(0);
	await expect(page.locator('input[type="range"]')).toHaveCount(1);
	await expect(
		page.locator('.horizontal-touch-fader input[type="range"]'),
	).toHaveCount(1);
	const level = page.getByRole("slider", { name: "Level" });
	await level.fill("73");
	await expect(page.getByText("73%", { exact: true })).toBeVisible();

	const color = page.getByRole("button", { name: "#1BD6EC" });
	const triggerBox = await color.boundingBox();
	const colorControlBox = await color.locator("..").boundingBox();
	const swatchBox = await color
		.locator(".ui-color-trigger-swatch")
		.boundingBox();
	expect(triggerBox).not.toBeNull();
	expect(colorControlBox).not.toBeNull();
	expect(swatchBox).not.toBeNull();
	expect(triggerBox!.width).toBeCloseTo(colorControlBox!.width, 0);
	expect(swatchBox!.x).toBeGreaterThan(triggerBox!.x);
	expect(swatchBox!.width).toBeLessThan(triggerBox!.width);
	await color.click();
	for (const choice of await page
		.getByRole("option", { name: /Use color/u })
		.all()) {
		const box = await choice.boundingBox();
		expect(Math.abs((box?.width ?? 0) - (box?.height ?? 0))).toBeLessThan(2);
	}
	const customHex = page.getByRole("textbox", { name: "Custom hex" });
	const customAction = page.getByRole("button", { name: "Use custom color" });
	const customPreview = page.getByLabel("Color preview");
	const customBoxes = await Promise.all([
		customHex.locator("..").boundingBox(),
		customAction.boundingBox(),
		customPreview.boundingBox(),
	]);
	for (const box of customBoxes) {
		expect(box).not.toBeNull();
		expect(box?.height).toBeCloseTo(48, 0);
		expect(box?.y).toBeCloseTo(customBoxes[0]?.y ?? 0, 0);
	}
	await page.keyboard.press("Escape");

	await page.getByRole("button", { name: "Choose icon" }).click();
	const iconDialog = page.getByRole("dialog", { name: "Choose icon" });
	await expect(iconDialog).toBeVisible();
	const iconGroup = iconDialog.getByRole("button", { name: "Icon group" });
	await expect(iconGroup).toContainText("Gobo");
	await expect(iconGroup.locator(".ui-select-chevron svg")).toBeVisible();
	await expect(iconGroup).toHaveAttribute("aria-expanded", "false");
	await expect(iconDialog.locator('[data-icon-group="gobo"]')).toBeVisible();
	await iconGroup.click();
	await expect(iconGroup).toHaveAttribute("aria-expanded", "true");
	await page.getByRole("option", { name: "Fixture type" }).click();
	await expect(
		iconDialog.locator('[data-icon-group="fixture-type"]'),
	).toBeVisible();
	await expect(page.getByLabel("Custom")).toHaveCount(0);
	await expect(
		page.getByRole("button", { name: "Use custom icon" }),
	).toHaveCount(0);
	const fixtureTypeIcons = iconDialog.locator(
		'[data-icon-group="fixture-type"] .ui-button',
	);
	expect(await fixtureTypeIcons.count()).toBeGreaterThan(0);
	await fixtureTypeIcons.first().click();

	const modeSelect = page.getByRole("button", { name: /^Software$/u });
	await expect(modeSelect.locator(".ui-select-chevron svg")).toBeVisible();
	await expect(modeSelect).toHaveAttribute("aria-expanded", "false");
	await modeSelect.click();
	await expect(modeSelect).toHaveAttribute("aria-expanded", "true");
	await expect(page.getByRole("listbox", { name: "Mode" })).toBeVisible();
	await page.getByRole("option", { name: "Hardware" }).click();
	await expect(page.getByRole("button", { name: /^Hardware$/u })).toBeVisible();

	const groupedTrigger = page.getByRole("button", { name: /^GO$/u });
	await expect(
		groupedTrigger.locator(".ui-grouped-selection-icon"),
	).toContainText("▶");
	const plainGroupedTrigger = page.getByRole("button", { name: /^Master$/u });
	await expect(
		plainGroupedTrigger.locator(".ui-grouped-selection-icon"),
	).toHaveCount(0);
	await expect(
		plainGroupedTrigger.locator(".ui-grouped-selection-value"),
	).toHaveClass(/has-no-icon/u);
	const plainValueBox = await plainGroupedTrigger
		.locator(".ui-grouped-selection-value")
		.boundingBox();
	const plainTriggerBox = await plainGroupedTrigger.boundingBox();
	expect((plainValueBox?.x ?? 0) - (plainTriggerBox?.x ?? 0)).toBeLessThan(20);
	await groupedTrigger.click();
	const grouped = page.getByRole("dialog", { name: "Choose Top button" });
	await expect(grouped.getByText("Advance to the next cue.")).toBeVisible();
	for (const option of await grouped
		.locator(".ui-grouped-selection-options .ui-button")
		.all()) {
		await expect(option).toHaveCSS("justify-content", "flex-start");
		const optionBox = await option.boundingBox();
		const contentBox = await option
			.locator(".ui-grouped-selection-option")
			.boundingBox();
		expect((contentBox?.x ?? 0) - (optionBox?.x ?? 0)).toBeLessThanOrEqual(17);
	}
	await expect(
		grouped.getByRole("button", { name: "Empty Button" }),
	).toBeVisible();
	await grouped.getByRole("button", { name: /GO MINUS/u }).click();
	await expect(page.getByRole("button", { name: /GO MINUS/u })).toBeVisible();

	const fileDrop = page.getByRole("button", {
		name: /Choose file.*Fixture profile/u,
	});
	await fileDrop.click();
	await expect(fileDrop).toContainText("ToskLight File Manager opened");
	await page.evaluate((selector) => {
		const field = document.querySelector(selector);
		if (!field) return;
		const transfer = new DataTransfer();
		transfer.items.add(new File(["fixture"], "profile.gdtf"));
		field.dispatchEvent(
			new DragEvent("dragenter", {
				bubbles: true,
				cancelable: true,
				dataTransfer: transfer,
			}),
		);
	}, ".ui-file-drop-field");
	await expect(page.locator(".ui-file-drop-field").first()).toHaveClass(
		/drag-accepted/u,
	);
});

test("input modal stories expose authoritative carets and literal keypad or keyboard geometry", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1100, height: 760 });
	await page.goto(
		"/iframe.html?id=tosklight-integration-input-modal-surfaces--input-modal-configurations&viewMode=story",
	);
	const numberModal = page.getByRole("dialog", { name: "Fade time" });
	await expect(numberModal.locator(".modal-caret-value > i")).toHaveCSS(
		"outline-style",
		"solid",
	);
	const numberColor = await numberModal
		.locator(".modal-caret-value")
		.evaluate((element) => getComputedStyle(element).color);
	const numberKeys = numberModal.locator(".modal-number-input .ui-button");
	const firstKey = await numberKeys.first().boundingBox();
	expect(firstKey?.width).toBeGreaterThanOrEqual(63);
	expect(firstKey?.height).toBeGreaterThanOrEqual(63);
	const numberPreview = numberModal.getByRole("textbox", {
		name: "Fade time value",
	});
	await expect(numberPreview).toHaveCSS("justify-content", "flex-start");
	await expect(numberPreview).toHaveCSS("text-align", "left");
	const previewBox = await numberPreview.boundingBox();
	const cursorLeft = numberModal.getByRole("button", {
		name: "Move cursor left",
	});
	const cursorRight = numberModal.getByRole("button", {
		name: "Move cursor right",
	});
	const cursorLeftBox = await cursorLeft.boundingBox();
	const cursorRightBox = await cursorRight.boundingBox();
	expect(cursorLeftBox?.x).toBeGreaterThan(
		(previewBox?.x ?? 0) + (previewBox?.width ?? 0),
	);
	expect(cursorRightBox?.x).toBeGreaterThan(cursorLeftBox?.x ?? 0);
	expect(cursorLeftBox?.y).toBe(previewBox?.y);
	expect(cursorRightBox?.y).toBe(previewBox?.y);
	await expect(
		numberModal.locator(".modal-number-input").getByRole("button", {
			name: "Move cursor left",
		}),
	).toHaveCount(0);
	const enter = numberModal.getByRole("button", { name: "ENTER" });
	const enterBox = await enter.boundingBox();
	expect(enterBox?.height).toBeGreaterThanOrEqual(136);
	await cursorLeft.click();
	await numberModal.getByRole("button", { name: "9" }).click();
	await expect(numberPreview).toContainText("62.98");

	await page.goto(
		"/iframe.html?id=tosklight-integration-input-modal-surfaces--empty-text-input-modal&viewMode=story",
	);
	const textModal = page.getByRole("dialog", { name: "Fixture name" });
	const preview = textModal.getByRole("textbox", {
		name: "Fixture name value",
	});
	await expect(preview).toHaveAccessibleName("Fixture name value");
	await expect(preview).toContainText("Enter fixture name");
	await expect(preview.locator("> i")).toHaveCSS("outline-style", "solid");
	const rail = textModal.locator(".modal-keyboard-actions");
	const textBackspace = textModal.getByRole("button", { name: "Backspace" });
	const textEnter = textModal.getByRole("button", { name: "Enter · Confirm" });
	const railBox = await rail.boundingBox();
	const textBackspaceBox = await textBackspace.boundingBox();
	const textEnterBox = await textEnter.boundingBox();
	expect(textBackspaceBox?.x).toBe(textEnterBox?.x);
	expect(textBackspaceBox?.width).toBe(textEnterBox?.width);
	expect(
		(textBackspaceBox?.y ?? 0) + (textBackspaceBox?.height ?? 0),
	).toBeLessThan(textEnterBox?.y ?? 0);
	expect(railBox?.height).toBeGreaterThan(textEnterBox?.height ?? 0);
	await expect(
		textModal.locator(".modal-keyboard-bottom .backspace"),
	).toHaveCount(0);
	const right = textModal.getByRole("button", { name: "Move cursor right" });
	const space = textModal.getByRole("button", { name: "SPACE", exact: true });
	const rightBox = await right.boundingBox();
	const spaceBox = await space.boundingBox();
	expect(
		(spaceBox?.x ?? 0) - ((rightBox?.x ?? 0) + (rightBox?.width ?? 0)),
	).toBeGreaterThanOrEqual(52);
	const escapeButton = textModal.getByRole("button", { name: /ESC/u });
	expect(
		await escapeButton.evaluate(
			(element) => getComputedStyle(element).backgroundColor,
		),
	).not.toBe("rgba(0, 0, 0, 0)");
	const shift = textModal.getByRole("button", { name: "Shift" });
	for (const letter of ["F", "I", "X"]) {
		await textModal.getByRole("button", { name: letter, exact: true }).click();
	}
	const leadingText = preview.locator("> span").first();
	const leadingBefore = await leadingText.boundingBox();
	await textModal.getByRole("button", { name: "Move cursor left" }).click();
	const leadingAfter = await leadingText.boundingBox();
	expect(
		Math.abs((leadingAfter?.x ?? 0) - (leadingBefore?.x ?? 0)),
	).toBeLessThan(1);
	await expect(preview).toHaveCSS("justify-content", "flex-start");
	await expect(preview).toHaveCSS("text-align", "left");
	expect(
		Number.parseFloat(
			await preview.evaluate((element) => getComputedStyle(element).fontSize),
		),
	).toBeLessThanOrEqual(20);
	const textColor = await preview.evaluate(
		(element) => getComputedStyle(element).color,
	);
	expect(textColor).not.toBe(numberColor);
	await shift.click();
	await expect(shift).toHaveAttribute("data-shift-state", "one-shot");
	await textModal.getByRole("button", { name: "A", exact: true }).click();
	await expect(preview).toContainText("A");
	await expect(shift).toHaveAttribute("data-shift-state", "inactive");

	await page.goto(
		"/iframe.html?id=tosklight-integration-input-modal-surfaces--multiline-input-modal&viewMode=story",
	);
	const multiline = page.getByRole("dialog", { name: "Fixture name" });
	const multilineEditor = multiline.getByRole("textbox", {
		name: "Fixture name value",
	});
	await expect(multilineEditor).not.toHaveAttribute("readonly");
	await expect(multilineEditor).toHaveCSS("overflow-y", "scroll");
	await expect(multilineEditor).toHaveCSS("outline-style", "none");
	await expect(multilineEditor).toHaveCSS("color", "rgb(255, 255, 255)");
	const multilineCaret = multiline.locator(
		".modal-multiline-caret-content > i",
	);
	await expect(multilineCaret).toHaveCSS("outline-style", "solid");
	await expect(multiline.locator(".modal-multiline-caret-layer")).toHaveCSS(
		"color",
		"rgb(255, 255, 255)",
	);
	expect(
		await multilineEditor.evaluate(
			(editor) => editor.scrollHeight > editor.clientHeight,
		),
	).toBe(true);
	const cursorUp = multiline.getByRole("button", {
		name: "Move cursor up one line",
	});
	const cursorDown = multiline.getByRole("button", {
		name: "Move cursor down one line",
	});
	const editorBox = await multilineEditor.boundingBox();
	const cursorUpBox = await cursorUp.boundingBox();
	const cursorDownBox = await cursorDown.boundingBox();
	expect(cursorUpBox?.x).toBeGreaterThan(
		(editorBox?.x ?? 0) + (editorBox?.width ?? 0),
	);
	expect(cursorDownBox?.x).toBe(cursorUpBox?.x);
	expect(cursorDownBox?.y).toBeGreaterThan(
		(cursorUpBox?.y ?? 0) + (cursorUpBox?.height ?? 0),
	);
	await multilineEditor.evaluate((editor) => {
		editor.scrollTop = 0;
		editor.dispatchEvent(new Event("scroll"));
	});
	await multilineEditor.click({ position: { x: 14, y: 45 } });
	await expect
		.poll(() =>
			multilineEditor.evaluate(
				(editor: HTMLTextAreaElement) => editor.selectionStart,
			),
		)
		.toBe(11);
	const clickedCaretBox = await multilineCaret.boundingBox();
	await cursorDown.click();
	await expect(cursorDown).toHaveAttribute("data-keyboard-pressed", "true");
	await expect(multilineCaret).toHaveAttribute("data-caret-moving", "true");
	await expect(multilineCaret).toHaveCSS("animation-name", "none");
	await expect
		.poll(() =>
			multilineEditor.evaluate(
				(editor: HTMLTextAreaElement) => editor.selectionStart,
			),
		)
		.toBe(23);
	const downCaretBox = await multilineCaret.boundingBox();
	expect(downCaretBox?.y).toBeGreaterThan(
		(clickedCaretBox?.y ?? Number.POSITIVE_INFINITY) + 10,
	);
	await cursorUp.click();
	await expect
		.poll(() =>
			multilineEditor.evaluate(
				(editor: HTMLTextAreaElement) => editor.selectionStart,
			),
		)
		.toBe(11);
	const cursorRightMultiline = multiline.getByRole("button", {
		name: "Move cursor right",
	});
	const beforeRightCaretBox = await multilineCaret.boundingBox();
	await cursorRightMultiline.click();
	await expect
		.poll(() =>
			multilineEditor.evaluate(
				(editor: HTMLTextAreaElement) => editor.selectionStart,
			),
		)
		.toBe(12);
	const afterRightCaretBox = await multilineCaret.boundingBox();
	expect(afterRightCaretBox?.x).toBeGreaterThan(
		beforeRightCaretBox?.x ?? Number.POSITIVE_INFINITY,
	);
	await multiline.getByRole("button", { name: "Move cursor left" }).click();
	await expect
		.poll(() =>
			multilineEditor.evaluate(
				(editor: HTMLTextAreaElement) => editor.selectionStart,
			),
		)
		.toBe(11);
	expect(
		await multilineEditor.evaluate(
			(editor) => document.activeElement === editor,
		),
	).toBe(true);
	const xKey = multiline.getByRole("button", { name: "X", exact: true });
	await xKey.click();
	await expect(xKey).toHaveAttribute("data-keyboard-pressed", "true");
	await expect(xKey).toHaveCSS("background-color", "rgb(8, 122, 140)");
	await expect(multilineEditor).toHaveValue(
		"First line\nxSecond line\nThird line\nFourth line\nFifth line\nSixth line\nSeventh line\nEighth line",
	);
	await expect
		.poll(() =>
			multilineEditor.evaluate(
				(editor: HTMLTextAreaElement) => editor.selectionStart,
			),
		)
		.toBe(12);
	const scrollBeforeCursorTravel = await multilineEditor.evaluate(
		(editor) => editor.scrollTop,
	);
	for (let line = 0; line < 6; line += 1) {
		await cursorDown.click();
	}
	await expect
		.poll(() => multilineEditor.evaluate((editor) => editor.scrollTop))
		.toBeGreaterThan(scrollBeforeCursorTravel);
	const [scrolledEditorBox, scrolledCaretBox] = await Promise.all([
		multilineEditor.boundingBox(),
		multilineCaret.boundingBox(),
	]);
	expect(scrolledCaretBox?.y).toBeGreaterThanOrEqual(scrolledEditorBox?.y ?? 0);
	expect(
		(scrolledCaretBox?.y ?? 0) + (scrolledCaretBox?.height ?? 0),
	).toBeLessThanOrEqual(
		(scrolledEditorBox?.y ?? 0) + (scrolledEditorBox?.height ?? 0),
	);
	await expect(
		multiline.getByRole("button", { name: "Enter · New line" }),
	).toBeVisible();
	const multilineRail = multiline.locator(".modal-keyboard-actions.multiline");
	const multilineBackspace = multiline.getByRole("button", {
		name: "Backspace",
	});
	const multilineEnter = multiline.getByRole("button", {
		name: "Enter · New line",
	});
	const firstLetterKey = multiline.locator('[data-keyboard-code="Digit1"]');
	const multilineRailBox = await multilineRail.boundingBox();
	const multilineBackspaceBox = await multilineBackspace.boundingBox();
	const multilineEnterBox = await multilineEnter.boundingBox();
	const firstLetterKeyBox = await firstLetterKey.boundingBox();
	expect(
		Math.abs(
			(multilineBackspaceBox?.height ?? 0) - (firstLetterKeyBox?.height ?? 0),
		),
	).toBeLessThanOrEqual(1);
	expect(multilineEnterBox?.height).toBeGreaterThan(
		(multilineBackspaceBox?.height ?? Number.POSITIVE_INFINITY) * 2,
	);
	expect(
		(multilineEnterBox?.y ?? 0) + (multilineEnterBox?.height ?? 0),
	).toBeCloseTo(
		(multilineRailBox?.y ?? 0) + (multilineRailBox?.height ?? 0),
		0,
	);
	const backspaceIconBox = await multilineBackspace.locator("b").boundingBox();
	const backspaceLabelBox = await multilineBackspace
		.locator("small")
		.boundingBox();
	expect(backspaceLabelBox?.y).toBeGreaterThan(
		(backspaceIconBox?.y ?? 0) + (backspaceIconBox?.height ?? 0),
	);
	const done = multiline.getByRole("button", { name: "Done" });
	await expect(done).toBeVisible();
	await expect(done).toHaveCSS("cursor", "pointer");
	const doneBox = await done.boundingBox();
	expect(doneBox?.width).toBeGreaterThanOrEqual(95);
	const doneBorders = await done.evaluate((element) => {
		const styles = getComputedStyle(element);
		return [styles.borderLeftColor, styles.borderRightColor];
	});
	expect(doneBorders).not.toContain("rgba(0, 0, 0, 0)");
	const doneBackground = await done.evaluate(
		(element) => getComputedStyle(element).backgroundColor,
	);
	await done.hover();
	await expect
		.poll(() =>
			done.evaluate((element) => getComputedStyle(element).backgroundColor),
		)
		.not.toBe(doneBackground);
	const multilineShift = multiline.getByRole("button", { name: "Shift" });
	const physicalZ = multiline.locator('[data-keyboard-code="KeyZ"]');
	expect(
		await multilineShift.evaluate(
			(shift, z) => shift.parentElement === (z as HTMLElement).parentElement,
			await physicalZ.elementHandle(),
		),
	).toBe(true);
	const shiftBox = await multilineShift.boundingBox();
	const zBox = await physicalZ.boundingBox();
	expect(shiftBox?.width).toBeGreaterThan(
		zBox?.width ?? Number.POSITIVE_INFINITY,
	);
	await expect(multilineShift.locator(".modal-shift-icon")).toBeVisible();
	await multiline.getByRole("button", { name: "Close input" }).click();
	let unsaved = page.getByRole("dialog", {
		name: "Unsaved multiline text changes",
	});
	await expect(unsaved).toBeVisible();
	await expect(
		unsaved.getByRole("button", { name: "Discard changes" }),
	).toBeVisible();
	await expect(
		unsaved.getByRole("button", { name: "Save changes" }),
	).toBeVisible();
	await unsaved.getByText("Stay in modal", { exact: true }).click();
	await expect(unsaved).toHaveCount(0);
	await expect(multiline).toBeVisible();
	await page.keyboard.press("Escape");
	unsaved = page.getByRole("dialog", {
		name: "Unsaved multiline text changes",
	});
	await expect(unsaved).toBeVisible();
	await unsaved.getByRole("button", { name: "Save changes" }).click();
	await expect(multiline).toHaveCount(0);
	await expect(page.getByLabel("Committed input modal value")).toContainText(
		"xSecond line",
	);
});

test("production marketing and application modal stories preserve their real compositions and workflows", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1920, height: 1080 });
	await page.goto(
		"/iframe.html?id=tosklight-marketing--complete-product-demo&viewMode=story",
	);
	await expect(page.getByTestId("product-demo")).toBeVisible();
	await expect(
		page.locator(".product-demo-application .app-shell"),
	).toBeVisible();
	await expect(
		page.locator(".product-demo-application .left-dock"),
	).toBeVisible();
	await expect(
		page.locator(".product-demo-stage .stage-3d-canvas"),
	).toBeVisible();
	await expect(page.locator(".product-demo-dmx-cell")).toHaveCount(2_048);
	await expect(page.locator(".product-demo-playback-strip")).toHaveCount(4);
	await expect(
		page.getByText("Preparing the product demo.", { exact: true }),
	).toBeVisible();

	await page.goto(
		"/iframe.html?id=tosklight-modal-workflows--playback-configuration&viewMode=story",
	);
	await expect(
		page.getByRole("dialog", { name: "Playback Configuration" }),
	).toBeVisible();
	await expect(page.getByText("Main Sequence", { exact: true })).toBeVisible();
	await page.getByRole("button", { name: "Behavior", exact: true }).click();
	await expect(
		page.getByText("Protect from Swap", { exact: true }),
	).toBeVisible();
	await page.getByRole("button", { name: "Layout", exact: true }).click();
	await expect(page.getByText("Top button", { exact: true })).toBeVisible();

	await page.goto(
		"/iframe.html?id=tosklight-modal-workflows--record-existing-target&viewMode=story",
	);
	await expect(
		page.getByRole("dialog", { name: "Record to Cuelist 1" }),
	).toBeVisible();
	await page.getByRole("button", { name: "Merge", exact: true }).click();
	await expect(page.getByLabel("Record mode choice")).toHaveText("merge");
});

test("marketing pools use catalog assets and keep vacant slots uncolored", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-marketing--groups-window&viewMode=story",
	);
	const populatedGroups = page.locator(".group-card:not(.empty)");
	await expect(populatedGroups).toHaveCount(5);
	await expect(populatedGroups.locator(".pool-card-icon-image")).toHaveCount(5);
	const groupIconSources = await populatedGroups
		.locator(".pool-card-icon-image")
		.evaluateAll((icons) =>
			icons.map((icon) => decodeURIComponent((icon as HTMLImageElement).src)),
		);
	expect(
		groupIconSources.some((source) =>
			source.includes("fixture type profile dimmer lamp"),
		),
	).toBe(true);
	expect(
		groupIconSources.some((source) =>
			source.includes("fixture type led wash moving light lenses"),
		),
	).toBe(true);
	expect(
		groupIconSources.some((source) => source.includes("fixture type blinder")),
	).toBe(true);
	const emptyGroup = page.locator(".group-card.empty").first();
	await expect(emptyGroup).toHaveCSS("--pool-card-color", "");
	await expect(emptyGroup).toHaveCSS("--pool-card-resolved-color", "#65717b");
	const emptyGroupStyle = await emptyGroup.evaluate((element) => {
		const style = getComputedStyle(element);
		return {
			background: style.backgroundColor,
			border: style.borderTopColor,
			borderStyle: style.borderTopStyle,
		};
	});

	await page.goto(
		"/iframe.html?id=tosklight-marketing--position-presets-window&viewMode=story",
	);
	const populatedPositions = page.locator(".preset-card:not(.empty)");
	await expect(populatedPositions).toHaveCount(6);
	await expect(populatedPositions.locator(".pool-card-icon-image")).toHaveCount(
		6,
	);
	const positionIconSources = await populatedPositions
		.locator(".pool-card-icon-image")
		.evaluateAll((icons) =>
			icons.map((icon) => decodeURIComponent((icon as HTMLImageElement).src)),
		);
	expect(
		positionIconSources.every((source) => source.includes("position")),
	).toBe(true);
	const emptyPosition = page.locator(".preset-card.empty").first();
	await expect(emptyPosition).toHaveCSS("--pool-card-color", "");
	await expect(emptyPosition).toHaveCSS(
		"--pool-card-resolved-color",
		"#65717b",
	);
	await expect
		.poll(() =>
			emptyPosition.evaluate((element) => {
				const style = getComputedStyle(element);
				return {
					background: style.backgroundColor,
					border: style.borderTopColor,
					borderStyle: style.borderTopStyle,
				};
			}),
		)
		.toEqual(emptyGroupStyle);
});

test("Grid Dynamics paints presets, toggles history, and queues Preload transport", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1600, height: 920 });
	await page.goto(
		"/iframe.html?id=tosklight-windows-grid-dynamics--full-application-discussion&viewMode=story",
	);
	const positionStep = page.getByRole("button", {
		name: "Position, step 2: Current",
		exact: true,
	});
	await expect(positionStep).toBeVisible();
	await positionStep.click({ button: "right" });
	const presetDialog = page.getByRole("dialog", {
		name: "Choose position preset",
	});
	await expect(presetDialog).toBeVisible();
	await presetDialog
		.getByRole("button", { name: "Cross", exact: true })
		.click();
	const paintedStep = page.getByRole("button", {
		name: "Position, step 2: Cross",
		exact: true,
	});
	await expect(paintedStep).toBeVisible();
	await paintedStep.click();
	await expect(positionStep).toBeVisible();
	await page.getByRole("button", { name: "Stop", exact: true }).click();
	await expect(
		page.getByRole("button", { name: "Play", exact: true }),
	).toBeVisible();

	await page.goto(
		"/iframe.html?id=tosklight-windows-grid-dynamics--preload-play-queued&viewMode=story",
	);
	await expect(
		page.getByRole("button", { name: "PRELOAD GO", exact: true }),
	).toBeVisible();
	await page.getByRole("button", { name: "Stop", exact: true }).click();
	await page.getByRole("button", { name: "Play", exact: true }).click();
	await expect(
		page.getByText("Preloaded · Play queued", { exact: true }),
	).toBeVisible();
	await expect(page.getByText("Playing live", { exact: true })).toHaveCount(0);
});

test("Title chrome preserves tab and terminal ordering for windows and modals", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-window-system-title-chrome--modal-chrome&viewMode=story",
	);
	const tabs = page.getByRole("tab");
	await expect(tabs).toHaveCount(2);
	await expect(tabs.filter({ hasText: "Select" })).toHaveAttribute(
		"aria-selected",
		"true",
	);
	await tabs.filter({ hasText: "Navigate" }).click();
	await expect(tabs.filter({ hasText: "Navigate" })).toHaveAttribute(
		"aria-selected",
		"true",
	);
	const terminals = page.locator(".ui-title-chrome-terminals button");
	await expect(terminals).toHaveCount(2);
	await expect(terminals.nth(0)).toHaveText("Apply");
	await expect(terminals.nth(1)).toHaveAttribute("aria-label", "Close modal");
});

test("Title chrome dropdown actions, toggles, custom close, Escape, and keyboard activation work", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-window-system-title-chrome--dropdown-items&viewMode=story",
	);
	const menuTrigger = page.getByRole("button", { name: "Add options" });
	await menuTrigger.focus();
	await menuTrigger.press("Enter");
	await page.getByRole("menuitemcheckbox", { name: "Follow Preload" }).click();
	await expect(page.getByRole("menu")).toBeVisible();
	await expect(page.getByText("Follow Preload on")).toBeVisible();
	await page.keyboard.press("Escape");
	await expect(page.getByRole("menu")).toHaveCount(0);
	await menuTrigger.click();
	await page.getByRole("menuitem", { name: "Add Cue" }).click();
	await expect(page.getByRole("menu")).toHaveCount(0);
	await expect(page.getByText("Cue added")).toBeVisible();

	await page.goto(
		"/iframe.html?id=tosklight-window-system-title-chrome--custom-dropdown-content&viewMode=story",
	);
	await page.getByRole("button", { name: "Add options" }).click();
	await page.getByRole("button", { name: "Apply and close" }).click();
	await expect(page.getByRole("menu")).toHaveCount(0);
	await expect(page.getByText("Custom close used")).toBeVisible();
});

test("Title chrome applies icon geometry and optional search settings exactly", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-window-system-title-chrome--content-contracts&viewMode=story",
	);
	await expect(page.getByRole("button", { name: "Icon only" })).toHaveClass(
		/is-icon-only/u,
	);
	await expect(page.getByRole("button", { name: "Label and icon" })).not.toHaveClass(
		/is-icon-only/u,
	);

	await page.goto(
		"/iframe.html?id=tosklight-window-system-title-chrome--search-with-and-without-settings&viewMode=story",
	);
	await expect(page.getByRole("button", { name: /search settings/iu })).toHaveCount(1);
});

test("Title chrome parity story uses the shared model on both surfaces", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=tosklight-window-system-title-chrome--window-modal-parity&viewMode=story",
	);
	await expect(page.locator(".ui-window-header .ui-title-chrome")).toHaveCount(1);
	await expect(page.locator(".ui-modal-titlebar .ui-title-chrome")).toHaveCount(1);
	await expect(page.getByRole("tablist")).toHaveCount(2);
	await expect(page.getByRole("textbox", { name: "Shared search" })).toHaveCount(2);
});
