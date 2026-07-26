import { readFileSync } from "node:fs";
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
	Button: "controls-production-controls--button-playground",
	FormLayout: "controls-production-controls--forms",
	FormField: "controls-production-controls--forms",
	Field: "controls-production-controls--forms",
	TextInput: "controls-production-controls--forms",
	NumberInput: "controls-production-controls--forms",
	Input: "controls-production-controls--forms",
	TextArea: "controls-production-controls--forms",
	LargeTextInput: "controls-production-controls--forms",
	TextField: "controls-production-controls--forms",
	NumberField: "controls-production-controls--forms",
	TextAreaField: "controls-production-controls--forms",
	LargeTextField: "controls-production-controls--forms",
	MultiValueToggle: "controls-production-controls--forms",
	MultiValueToggleField: "controls-production-controls--forms",
	SelectField: "controls-production-controls--forms",
	Select: "controls-production-controls--forms",
	CheckboxField: "controls-production-controls--forms",
	SwitchField: "controls-production-controls--forms",
	IconPickerField: "controls-production-controls--forms",
	ColorPickerField: "controls-production-controls--forms",
	FileDropField: "controls-production-controls--forms",
	GroupedSelectionField: "controls-production-controls--forms",
	SearchBar: "controls-production-controls--search-and-touch-select",
	TouchSelect: "controls-production-controls--search-and-touch-select",
	HorizontalFader: "faders-horizontal-fader--default",
	HorizontalFaderField: "faders-horizontal-fader--states",
	HorizontalTouchFader: "faders-horizontal-fader--default",
	InputModal: "input-keyboard-and-numpad--input-modal-configurations",
	ModalNumberInput: "input-keyboard-and-numpad--number-pad",
	ModalNumberValue: "input-keyboard-and-numpad--number-pad",
	ModalNumberEditor: "input-keyboard-and-numpad--number-pad",
	ModalTextKeyboard: "input-keyboard-and-numpad--keyboard",
	ModalCaretValue: "input-keyboard-and-numpad--keyboard",
	ModalPortal: "modals-production-modal-stack--portal-primitive",
	ModalTitleBar: "modals-production-modal-stack--title-bar-configuration",
	TitleBarSearchDivider:
		"modals-production-modal-stack--title-bar-configuration",
	ModalProvider: "modals-production-modal-stack--three-deep",
	ModalLayer: "modals-production-modal-stack--close-policies",
	ModalFrame: "modals-production-modal-stack--close-policies",
	ModalRegistration: "modals-production-modal-stack--application-registration",
	WindowHeader: "windows-production-window-kit--header-configurations",
	WindowSettings: "windows-production-window-kit--settings-configurations",
	WindowFrame: "windows-production-window-kit--configuration",
	WindowScrollArea: "windows-production-window-kit--scroll-and-empty-states",
	DataTable: "tables-generic-table--interactive",
	ButtonGrid: "windows-production-window-kit--pool-grid",
	GridButton: "windows-production-window-kit--pool-grid",
	SelectionList: "windows-production-window-kit--selection-list-states",
	SelectionTree: "windows-production-window-kit--multi-step-selection",
	FixtureSheetTableView: "tables-fixture-sheet--step-selection",
	VerticalTouchFaderSurface: "faders-vertical-touch-fader--software",
	TouchValueButton: "faders-vertical-touch-fader--direct-value-button",
	FaderView: "faders-vertical-touch-fader--fader-view-composition",
	TouchEncoder: "encoders-production-encoder-surfaces--individual-touch",
	HardwareEncoderDisplayView:
		"encoders-production-encoder-surfaces--individual-hardware",
	EncoderSection: "encoders-production-encoder-surfaces--configurable-family",
	GridDesktop: "desktop-24-×-18-grid-manager--constrained-placement",
	PaneView: "desktop-24-×-18-grid-manager--drag-and-resize",
	TouchPlaybackCardView: "playbacks-playback-bank--touch-bank",
	HardwarePlaybackCardView: "playbacks-playback-bank--hardware-bank",
	HardwareCueRowsView: "playbacks-playback-bank--hardware-bank",
	HardwarePlaybackFaderView: "playbacks-playback-bank--hardware-bank",
	PlaybackBankView: "playbacks-playback-bank--deterministic-bank",
	VirtualPlaybackGridView: "playbacks-virtual-playback-grid--sparse-grid",
	PoolCard: "pools-production-pool-cards--scaling-and-every-state",
	PoolGrid: "pools-generic-pool-window--sparse",
	PoolWindow: "pools-generic-pool-window--sparse",
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

test("Storybook uses the exact application background token", () => {
	expect(applicationBackground).toBe("#07090c");
	expect(packageBackground).toBe(applicationBackground);
});

test("DMX application stories render the production matrix, inspector, and source mutations", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1496, height: 761 });
	await page.goto(
		"/iframe.html?id=application-windows-dmx--values-output-summary&viewMode=story",
	);
	await expect(page.getByText("DMX Output", { exact: true })).toBeVisible();
	await expect(
		page.getByRole("button", { name: "Values as dots" }),
	).toBeVisible();
	await expect(page.getByRole("button", { name: "Sources" })).toBeVisible();
	await expect(page.locator(".dmx-universe")).toHaveCount(2);
	await expect(page.locator(".dmx-universe button")).toHaveCount(1_024);
	await expect(page.locator(".dmx-info-pane")).toContainText("Output summary");
	await expect(page.locator(".ui-data-table")).toHaveCount(0);
	await expect(page.locator(".ui-selection-tree")).toHaveCount(0);

	await page.goto(
		"/iframe.html?id=application-windows-dmx--selected-patched-channel&viewMode=story",
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
		"/iframe.html?id=application-windows-dmx--sources-with-overrides&viewMode=story",
	);
	const source = page.locator(".dmx-detail-list article").first();
	await source.getByRole("button", { name: "Release" }).click();
	await expect(
		page.locator('output[aria-label="Last DMX mutation"]'),
	).toHaveText("1.13:released");

	await page.goto(
		"/iframe.html?id=application-windows-dmx--sources-empty&viewMode=story",
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
		"/iframe.html?id=application-windows-help--quick-start&viewMode=story",
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
		"/iframe.html?id=application-windows-help--loading&viewMode=story",
	);
	await expect(page.getByText("Loading help…")).toBeVisible();
	await page.goto(
		"/iframe.html?id=application-windows-help--empty-catalog&viewMode=story",
	);
	await expect(page.getByText("No help topics found.")).toBeVisible();
	await page.goto(
		"/iframe.html?id=application-windows-help--catalog-error&viewMode=story",
	);
	await expect(
		page.getByText("Unable to load help: Catalog request failed"),
	).toBeVisible();
	await page.goto(
		"/iframe.html?id=application-windows-help--catalog-warning&viewMode=story",
	);
	await expect(
		page.getByText("One optional help topic could not be indexed."),
	).toBeVisible();
});

test("application shell stories preserve Dock and software or hardware control modes", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1496, height: 761 });
	await page.goto(
		"/iframe.html?id=application-shell-and-control--dock-desktops&viewMode=story",
	);
	await expect(page.locator(".left-dock")).toBeVisible();
	await expect(page.getByRole("button", { name: "DESKTOPS" })).toBeVisible();
	await expect(
		page.getByRole("button", { name: /New desktop/u }),
	).toBeVisible();

	await page.goto(
		"/iframe.html?id=application-shell-and-control--dock-built-ins&viewMode=story",
	);
	await expect(page.getByRole("button", { name: "Stage" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Fixtures" })).toBeVisible();

	await page.goto(
		"/iframe.html?id=application-shell-and-control--programmer-software&viewMode=story",
	);
	await expect(
		page.locator(".control-section.programmer.touch-connected"),
	).toBeVisible();
	await expect(page.getByRole("textbox", { name: "Command line" })).toHaveValue(
		"FIXTURE 1 AT 68",
	);
	await expect(page.locator(".touch-encoder")).toHaveCount(4);
	await expect(page.locator(".numeric-pad")).toBeVisible();

	await page.goto(
		"/iframe.html?id=application-shell-and-control--playbacks-hardware-connected&viewMode=story",
	);
	await expect(
		page.locator(".control-section.playbacks.hardware-connected"),
	).toBeVisible();
	await expect(
		page.locator('[data-playback-bank-mode="hardware"]'),
	).toBeVisible();
	await expect(page.locator(".hardware-encoder-display")).toHaveCount(6);
});

test("application control stories cover parameter families, playback banks, and keypad actions", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1496, height: 761 });
	await page.goto(
		"/iframe.html?id=application-shell-and-control--parameter-families-and-touch-encoders&viewMode=story",
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
		"/iframe.html?id=application-shell-and-control--playback-bank-touch&viewMode=story",
	);
	await expect(page.locator('[data-playback-bank-mode="touch"]')).toBeVisible();
	await expect(
		page.locator('[data-playback-bank-mode="touch"] > article'),
	).toHaveCount(4);

	await page.goto(
		"/iframe.html?id=application-shell-and-control--keypad-programmer-fade-preload-highlight-and-step&viewMode=story",
	);
	await expect(page.getByText("Prog. Fade", { exact: true })).toBeVisible();
	await expect(page.getByRole("button", { name: "PRELOAD GO" })).toBeVisible();
	await expect(page.getByRole("button", { name: "HIGH" })).toBeVisible();
	await expect(page.getByRole("button", { name: "PREV" })).toBeVisible();
	await expect(page.getByRole("button", { name: "NEXT" })).toBeVisible();
});

test("touch and hardware encoder stories exercise continuous input, modal entry, and release", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=encoders-production-encoder-surfaces--individual-touch&viewMode=story",
	);
	const encoder = page.getByRole("group", { name: "Enc 1 · Dimmer" });
	const value = encoder.locator("header strong");
	await expect(value).toHaveText("52%");
	await encoder.locator(".touch-encoder-tap-positive").click();
	await expect(value).toHaveText("53%");
	await encoder.locator(".touch-encoder-tap-negative").click();
	await expect(value).toHaveText("52%");
	await encoder.hover();
	await page.mouse.wheel(0, -100);
	await expect(value).toHaveText("53%");
	await page.keyboard.down("Shift");
	await page.mouse.wheel(0, 100);
	await page.keyboard.up("Shift");
	await expect(value).toHaveText("43%");

	const box = await encoder.boundingBox();
	expect(box).not.toBeNull();
	await page.mouse.move((box?.x ?? 0) + 30, (box?.y ?? 0) + 180);
	await page.mouse.down();
	await page.mouse.move((box?.x ?? 0) + 30, (box?.y ?? 0) + 160);
	await expect(encoder.getByText("Up · Fine")).toBeVisible();
	await page.waitForTimeout(90);
	await page.mouse.move((box?.x ?? 0) + 30, (box?.y ?? 0) + 110);
	await expect(encoder.getByText("Up · Coarse")).toBeVisible();
	await page.waitForTimeout(90);
	await page.mouse.up();
	await expect(value).not.toHaveText("43%");

	await encoder.getByRole("button", { name: "Set Value" }).click();
	const touchEditor = page.getByRole("dialog", {
		name: "Enc 1 · Dimmer value",
	});
	await touchEditor.getByRole("button", { name: "7" }).click();
	await touchEditor.getByRole("button", { name: "5" }).click();
	await touchEditor.getByRole("button", { name: "ENTER" }).click();
	await expect(value).toHaveText("75%");
	await encoder.getByRole("button", { name: "Set Value" }).click();
	await touchEditor.getByRole("button", { name: "Release" }).click();
	await expect(value).toHaveText("Released");

	for (const constrained of [
		"individual-touch-disabled",
		"individual-touch-indexed",
	]) {
		await page.goto(
			`/iframe.html?id=encoders-production-encoder-surfaces--${constrained}&viewMode=story`,
		);
		const constrainedEncoder = page.getByRole("group");
		await expect(constrainedEncoder).toHaveAttribute("aria-disabled", "true");
		await expect(
			constrainedEncoder.getByRole("button", { name: "Set Value" }),
		).toBeDisabled();
	}

	await page.goto(
		"/iframe.html?id=encoders-production-encoder-surfaces--individual-hardware&viewMode=story",
	);
	await page
		.getByRole("button", { name: "Encoder 1: Pan, 20°" })
		.click();
	const hardwareEditor = page.getByRole("dialog", {
		name: "Encoder 1 value",
	});
	await hardwareEditor.getByRole("button", { name: "Release Pan" }).click();
	await expect(
		page.getByRole("button", { name: "Encoder 1: Pan, Released" }),
	).toBeVisible();
});

test("playback bank states are explicit, valid, and preserve readable geometry", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=playbacks-playback-bank--faderless-bump&viewMode=story",
	);
	const bump = page.locator('[data-playback-slot="3"]');
	await expect(bump.getByText("3 · Bump", { exact: true })).toBeVisible();
	await expect(bump.getByRole("button", { name: "FLASH" })).toBeVisible();
	await expect(bump.getByText("PICKUP", { exact: true })).toHaveCount(0);

	for (const [story, label] of [
		["loaded-next", "LOADED"],
		["held-flash", "FLASH HELD"],
		["held-swap", "SWAP HELD"],
	] as const) {
		await page.goto(
			`/iframe.html?id=playbacks-playback-bank--${story}&viewMode=story`,
		);
		await expect(page.getByRole("status")).toHaveText(label);
	}

	await page.goto(
		"/iframe.html?id=playbacks-playback-bank--pickup-required&viewMode=story",
	);
	await expect(page.getByRole("status")).toHaveCount(0);
	await expect(page.getByText("Physical 62% · Target 0%")).toBeVisible();
	await expect(page.getByText("Lower to 0%")).toBeVisible();
	await expect(page.locator(".hardware-fader-pickup-difference")).toBeVisible();
	await expect(page.locator("article.pickup-required")).toHaveCount(0);

	for (const story of [
		"touch-never-shows-pickup",
		"faderless-never-shows-pickup",
	]) {
		await page.goto(
			`/iframe.html?id=playbacks-playback-bank--${story}&viewMode=story`,
		);
		await expect(page.locator(".hardware-fader-pickup-difference")).toHaveCount(
			0,
		);
		await expect(page.getByText(/Physical \d+%/)).toHaveCount(0);
	}

	await page.goto(
		"/iframe.html?id=playbacks-playback-bank--only-one-of-multiple-faders-requires-pickup&viewMode=story",
	);
	await expect(page.locator(".hardware-fader-pickup-difference")).toHaveCount(
		1,
	);

	await page.goto(
		"/iframe.html?id=playbacks-playback-bank--pickup-approach-and-release-from-below&viewMode=story",
	);
	const pickupSlider = page.getByRole("slider").first();
	await pickupSlider.fill("70");
	await expect(page.locator(".hardware-fader-pickup-difference")).toHaveCount(
		0,
	);

	await page.goto(
		"/iframe.html?id=playbacks-playback-bank--pickup-authority-replacement&viewMode=story",
	);
	await expect(page.locator(".hardware-fader-pickup-difference")).toHaveCount(
		1,
	);
	await page
		.getByRole("button", { name: "Replace hardware authority" })
		.click();
	await expect(page.locator(".hardware-fader-pickup-difference")).toHaveCount(
		0,
	);

	await page.goto(
		"/iframe.html?id=playbacks-playback-bank--zero-mid-full-levels&viewMode=story",
	);
	const faders = page.locator(".vertical-touch-fader");
	await expect(faders).toHaveCount(3);
	const fills = await faders.evaluateAll((elements) =>
		elements.map((element) => ({
			background: getComputedStyle(element, "::before").backgroundImage,
			transform: getComputedStyle(element, "::before").transform,
		})),
	);
	expect(
		fills.every(({ background }) => background.includes("linear-gradient")),
	).toBe(true);
	expect(new Set(fills.map(({ transform }) => transform)).size).toBe(3);

	await page.goto(
		"/iframe.html?id=playbacks-playback-bank--running-with-gradient&viewMode=story",
	);
	const modeBounds = await page
		.getByText("Cue 4 · Solo", { exact: true })
		.boundingBox();
	const valueBounds = await page
		.getByText("62%", { exact: true })
		.boundingBox();
	expect(modeBounds).not.toBeNull();
	expect(valueBounds).not.toBeNull();
	expect(modeBounds!.y + modeBounds!.height).toBeLessThanOrEqual(
		valueBounds!.y,
	);

	await page.goto(
		"/iframe.html?id=playbacks-playback-bank--long-labels&viewMode=story",
	);
	const header = page.locator(".hardware-playback-card > header");
	const title = header.locator(".playback-software-representation");
	const address = header.locator("strong");
	const [headerBox, titleBox, addressBox] = await Promise.all([
		header.boundingBox(),
		title.boundingBox(),
		address.boundingBox(),
	]);
	expect(titleBox!.width).toBeGreaterThan(addressBox!.width);
	expect(addressBox!.x + addressBox!.width).toBeLessThanOrEqual(
		headerBox!.x + headerBox!.width,
	);
	await expect(page.locator(".hardware-cue-row > i")).toHaveCount(0);
	for (const button of await page
		.locator(".hardware-playback-controls footer button")
		.all()) {
		expect(
			await button.evaluate(
				(element) => element.scrollWidth <= element.clientWidth,
			),
		).toBe(true);
	}
});

test("hardware pickup uses explicit physical and target positions and clears safely", async ({
	page,
}) => {
	const expectPickupGeometry = async ({
		physical,
		story,
		target,
	}: {
		physical: number;
		story: string;
		target: number;
	}) => {
		await page.goto(
			`/iframe.html?id=playbacks-playback-bank--${story}&viewMode=story`,
		);
		const track = page.locator(".hardware-fader-track");
		const fill = track.locator(".hardware-fader-fill");
		const difference = track.locator(".hardware-fader-pickup-difference");
		const physicalMarker = track.locator(".hardware-fader-physical-marker");
		const targetMarker = track.locator(".hardware-fader-target-marker");
		const [trackBox, fillBox, differenceBox, physicalBox, targetBox] =
			await Promise.all([
				track.boundingBox(),
				fill.boundingBox(),
				difference.boundingBox(),
				physicalMarker.boundingBox(),
				targetMarker.boundingBox(),
			]);
		const trackBottom = (trackBox?.y ?? 0) + (trackBox?.height ?? 0);
		const boundary = (percent: number) =>
			trackBottom - (trackBox?.height ?? 0) * percent;
		expect(Math.abs((fillBox?.y ?? 0) - boundary(physical))).toBeLessThan(1);
		expect(
			Math.abs(
				(differenceBox?.height ?? 0) -
					(trackBox?.height ?? 0) * Math.abs(target - physical),
			),
		).toBeLessThan(1);
		expect(
			Math.abs(
				(physicalBox?.y ?? 0) +
					(physicalBox?.height ?? 0) / 2 -
					boundary(physical),
			),
		).toBeLessThan(1);
		expect(
			Math.abs(
				(targetBox?.y ?? 0) +
					(targetBox?.height ?? 0) / 2 -
					boundary(target),
			),
		).toBeLessThan(1);
	};
	await expectPickupGeometry({
		story: "pickup-raise",
		physical: 0.5,
		target: 0.75,
	});
	await expectPickupGeometry({
		story: "pickup-lower",
		physical: 0.75,
		target: 0.5,
	});

	await page.goto(
		"/iframe.html?id=playbacks-playback-bank--pickup-required&viewMode=story",
	);
	await expect(page.getByRole("status")).toHaveCount(0);
	await expect(page.getByText("Physical 62% · Target 0%")).toBeVisible();
	await expect(page.getByText("Lower to 0%")).toBeVisible();
	await expect(page.locator(".hardware-fader-pickup-difference")).toHaveCount(
		1,
	);
	await expect(page.locator("article.pickup-required")).toHaveCount(0);

	for (const story of [
		"touch-never-shows-pickup",
		"faderless-never-shows-pickup",
	]) {
		await page.goto(
			`/iframe.html?id=playbacks-playback-bank--${story}&viewMode=story`,
		);
		await expect(page.locator(".hardware-fader-pickup-difference")).toHaveCount(
			0,
		);
		await expect(page.getByText(/Physical \d+%/)).toHaveCount(0);
	}

	await page.goto(
		"/iframe.html?id=playbacks-playback-bank--only-one-of-multiple-faders-requires-pickup&viewMode=story",
	);
	await expect(page.locator(".hardware-fader-pickup-difference")).toHaveCount(
		1,
	);

	await page.goto(
		"/iframe.html?id=playbacks-playback-bank--pickup-approach-and-release-from-below&viewMode=story",
	);
	await page.getByRole("slider").first().fill("70");
	await expect(page.locator(".hardware-fader-pickup-difference")).toHaveCount(
		0,
	);

	await page.goto(
		"/iframe.html?id=playbacks-playback-bank--pickup-authority-replacement&viewMode=story",
	);
	await expect(page.locator(".hardware-fader-pickup-difference")).toHaveCount(
		1,
	);
	await page
		.getByRole("button", { name: "Replace hardware authority" })
		.click();
	await expect(page.locator(".hardware-fader-pickup-difference")).toHaveCount(
		0,
	);

	await page.goto(
		"/iframe.html?id=playbacks-playback-bank--pickup-hardware-disconnected-and-reconnected&viewMode=story",
	);
	await expect(page.locator(".hardware-fader-pickup-difference")).toHaveCount(1);
	await page.getByRole("button", { name: "Disconnect hardware" }).click();
	await expect(page.locator(".hardware-fader-pickup-difference")).toHaveCount(0);
	await page.getByRole("button", { name: "Reconnect hardware" }).click();
	await expect(page.locator(".hardware-fader-pickup-difference")).toHaveCount(1);
});

test("application Stage stories render deterministic 2D fixtures and the real 3D canvas", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1496, height: 761 });
	await page.goto(
		"/iframe.html?id=application-windows-stage--stage-2-d&viewMode=story",
	);
	await expect(page.locator(".stage-fixture")).toHaveCount(5);
	await expect(page.locator(".stage-fixture.selected")).toHaveCount(1);

	await page.goto(
		"/iframe.html?id=application-windows-stage--stage-3-d&viewMode=story",
	);
	await expect(page.locator(".stage-3d-canvas")).toBeVisible();
	await expect(page.locator(".stage-3d-canvas canvas")).toBeVisible();
});

test("generic and application-owned pool stories preserve their slot contracts", async ({
	page,
}) => {
	for (const storyId of [
		"pools-generic-pool-window--empty",
		"pools-generic-pool-window--sparse",
	]) {
		await page.goto(`/iframe.html?id=${storyId}&viewMode=story`);
		await expect(page.locator(".pool-card")).toHaveCount(200);
	}
	await page.goto(
		"/iframe.html?id=application-windows-pools--groups&viewMode=story",
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
		"/iframe.html?id=application-windows-pools--presets&viewMode=story",
	);
	await expect(page.locator(".preset-card")).toHaveCount(200);
	await expect(page.locator(".preset-card").nth(4)).toHaveAttribute(
		"data-pool-slot-id",
		"2.5",
	);
	await expect(page.locator(".preset-card").last()).toHaveClass(
		/store-target/u,
	);
	await page.locator(".preset-card").nth(4).click();
	await expect(page.getByLabel("Preset pool interaction")).toHaveText(
		"Activated Color 5",
	);

	await page.goto(
		"/iframe.html?id=application-windows-cuelists-and-cues--pool&viewMode=story",
	);
	await expect(page.locator(".cuelist-card")).toHaveCount(1000);
	await expect(page.locator(".cuelist-card").first()).toHaveAttribute(
		"data-pool-slot-id",
		"1",
	);
	await page.getByRole("textbox", { name: "Search Cuelists" }).fill("Main");
	await expect(page.locator(".cuelist-card")).toHaveCount(1);
	await page.getByRole("textbox", { name: "Search Cuelists" }).fill("");
	await expect(page.locator(".cuelist-card")).toHaveCount(1000);

	await page.goto(
		"/iframe.html?id=pools-generic-pool-window--extended&viewMode=story",
	);
	await expect(page.locator(".pool-card")).toHaveCount(260);
});

test("the narrow generic pool keeps touch-sized boxes in a scrollable window", async ({
	page,
}) => {
	await page.setViewportSize({ width: 430, height: 844 });
	await page.goto(
		"/iframe.html?id=pools-generic-pool-window--narrow&viewMode=story",
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
		"pools-generic-pool-window--empty",
		"pools-generic-pool-window--sparse",
		"pools-generic-pool-window--narrow-tall",
		"pools-generic-pool-window--narrow-short",
		"pools-generic-pool-window--wide-tall",
		"pools-generic-pool-window--wide-short",
		"pools-generic-pool-window--extended",
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
			(widths.get("pools-generic-pool-window--narrow-tall") ?? 0) -
				(widths.get("pools-generic-pool-window--narrow-short") ?? 0),
		),
	).toBeLessThanOrEqual(1);
	expect(
		Math.abs(
			(widths.get("pools-generic-pool-window--wide-tall") ?? 0) -
				(widths.get("pools-generic-pool-window--wide-short") ?? 0),
		),
	).toBeLessThanOrEqual(1);

	await page.goto(
		"/iframe.html?id=pools-generic-pool-window--extended&viewMode=story",
	);
	const extendedScroller = page.locator(".ui-window-scroller");
	expect(
		await extendedScroller.evaluate(
			(node) => node.scrollHeight > node.clientHeight,
		),
	).toBe(true);

	await page.goto(
		"/iframe.html?id=pools-generic-pool-window--live-resize&viewMode=story",
	);
	const before = await expectSquarePool(page);
	await page.getByRole("button", { name: "Resize pool viewport" }).click();
	await expect
		.poll(async () => (await poolGeometry(page, ".pool-card"))[0]?.width)
		.not.toBe(before.width);
	await expectSquarePool(page);

	for (const [storyId, selector] of [
		["application-windows-pools--groups-narrow-short", ".group-card"],
		["application-windows-pools--groups-wide-tall", ".group-card"],
		["application-windows-pools--presets-narrow-short", ".preset-card"],
		["application-windows-pools--presets-wide-tall", ".preset-card"],
		[
			"application-windows-cuelists-and-cues--pool-narrow-short",
			".cuelist-card",
		],
		["application-windows-cuelists-and-cues--pool-wide-tall", ".cuelist-card"],
	] as const) {
		await page.goto(`/iframe.html?id=${storyId}&viewMode=story`);
		await expectSquarePool(page, selector);
	}
});

test("configured colors and derived or frozen states use complete outlines and readable markers", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=pools-generic-pool-window--every-card-state&viewMode=story",
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
		"/iframe.html?id=application-windows-pools--groups-status-markers&viewMode=story",
	);
	await expect(page.getByLabel(/Derived state/u)).toBeVisible();
	await expect(page.getByLabel(/Frozen state/u)).toBeVisible();
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
		"/iframe.html?id=application-virtual-playbacks--sparse-grid&viewMode=story",
	);
	await expect(page.locator(".virtual-playback-box")).toHaveCount(12);
	await expect(page.locator(".playback-fader-bank")).toHaveCount(0);
	await expect(page.locator('input[type="range"]')).toHaveCount(0);
	await expect(
		page.getByRole("button", { name: /cell 2 empty/u }),
	).toBeVisible();

	await page.goto(
		"/iframe.html?id=application-virtual-playbacks--page-switching&viewMode=story",
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
		"/iframe.html?id=application-virtual-playbacks--sparse-grid&viewMode=story",
	);
	await page.getByRole("button", { name: /cell 1 Main/u }).click();
	await expect(page.getByRole("status")).toHaveText("Action 7");

	await page.goto(
		"/iframe.html?id=application-virtual-playbacks--held-flash-and-swap&viewMode=story",
	);
	const flash = page.getByRole("button", { name: /cell 1 Bump/u });
	const bounds = await flash.boundingBox();
	expect(bounds).not.toBeNull();
	await page.mouse.move(
		bounds!.x + bounds!.width / 2,
		bounds!.y + bounds!.height / 2,
	);
	await page.mouse.down();
	await expect(page.getByRole("status")).toHaveText("Pressed 8");
	await page.mouse.up();
	await expect(page.getByRole("status")).toHaveText("Released 8");
});

test("the application Virtual Playbacks stories cover adapter-owned targeting and availability states", async ({
	page,
}) => {
	for (const [story, selector, className] of [
		["configuration-state", '[data-grid-position="0"]', "configuration-armed"],
		["assignment-state", '[data-grid-position="1"]', "assignment-pending"],
		["update-state", '[data-grid-position="0"]', "update-target"],
		["exclusion-zone-state", '[data-grid-position="3"]', "exclusion-selected"],
	] as const) {
		await page.goto(
			`/iframe.html?id=application-virtual-playbacks--${story}&viewMode=story`,
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
		"/iframe.html?id=application-virtual-playbacks--unavailable-slots&viewMode=story",
	);
	await expect(
		page.getByRole("button", { name: /cell 128 unavailable/u }),
	).toBeDisabled();
});

test("Virtual Playback cards use outline, full-fill, edge-status, and artwork hierarchy", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=application-virtual-playbacks--running-transition&viewMode=story",
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
	const running = await card.evaluate((element) => {
		const style = getComputedStyle(element);
		return {
			background: style.backgroundColor,
			border: style.borderTopColor,
		};
	});
	expect(running.background).toBe(running.border);
	expect(running.background).not.toBe(inactive.background);
	await page.getByRole("button", { name: "Toggle running" }).click();
	await expect(card).not.toHaveClass(/running/u);
	expect(
		await card.evaluate((element) => getComputedStyle(element).backgroundColor),
	).toBe(inactive.background);

	await page.goto(
		"/iframe.html?id=application-virtual-playbacks--icon-and-image-artwork&viewMode=story",
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
		["assignment-state", "Record"],
		["update-state", "Update"],
	] as const) {
		await page.goto(
			`/iframe.html?id=application-virtual-playbacks--${story}&viewMode=story`,
		);
		await expect(page.locator(".pool-card-workflow").first()).toHaveText(label);
	}
});

test("nested modal story keeps deterministic stack order and top-only Escape", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=modals-production-modal-stack--three-deep&viewMode=story",
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
		"/iframe.html?id=modals-production-modal-stack--close-policies&viewMode=story",
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
		"/iframe.html?id=modals-production-modal-stack--title-bar-configuration&viewMode=story",
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
		"/iframe.html?id=modals-production-modal-stack--programmatic-close&viewMode=story",
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
		"/iframe.html?id=modals-production-modal-stack--title-bar-configuration&viewMode=story",
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
	const [labelBox, switchBox, stateBox] = await Promise.all([
		switchField.locator(":scope > label").boundingBox(),
		switchField.locator(".ui-switch-track").boundingBox(),
		switchField.getByText(/^(On|Off)$/u).boundingBox(),
	]);
	expect(labelBox).not.toBeNull();
	expect(switchBox).not.toBeNull();
	expect(stateBox).not.toBeNull();
	expect(Math.abs(labelBox!.y - switchBox!.y)).toBeLessThan(24);
	expect(switchBox!.x + switchBox!.width).toBeLessThanOrEqual(stateBox!.x);

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
		expect(geometry.width).toBe(8);
		expect(geometry.background).not.toBe("rgba(0, 0, 0, 0)");
		expect(geometry.lineWidth).toBeGreaterThan(0);
		expect(geometry.lineColor).toContain("196");
	}

	await page.goto(
		"/iframe.html?id=modals-production-modal-stack--search-without-adjacent-buttons&viewMode=story",
	);
	await expect(page.locator(".ui-titlebar-search-divider")).toHaveCount(0);

	await page.goto(
		"/iframe.html?id=modals-production-modal-stack--window-title-bar-search&viewMode=story",
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

test("title-bar search dividers remain one device pixel at DPR 1 and 2", async ({
	browser,
}) => {
	for (const deviceScaleFactor of [1, 2]) {
		const context = await browser.newContext({
			deviceScaleFactor,
			viewport: { width: 1280, height: 800 },
		});
		const dprPage = await context.newPage();
		await dprPage.goto(
			"/iframe.html?id=modals-production-modal-stack--title-bar-configuration&viewMode=story",
		);
		const divider = dprPage.locator(".ui-titlebar-search-divider").first();
		await expect(divider).toBeVisible();
		const cssWidth = await divider.evaluate((element) =>
			Number.parseFloat(getComputedStyle(element, "::after").width),
		);
		expect(cssWidth * deviceScaleFactor).toBeCloseTo(1, 5);
		await context.close();
	}
});

test("window and modal chrome use the same standard search geometry", async ({
	page,
}) => {
	const measure = async () =>
		page.locator(".console-search").evaluate((element) => {
			return {
				inputHeight: Math.round(
					element.querySelector("input")?.getBoundingClientRect().height ?? 0,
				),
				clearHeight: Math.round(
					element
						.querySelector('[aria-label="Clear search"]')
						?.getBoundingClientRect().height ?? 0,
				),
				usesStandardClass: element.classList.contains("console-search"),
			};
		});
	await page.goto(
		"/iframe.html?id=windows-production-window-kit--configuration&viewMode=story",
	);
	const windowSearch = await measure();
	await page.goto(
		"/iframe.html?id=modals-production-modal-stack--title-bar-configuration&viewMode=story",
	);
	const modalSearch = await measure();
	expect(modalSearch).toEqual(windowSearch);
	expect(modalSearch.inputHeight).toBeGreaterThanOrEqual(44);
});

test("desktop story uses the real 24 × 18 non-overlapping geometry", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=desktop-24-%C3%97-18-grid-manager--constrained-placement&viewMode=story",
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
		"/iframe.html?id=desktop-24-%C3%97-18-grid-manager--drag-and-resize&viewMode=story",
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
		"/iframe.html?id=desktop-24-%C3%97-18-grid-manager--maximized&viewMode=story",
	);
	const maximized = page.getByRole("region", { name: "Fixture Sheet pane" });
	await expect(maximized).toHaveAttribute("aria-expanded", "true");
	await expect(maximized.locator(".pane-resize-handle")).toHaveCount(0);

	await page.goto(
		"/iframe.html?id=desktop-24-%C3%97-18-grid-manager--empty-grid&viewMode=story",
	);
	await page.getByRole("button", { name: /24 × 18 desktop grid/u }).click({
		position: { x: 760, y: 380 },
	});
	await expect(page.getByLabel("Requested grid rectangle")).not.toHaveText(
		"No window requested",
	);
});

for (const storyId of [
	"controls-production-controls--button-playground",
	"input-keyboard-and-numpad--number-pad",
	"faders-vertical-touch-fader--software",
	"encoders-production-encoder-surfaces--individual-touch",
	"application-virtual-playbacks--narrow-touch",
	"pools-generic-pool-window--narrow",
	"modals-production-modal-stack--close-policies",
	"modals-production-modal-stack--title-bar-configuration",
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

test("named application windows use production Fixture, Cuelist, Patch, Setup, and Development views", async ({
	page,
}) => {
	await page.goto(
		"/iframe.html?id=application-windows-fixture-sheet--selected-and-active-steps&viewMode=story",
	);
	await expect(page.getByText("Fixture Sheet", { exact: true })).toBeVisible();
	await expect(page.locator(".ui-data-table-row:not(.empty)")).toHaveCount(5);
	await expect(
		page.getByText("Stage Left Mover", { exact: true }),
	).toBeVisible();
	await expect(page.locator(".ui-data-table-row.selected")).toHaveCount(2);

	await page.goto(
		"/iframe.html?id=application-windows-cuelists-and-cues--pool&viewMode=story",
	);
	await expect(page.getByText("Cuelist Pool", { exact: true })).toBeVisible();
	await expect(page.getByText("Main Sequence", { exact: true })).toBeVisible();
	await page.goto(
		"/iframe.html?id=application-windows-cuelists-and-cues--cues-with-properties&viewMode=story",
	);
	await expect(
		page.getByText(/Cuelist View · Cuelist 1 · Main Sequence/u),
	).toBeVisible();
	await expect(page.getByRole("row")).toHaveCount(3);
	await expect(page.getByText("Opening Look", { exact: true })).toBeVisible();
	await expect(page.locator(".cue-properties")).toBeVisible();
	await page.goto(
		"/iframe.html?id=application-windows-cuelists-and-cues--fixed-cues-unavailable&viewMode=story",
	);
	await expect(
		page.getByText("Fixed Cuelist is unavailable", { exact: true }),
	).toBeVisible();

	await page.goto(
		"/iframe.html?id=application-windows-patch--empty-patch&viewMode=story",
	);
	await expect(page.getByText("Show Patch", { exact: true })).toBeVisible();
	await expect(page.locator(".patch-table")).toBeVisible();
	await expect(
		page.getByText("No fixtures in this layer.", { exact: true }),
	).toBeVisible();

	await page.goto(
		"/iframe.html?id=application-windows-setup--timecode&viewMode=story",
	);
	await expect(page.getByText("Desk Setup", { exact: true })).toBeVisible();
	await expect(page.getByRole("heading", { name: "Timecode" })).toBeVisible();
	await expect(page.getByText("ltc:", { exact: true })).toBeVisible();
	await expect(
		page.getByText("Fallback allowed", { exact: true }),
	).toBeVisible();

	for (const [story, heading] of [
		["forms", "Side labels"],
		["faders", "Vertical faders and optional actions"],
		["buttons", "Buttons"],
	] as const) {
		await page.goto(
			`/iframe.html?id=application-windows-development--${story}&viewMode=story`,
		);
		await expect(page.getByText("Development", { exact: true })).toBeVisible();
		await expect(page.getByRole("heading", { name: heading })).toBeVisible();
	}
});

test("Forms story keeps state, scrolling, fader, pickers, grouped selections, and file drop interactive", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1100, height: 720 });
	await page.goto(
		"/iframe.html?id=controls-production-controls--forms&viewMode=story",
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

	await canvas.evaluate((element) => {
		element.scrollTop = 0;
	});
	await page.getByRole("radio", { name: "3D" }).click();
	await expect(page.getByRole("radio", { name: "3D" })).toHaveAttribute(
		"aria-checked",
		"true",
	);

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
	const swatchBox = await color
		.locator(".ui-color-trigger-swatch")
		.boundingBox();
	expect(triggerBox).not.toBeNull();
	expect(swatchBox).not.toBeNull();
	expect(swatchBox!.x).toBeGreaterThan(triggerBox!.x);
	expect(swatchBox!.width).toBeLessThan(triggerBox!.width);
	await color.click();
	for (const choice of await page
		.getByRole("option", { name: /Use color/u })
		.all()) {
		const box = await choice.boundingBox();
		expect(Math.abs((box?.width ?? 0) - (box?.height ?? 0))).toBeLessThan(2);
	}
	await page.keyboard.press("Escape");

	await page.getByRole("button", { name: "Choose icon" }).click();
	await expect(page.getByRole("combobox", { name: "Icon group" })).toHaveValue(
		"gobo",
	);
	await page
		.getByRole("combobox", { name: "Icon group" })
		.selectOption("fixture-type");
	await expect(page.locator('[data-icon-group="fixture-type"]')).toBeVisible();
	await expect(page.getByLabel("Custom")).toHaveCount(0);
	await expect(
		page.getByRole("button", { name: "Use custom icon" }),
	).toHaveCount(0);
	await page.locator('[data-icon-group="fixture-type"] button').first().click();

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
		"/iframe.html?id=input-keyboard-and-numpad--input-modal-configurations&viewMode=story",
	);
	const numberModal = page.getByRole("dialog", { name: "Fade time" });
	await expect(numberModal.locator(".modal-caret-value > i")).toBeVisible();
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
		"/iframe.html?id=input-keyboard-and-numpad--empty-text-input-modal&viewMode=story",
	);
	const textModal = page.getByRole("dialog", { name: "Fixture name" });
	const preview = textModal.getByRole("textbox", {
		name: "Fixture name value",
	});
	await expect(preview).toHaveAccessibleName("Fixture name value");
	await expect(preview).toContainText("Enter fixture name");
	await expect(preview.locator("> i")).toBeVisible();
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
	const escape = textModal.getByRole("button", { name: /ESC/u });
	expect(
		await escape.evaluate(
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
		"/iframe.html?id=input-keyboard-and-numpad--multiline-input-modal&viewMode=story",
	);
	const multiline = page.getByRole("dialog", { name: "Fixture name" });
	const multilineEditor = multiline.getByRole("textbox", {
		name: "Fixture name value",
	});
	await expect(multilineEditor).toHaveAttribute("readonly", "");
	await expect(multilineEditor).toHaveCSS("overflow-y", "scroll");
	expect(
		await multilineEditor.evaluate(
			(editor) => editor.scrollHeight > editor.clientHeight,
		),
	).toBe(true);
	await multilineEditor.click({ position: { x: 14, y: 45 } });
	await expect
		.poll(() =>
			multilineEditor.evaluate(
				(editor: HTMLTextAreaElement) => editor.selectionStart,
			),
		)
		.toBe(11);
	await multiline.getByRole("button", { name: "X", exact: true }).click();
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
	await expect(
		multiline.getByRole("button", { name: "Enter · New line" }),
	).toBeVisible();
	await expect(multiline.getByRole("button", { name: "Done" })).toBeVisible();
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
});

test("production marketing and application modal stories preserve their real compositions and workflows", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1920, height: 1080 });
	await page.goto(
		"/iframe.html?id=application-marketing--complete-product-demo&viewMode=story",
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
		"/iframe.html?id=application-modal-workflows--playback-configuration&viewMode=story",
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
		"/iframe.html?id=application-modal-workflows--record-existing-target&viewMode=story",
	);
	await expect(
		page.getByRole("dialog", { name: "Record to Cuelist 1" }),
	).toBeVisible();
	await page.getByRole("button", { name: "Merge", exact: true }).click();
	await expect(page.getByLabel("Record mode choice")).toHaveText("merge");
});
