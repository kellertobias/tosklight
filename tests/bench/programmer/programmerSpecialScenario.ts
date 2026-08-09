import { expect, type Locator, type Page } from "@playwright/test";
import type { PatchedFixture } from "../../../apps/light-desktop/src/api/types";
import { replaceProgrammingSelection } from "../command-selection/programmingSelection";
import type { BrowserSelection } from "../command-selection/selectionScenario";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type { SimulatedHardware } from "../hardware/hardwareScenario";
import { batchProgrammerValues } from "./programmerValues";

export type PositionAlignMode = "left" | "right" | "out" | "in";
export type ControlSemantic =
	| "lamp_on"
	| "lamp_off"
	| "reset"
	| "fan_auto"
	| "fan_low"
	| "fan_high"
	| "fan_max";
export type BeamSpecialFamily = "Beam" | "Shapers";

type PositionAssignment = {
	fixture_id: string;
	attribute: "pan" | "tilt";
	value: number;
};
type ColorAssignment = {
	fixtureId: string;
	attribute: string;
	value: number;
};
type PickerColor = {
	hue: number;
	saturation: number;
	brightness: number;
};

const CONTROL_LABELS: Record<ControlSemantic, string> = {
	lamp_on: "Lamps On",
	lamp_off: "Lamp Off",
	reset: "Reset",
	fan_auto: "Fan Auto",
	fan_low: "Fan Low",
	fan_high: "Fan High",
	fan_max: "Fan Max",
};

export class BrowserProgrammerSpecials {
	private positionContract?: {
		selected: string[];
		home: PositionAssignment[];
		before: PositionAssignment[];
	};
	private colorContract?: {
		selected: string[];
		range: ColorAssignment[];
		prior: ColorAssignment[];
	};
	readonly position = {
		prepareReturnHomeContract: () => this.prepareReturnHomeContract(),
		returnHome: () => this.returnHome(),
		expectAtHome: () => this.expectPositionAssignments("home"),
		expectBeforeReturnHome: () => this.expectPositionAssignments("before"),
		expectUnavailable: () => this.expectReturnHomeUnavailable(),
		align: (mode: PositionAlignMode) => this.align(mode),
		alignViaApi: (mode: PositionAlignMode) => this.alignViaApi(mode),
	};
	readonly color = {
		prepareRangeContract: () => this.prepareColorRangeContract(),
		setUniform: () => this.setUniformColor(),
		applyRangeWithShift: () => this.applyColorRangeWithShift(),
		cancelRangeWithShift: () => this.cancelColorRangeWithShift(),
		applyRangeWithHardwareShift: () => this.applyColorRangeWithHardwareShift(),
		expectPrior: () => this.expectColorAssignments("prior"),
		expectRange: () => this.expectColorAssignments("range"),
		expectSelectionPreserved: () => this.expectColorSelection(),
	};
	readonly beam = new BrowserBeamSpecial(this, "Beam");
	readonly shapers = new BrowserBeamSpecial(this, "Shapers");
	readonly control = {
		invoke: (semantic: ControlSemantic) => this.controlAction(semantic),
		invokeViaApi: (semantic: ControlSemantic) =>
			this.controlActionViaApi(semantic),
	};

	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly selection: BrowserSelection,
		private readonly hardware?: SimulatedHardware,
		private readonly showId?: () => string,
	) {}

	private async prepareReturnHomeContract(): Promise<void> {
		const patch = await this.fixtures();
		const targets = patch.flatMap((fixture) => {
			const logicalByIndex = new Map(
				fixture.logical_heads.map((head) => [head.head_index, head.fixture_id]),
			);
			return fixture.definition.heads.flatMap((head) => {
				const fixtureId = head.shared
					? fixture.fixture_id
					: logicalByIndex.get(head.index);
				if (!fixtureId) return [];
				const home = (["pan", "tilt"] as const).flatMap((attribute) => {
					const parameter = head.parameters.find(
						(candidate) => candidate.attribute === attribute,
					);
					if (!parameter) return [];
					return [
						{
							fixture_id: fixtureId,
							attribute,
							value: Number.isFinite(parameter.default)
								? parameter.default
								: 0.5,
						},
					];
				});
				return home.length ? [{ fixtureId, home }] : [];
			});
		});
		expect(targets.length).toBeGreaterThanOrEqual(2);
		const chosen = [targets[1], targets[0]];
		const home = chosen.flatMap((target) => target.home);
		const before = home.map((assignment, index) => ({
			...assignment,
			value: index % 2 === 0 ? 0.91 : 0.09,
		}));
		const nonPosition = patch.find((fixture) =>
			fixture.definition.heads.every((head) =>
				head.parameters.every(
					(parameter) => !["pan", "tilt"].includes(parameter.attribute),
				),
			),
		);
		const selected = [
			chosen[0].fixtureId,
			...(nonPosition ? [nonPosition.fixture_id] : []),
			chosen[1].fixtureId,
		];
		await replaceProgrammingSelection(this.api, {
			surface: "api",
			showId: this.requiredShowId(),
			fixtures: selected,
		});
		await this.setPositionAssignments(before);
		this.positionContract = { selected, home, before };
	}

	async available(family: BeamSpecialFamily): Promise<string[]> {
		const selected = new Set((await this.selection.observe()).selected);
		const attributes = new Set<string>();
		for (const fixture of await this.fixtures()) {
			if (!fixtureIsSelected(fixture, selected)) continue;
			for (const head of fixture.definition.heads)
				for (const parameter of head.parameters)
					if (belongsToSpecialFamily(parameter.attribute, family))
						attributes.add(parameter.attribute);
		}
		return [...attributes].sort();
	}

	async setBeamValue(
		family: BeamSpecialFamily,
		attribute: string,
		percentage: number,
	): Promise<void> {
		if (!Number.isFinite(percentage) || percentage < 0 || percentage > 100)
			throw new Error("Special-dialog value must be between 0 and 100");
		const available = await this.available(family);
		if (!available.includes(attribute))
			throw new Error(
				`${attribute} is not supplied by the selected ${family} fixtures`,
			);
		const dialog = await this.openDialog(family);
		const slider = dialog.getByRole("slider", {
			name: attribute.replaceAll(".", " "),
			exact: true,
		});
		await expect(slider).toBeVisible();
		await pointerSet(this.page, slider, percentage);
		await this.closeDialog(dialog);
	}

	private async returnHome(): Promise<void> {
		if (this.hardware?.connected)
			await expect(
				this.page.locator(".control-section.hardware-connected"),
			).toBeVisible();
		const dialog = await this.openDialog("Position");
		const action = dialog.getByRole("button", {
			name: "Return Home",
			exact: true,
		});
		await expect(action).toBeEnabled();
		await this.desk.click(action);
		await this.closeDialog(dialog);
	}

	private async expectReturnHomeUnavailable(): Promise<void> {
		const dialog = await this.openDialog("Position");
		await expect(
			dialog.getByRole("button", { name: "Return Home", exact: true }),
		).toBeDisabled();
		await this.closeDialog(dialog);
	}

	private async expectPositionAssignments(
		key: "home" | "before",
	): Promise<void> {
		const contract = this.requiredPositionContract();
		await expectProgrammerAssignments(
			this.api,
			contract[key].map(({ fixture_id, attribute, value }) => ({
				fixtureId: fixture_id,
				attribute,
				value,
			})),
		);
	}

	private async setPositionAssignments(
		assignments: readonly PositionAssignment[],
	): Promise<void> {
		await batchProgrammerValues(this.api, {
			surface: "api",
			showId: this.requiredShowId(),
			mutations: assignments.map((assignment) => ({
				action: "set_fixture",
				fixtureId: assignment.fixture_id,
				attribute: assignment.attribute,
				value: { kind: "normalized", value: assignment.value },
				timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
			})),
		});
	}

	private requiredPositionContract() {
		if (!this.positionContract)
			throw new Error(
				"Call special.position.prepareReturnHomeContract() first",
			);
		return this.positionContract;
	}

	private async prepareColorRangeContract(): Promise<void> {
		const patch = await this.fixtures();
		const colorTargets = patch.flatMap((fixture) => {
			const logicalByIndex = new Map(
				fixture.logical_heads.map((head) => [head.head_index, head.fixture_id]),
			);
			return fixture.definition.heads.flatMap((head) => {
				const fixtureId = head.shared
					? fixture.fixture_id
					: logicalByIndex.get(head.index);
				const attributes = new Set(
					head.parameters.map((parameter) => parameter.attribute),
				);
				const supported = COLOR_ATTRIBUTES.some((attribute) =>
					attributes.has(attribute),
				);
				return fixtureId && supported
					? [{ fixtureId, logical: !head.shared }]
					: [];
			});
		});
		expect(colorTargets.length).toBeGreaterThanOrEqual(3);
		const logical = colorTargets.find((target) => target.logical);
		const chosen = [
			logical ?? colorTargets[2],
			...colorTargets
				.filter((target) => target.fixtureId !== logical?.fixtureId)
				.slice(0, 2),
		];
		const nonColor = patch.find((fixture) =>
			fixture.definition.heads.every((head) =>
				head.parameters.every(
					(parameter) => !parameter.attribute.startsWith("color."),
				),
			),
		);
		const selected = [
			chosen[0].fixtureId,
			chosen[1].fixtureId,
			...(nonColor ? [nonColor.fixture_id] : []),
			chosen[2].fixtureId,
		];
		const range = colorProgrammerAssignments(
			selected,
			patch,
			interpolatePickerRange(
				selected.length,
				COLOR_RANGE_START,
				COLOR_RANGE_END,
			),
		);
		const prior = range.map((assignment) => ({
			...assignment,
			value: 0.33,
		}));
		await replaceProgrammingSelection(this.api, {
			surface: "api",
			showId: this.requiredShowId(),
			fixtures: selected,
		});
		await this.setColorAssignments(prior);
		this.colorContract = { selected, range, prior };
	}

	private async setUniformColor(): Promise<void> {
		const contract = this.requiredColorContract();
		const uniformPoint = {
			hue: 0.35,
			saturation: 0.6,
			brightness: 0.85,
		};
		const expected = colorProgrammerAssignments(
			contract.selected,
			await this.fixtures(),
			contract.selected.map(() => uniformPoint),
		);
		const dialog = await this.openDialog("Color");
		const before = await this.programmerValuesRevision();
		await clickPicker(this.page, uniformPoint.hue, 1 - uniformPoint.saturation);
		await expect.poll(() => this.programmerValuesRevision()).toBe(before + 1);
		await expectProgrammerAssignments(this.api, expected);
		await this.closeDialog(dialog);
	}

	private async applyColorRangeWithShift(): Promise<void> {
		const contract = this.requiredColorContract();
		const dialog = await this.openDialog("Color");
		const before = await this.programmerValuesRevision();
		await this.page.keyboard.down("Shift");
		await beginPickerDrag(this.page, COLOR_RANGE_START, COLOR_RANGE_END);
		await expect(
			this.page.locator('.color-range-preview[data-active="true"]'),
		).toBeVisible();
		expect(await this.programmerValuesRevision()).toBe(before);
		await this.page.mouse.up();
		await this.page.keyboard.up("Shift");
		await expect.poll(() => this.programmerValuesRevision()).toBe(before + 1);
		await expect(
			this.page.locator('.color-range-preview[data-active="false"]'),
		).toBeVisible();
		await expectProgrammerAssignments(this.api, contract.range);
		await this.closeDialog(dialog);
	}

	private async cancelColorRangeWithShift(): Promise<void> {
		const dialog = await this.openDialog("Color");
		const before = await this.programmerValuesRevision();
		await this.page.keyboard.down("Shift");
		await beginPickerDrag(this.page, COLOR_RANGE_START, COLOR_RANGE_END);
		await expect(
			this.page.locator('.color-range-preview[data-active="true"]'),
		).toBeVisible();
		await this.page.locator(".color-sheet").dispatchEvent("pointercancel", {
			pointerId: 1,
			pointerType: "mouse",
		});
		await this.page.keyboard.up("Shift");
		await this.page.mouse.up();
		await expect(this.page.locator(".color-range-preview")).toHaveCount(0);
		expect(await this.programmerValuesRevision()).toBe(before);
		await this.closeDialog(dialog);
	}

	private async applyColorRangeWithHardwareShift(): Promise<void> {
		if (!this.hardware.connected)
			throw new Error("Hardware Color range requires hardware.connect() first");
		const contract = this.requiredColorContract();
		const alias = this.session().desk.osc_alias;
		const dialog = await this.openDialog("Color");
		try {
			await this.hardware.send(`/light/${alias}/programmer/shift`, [true]);
			await expect(
				this.page.locator('.color-sheet[data-range-shift="armed"]'),
			).toBeVisible();
			const before = await this.programmerValuesRevision();
			await beginPickerDrag(this.page, COLOR_RANGE_START, COLOR_RANGE_END);
			expect(await this.programmerValuesRevision()).toBe(before);
			await this.page.mouse.up();
			await expect.poll(() => this.programmerValuesRevision()).toBe(before + 1);
			await expectProgrammerAssignments(this.api, contract.range);
			await expect(
				this.page.locator('.color-sheet[data-range-shift="armed"]'),
			).toBeVisible();
			await this.hardware.send(`/light/${alias}/programmer/shift`, [false]);
			await expect(
				this.page.locator('.color-sheet[data-range-shift="idle"]'),
			).toBeVisible();
		} finally {
			await this.hardware
				.send(`/light/${alias}/programmer/shift`, [false])
				.catch(() => undefined);
			await this.closeDialog(dialog);
		}
	}

	private async expectColorAssignments(key: "prior" | "range"): Promise<void> {
		await expectProgrammerAssignments(
			this.api,
			this.requiredColorContract()[key],
		);
	}

	private async expectColorSelection(): Promise<void> {
		await expect
			.poll(async () => (await currentProgrammer(this.api)).selected)
			.toEqual(this.requiredColorContract().selected);
	}

	private async setColorAssignments(
		assignments: readonly ColorAssignment[],
	): Promise<void> {
		await batchProgrammerValues(this.api, {
			surface: "api",
			showId: this.requiredShowId(),
			mutations: assignments.map(({ fixtureId, attribute, value }) => ({
				action: "set_fixture",
				fixtureId,
				attribute,
				value: { kind: "normalized", value },
				timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
			})),
		});
	}

	private requiredShowId(): string {
		if (!this.showId)
			throw new Error(
				"A show identity is required for semantic programmer contracts",
			);
		return this.showId();
	}

	private requiredColorContract() {
		if (!this.colorContract)
			throw new Error("Call special.color.prepareRangeContract() first");
		return this.colorContract;
	}

	private async programmerValuesRevision(): Promise<number> {
		const snapshot = await this.api.request<any>(
			"GET",
			`/api/v2/users/${this.session().user.id}/programmer-values/snapshot`,
		);
		return snapshot.projection.revision;
	}

	private session() {
		if (!this.api.session)
			throw new Error("Programmer Special helper requires an API session");
		return this.api.session;
	}

	private async align(mode: PositionAlignMode): Promise<void> {
		await this.chooseFamily("Position");
		const order: PositionAlignMode[] = ["left", "right", "out", "in"];
		for (let index = 0; index <= order.indexOf(mode); index += 1) {
			const current = index === 0 ? "Off" : title(order[index - 1]);
			await this.desk.click(
				this.page.getByRole("button", {
					name: `Align ${current}`,
					exact: true,
				}),
			);
		}
		await expect(
			this.page.getByRole("button", {
				name: `Align ${title(mode)}`,
				exact: true,
			}),
		).toBeVisible();
	}

	private async alignViaApi(mode: PositionAlignMode): Promise<void> {
		await this.desk.recordStep(
			"POSITION ALIGN",
			`Align selected Pan values ${mode} through the production command boundary.`,
		);
		await this.api.alignProgrammerSelection(mode);
	}

	private async controlAction(semantic: ControlSemantic): Promise<void> {
		const dialog = await this.openDialog("Control");
		await this.desk.click(
			dialog.getByRole("button", {
				name: CONTROL_LABELS[semantic],
				exact: true,
			}),
		);
		await this.closeDialog(dialog);
	}

	private async controlActionViaApi(semantic: ControlSemantic): Promise<void> {
		const selected = new Set((await this.selection.observe()).selected);
		const actions = compatibleControlActions(
			await this.fixtures(),
			selected,
			semantic,
		);
		if (actions.length === 0)
			throw new Error(
				`No selected fixture supplies the ${semantic} control action`,
			);
		for (const { fixtureId, actionId } of actions)
			await this.controlCommand(fixtureId, actionId);
	}

	private async controlCommand(
		fixtureId: string,
		actionId: string,
	): Promise<void> {
		for (let attempt = 0; attempt < 20; attempt += 1) {
			try {
				await this.api.controlFixtureAction(fixtureId, actionId, true);
				return;
			} catch (error) {
				if (
					!(error instanceof Error) ||
					!error.message.includes("active show is changing") ||
					attempt === 19
				)
					throw error;
				await this.page.waitForTimeout(25);
			}
		}
	}

	private async openDialog(
		family: BeamSpecialFamily | "Position" | "Color" | "Control",
	): Promise<Locator> {
		await this.chooseFamily(family);
		await this.desk.click(
			this.page.getByRole("button", {
				name: "Special Dialog",
				exact: true,
			}),
		);
		const dialog = this.page
			.getByRole("heading", {
				name: `${family} · Special Dialog`,
				exact: true,
			})
			.locator("..");
		await expect(dialog).toBeVisible();
		return dialog;
	}

	private async chooseFamily(family: string): Promise<void> {
		await this.desk.click(
			this.page.getByRole("button", { name: family, exact: true }),
		);
	}

	private async fixtures(): Promise<PatchedFixture[]> {
		const patch = await this.api.patch();
		return (Array.isArray(patch) ? patch : patch.fixtures) as PatchedFixture[];
	}

	private async closeDialog(dialog: Locator): Promise<void> {
		await this.desk.click(dialog.getByRole("button", { name: "×" }));
		await expect(dialog).toBeHidden();
	}
}

class BrowserBeamSpecial {
	constructor(
		private readonly owner: BrowserProgrammerSpecials,
		private readonly family: BeamSpecialFamily,
	) {}

	available(): Promise<string[]> {
		return this.owner.available(this.family);
	}

	set(attribute: string, percentage: number): Promise<void> {
		return this.owner.setBeamValue(this.family, attribute, percentage);
	}
}

function fixtureIsSelected(
	fixture: PatchedFixture,
	selected: ReadonlySet<string>,
): boolean {
	return (
		selected.has(fixture.fixture_id) ||
		fixture.logical_heads.some((head) => selected.has(head.fixture_id))
	);
}

function belongsToSpecialFamily(
	attribute: string,
	family: BeamSpecialFamily,
): boolean {
	return family === "Shapers"
		? attribute.startsWith("shaper.")
		: /^(gobo|prism|iris)/.test(attribute);
}

function compatibleControlActions(
	fixtures: readonly PatchedFixture[],
	selected: ReadonlySet<string>,
	semantic: ControlSemantic,
): Array<{ fixtureId: string; actionId: string }> {
	return fixtures.flatMap((fixture) => {
		if (!fixtureIsSelected(fixture, selected)) return [];
		const profile = fixture.definition.profile_snapshot;
		const mode = profile?.modes.find(
			(candidate) => candidate.id === fixture.definition.mode_id,
		);
		return (
			mode?.control_actions
				.filter((action) => action.semantic === semantic)
				.map((action) => ({
					fixtureId: fixture.fixture_id,
					actionId: action.id,
				})) ?? []
		);
	});
}

async function pointerSet(
	page: Page,
	slider: Locator,
	percentage: number,
): Promise<void> {
	const box = await slider.boundingBox();
	if (!box) throw new Error("Special-dialog fader has no pointer box");
	const x = box.x + box.width / 2;
	const endpointZone = Math.min(
		box.height / 3,
		Math.max(18, Math.min(36, box.height * 0.1)),
	);
	const y =
		box.y +
		endpointZone +
		(1 - percentage / 100) * Math.max(1, box.height - endpointZone * 2);
	await page.mouse.move(x, box.y + box.height - endpointZone);
	await page.mouse.down();
	await page.mouse.move(x, y, { steps: 8 });
	await page.mouse.up();
}

function title(value: string): string {
	return value[0].toUpperCase() + value.slice(1);
}

const COLOR_ATTRIBUTES = [
	"color.red",
	"color.green",
	"color.blue",
	"color.cyan",
	"color.magenta",
	"color.yellow",
] as const;
const COLOR_RANGE_START: PickerColor = {
	hue: 0.1,
	saturation: 0.8,
	brightness: 0.85,
};
const COLOR_RANGE_END: PickerColor = {
	hue: 0.9,
	saturation: 0.2,
	brightness: 0.85,
};

function interpolatePickerRange(
	count: number,
	start: PickerColor,
	end: PickerColor,
): PickerColor[] {
	if (count <= 0) return [];
	if (count === 1) return [end];
	return Array.from({ length: count }, (_, index) => {
		if (index === 0) return { ...start, brightness: end.brightness };
		if (index === count - 1) return end;
		const ratio = index / (count - 1);
		return {
			hue: start.hue + (end.hue - start.hue) * ratio,
			saturation:
				start.saturation + (end.saturation - start.saturation) * ratio,
			brightness: end.brightness,
		};
	});
}

function colorProgrammerAssignments(
	selectedFixtures: readonly string[],
	patch: readonly PatchedFixture[],
	colors: readonly PickerColor[],
): ColorAssignment[] {
	return selectedFixtures.flatMap((fixtureId, index) => {
		const fixture = patch.find(
			(candidate) =>
				candidate.fixture_id === fixtureId ||
				candidate.logical_heads.some((head) => head.fixture_id === fixtureId),
		);
		if (!fixture) return [];
		const logicalHead = fixture.logical_heads.find(
			(head) => head.fixture_id === fixtureId,
		);
		const heads = logicalHead
			? fixture.definition.heads.filter(
					(head) => head.index === logicalHead.head_index,
				)
			: fixture.definition.heads.filter((head) => head.shared);
		const attributes = new Set(
			heads.flatMap((head) =>
				head.parameters.map((parameter) => parameter.attribute),
			),
		);
		const color = colors[index];
		if (!color) return [];
		const [red, green, blue] = hsvToRgb(color);
		const values: Array<[string, number]> = [
			["color.red", red],
			["color.green", green],
			["color.blue", blue],
			["color.cyan", 1 - red],
			["color.magenta", 1 - green],
			["color.yellow", 1 - blue],
		];
		return values.flatMap(([attribute, value]) =>
			attributes.has(attribute) ? [{ fixtureId, attribute, value }] : [],
		);
	});
}

function hsvToRgb({ hue, saturation, brightness }: PickerColor): number[] {
	const i = Math.floor(hue * 6);
	const f = hue * 6 - i;
	const p = brightness * (1 - saturation);
	const q = brightness * (1 - f * saturation);
	const t = brightness * (1 - (1 - f) * saturation);
	return [
		[brightness, t, p],
		[q, brightness, p],
		[p, brightness, t],
		[p, q, brightness],
		[t, p, brightness],
		[brightness, p, q],
	][i % 6];
}

async function clickPicker(page: Page, x: number, y: number): Promise<void> {
	const box = await page.locator(".color-sheet").boundingBox();
	if (!box) throw new Error("Color sheet has no pointer box");
	await page.mouse.click(box.x + box.width * x, box.y + box.height * y);
}

async function beginPickerDrag(
	page: Page,
	start: PickerColor,
	end: PickerColor,
): Promise<void> {
	const box = await page.locator(".color-sheet").boundingBox();
	if (!box) throw new Error("Color sheet has no pointer box");
	await page.mouse.move(
		box.x + box.width * start.hue,
		box.y + box.height * (1 - start.saturation),
	);
	await page.mouse.down();
	await page.mouse.move(
		box.x + box.width * end.hue,
		box.y + box.height * (1 - end.saturation),
		{ steps: 5 },
	);
}

async function currentProgrammer(api: ApiDriver): Promise<{
	selected: string[];
	values: Array<{
		fixture_id: string;
		attribute: string;
		value: { value?: number } | number;
	}>;
}> {
	const programmers = await api.request<
		Array<{
			session_id?: string;
			selected: string[];
			values: Array<{
				fixture_id: string;
				attribute: string;
				value: { value?: number } | number;
			}>;
		}>
	>("GET", "/api/v2/programmers");
	const current =
		programmers.find(
			(programmer) => programmer.session_id === api.session?.session_id,
		) ?? programmers[0];
	if (!current) throw new Error("No programmer is available");
	return current;
}

async function expectProgrammerAssignments(
	api: ApiDriver,
	expected: readonly ColorAssignment[],
): Promise<void> {
	await expect
		.poll(async () => {
			const values = (await currentProgrammer(api)).values;
			return expected.every((assignment) => {
				const actual = values.find(
					(value) =>
						value.fixture_id === assignment.fixtureId &&
						value.attribute === assignment.attribute,
				);
				const value =
					typeof actual?.value === "number"
						? actual.value
						: actual?.value?.value;
				return (
					typeof value === "number" &&
					Math.abs(value - assignment.value) < 0.00001
				);
			});
		})
		.toBe(true);
}
