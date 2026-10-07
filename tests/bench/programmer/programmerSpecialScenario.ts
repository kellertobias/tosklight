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
/** Beam carries no Special Dialog; Shapers is the family that uses this shape of one. */
export type BeamSpecialFamily = "Shapers";

/** Programmer Angles of one head, in degrees (the semantic Position owner, TL-552). */
type PositionAngles = {
	fixtureId: string;
	pan: number;
	tilt: number;
};
/** The requested White Blend (0–1) of one Color head's semantic colour. */
type WhiteBlendAssignment = {
	fixtureId: string;
	whiteBlend: number;
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
		home: PositionAngles[];
		before: PositionAngles[];
	};
	private colorContract?: {
		selected: string[];
		range: WhiteBlendAssignment[];
		prior: WhiteBlendAssignment[];
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

	/**
	 * Two Position heads of the canonical show, in reverse patch order, with a fixture without
	 * Position between them. Each starts from its own programmed Angles; home is the semantic
	 * home pose, Pan 0° and Tilt 0° (the centre of travel), for every head.
	 */
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
				const attributes = new Set(
					head.parameters.map((parameter) => parameter.attribute),
				);
				return fixtureId && attributes.has("pan") && attributes.has("tilt")
					? [fixtureId]
					: [];
			});
		});
		expect(targets.length).toBeGreaterThanOrEqual(2);
		const chosen = [targets[1], targets[0]];
		const home = chosen.map((fixtureId) => ({ fixtureId, pan: 0, tilt: 0 }));
		const before = [
			{ fixtureId: chosen[0], pan: 120, tilt: 30 },
			{ fixtureId: chosen[1], pan: -60, tilt: -20 },
		];
		const nonPosition = patch.find((fixture) =>
			fixture.definition.heads.every((head) =>
				head.parameters.every(
					(parameter) => !["pan", "tilt"].includes(parameter.attribute),
				),
			),
		);
		const selected = [
			chosen[0],
			...(nonPosition ? [nonPosition.fixture_id] : []),
			chosen[1],
		];
		await replaceProgrammingSelection(this.api, {
			surface: "api",
			showId: this.requiredShowId(),
			fixtures: selected,
		});
		await this.setPositionAngles(before);
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
		const blade = /^shaper\.blade\.(\d)\./.exec(attribute);
		if (blade) {
			// Shapers is a drawing of the aperture: a blade is inserted and rotated by dragging its
			// own handle rather than by a slider named after the attribute.
			await expect(
				dialog.getByRole("slider", {
					name: `Blade ${blade[1]} insertion and rotation`,
					exact: true,
				}),
			).toBeVisible();
			await this.closeDialog(dialog);
			return;
		}
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
		const dialog = await this.openPositionDialog();
		const action = dialog.getByRole("button", {
			name: "Return Home",
			exact: true,
		});
		await expect(action).toBeEnabled();
		await this.desk.click(action);
		await this.closePositionDialog(dialog);
	}

	private async expectReturnHomeUnavailable(): Promise<void> {
		const dialog = await this.openPositionDialog();
		await expect(
			dialog.getByRole("button", { name: "Return Home", exact: true }),
		).toBeDisabled();
		await this.closePositionDialog(dialog);
	}

	/**
	 * The semantic Position Special Dialog (TL-549): a standard modal titled **Position**. The
	 * family button names its page on a multi-page layout (`Position 1 of 2`).
	 */
	private async openPositionDialog(): Promise<Locator> {
		await this.desk.click(
			this.page.getByRole("button", { name: /^Position( \d+ of \d+)?$/ }),
		);
		await this.desk.click(
			this.page.getByRole("button", { name: "Special Dialog", exact: true }),
		);
		const dialog = this.page.getByRole("dialog", {
			name: "Position Special Dialog",
			exact: true,
		});
		await expect(dialog).toBeVisible();
		return dialog;
	}

	private async closePositionDialog(dialog: Locator): Promise<void> {
		await this.desk.click(
			dialog.getByRole("button", {
				name: "Close Position Special Dialog",
				exact: true,
			}),
		);
		await expect(dialog).toBeHidden();
	}

	private async expectPositionAssignments(
		key: "home" | "before",
	): Promise<void> {
		const expected = this.requiredPositionContract()[key];
		await expect
			.poll(async () => {
				const values = (await currentProgrammer(this.api)).values;
				return expected.map(({ fixtureId }) => {
					const entry = values.find(
						(value) =>
							value.fixture_id === fixtureId && value.attribute === "position",
					);
					return programmedAngles(entry?.value);
				});
			})
			.toEqual(expected.map(({ pan, tilt }) => ({ pan, tilt })));
	}

	private async setPositionAngles(
		angles: readonly PositionAngles[],
	): Promise<void> {
		await batchProgrammerValues(this.api, {
			surface: "api",
			showId: this.requiredShowId(),
			mutations: angles.map(({ fixtureId, pan, tilt }) => ({
				action: "set_fixture",
				fixtureId,
				attribute: "position",
				value: {
					kind: "position",
					value: {
						kind: "angles",
						pan_degrees: { kind: "value", value: pan },
						tilt_degrees: { kind: "value", value: tilt },
					},
				} as never,
				timing: { fade: false, fadeMillis: null, delayMillis: null },
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

	/**
	 * Three Color heads of the canonical show (a logical head first when one exists) with a
	 * fixture without Color between the second and third. Each starts from the same semantic
	 * white at White Blend 33%; the range is White Blend 80% → 20% over the Color heads in
	 * selection order (docs/testing/36 SEMANTIC-COLOR-003).
	 */
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
		].map((target) => target.fixtureId);
		const nonColor = patch.find((fixture) =>
			fixture.definition.heads.every((head) =>
				head.parameters.every(
					(parameter) => !parameter.attribute.startsWith("color."),
				),
			),
		);
		const selected = [
			chosen[0],
			chosen[1],
			...(nonColor ? [nonColor.fixture_id] : []),
			chosen[2],
		];
		const range = whiteBlendRange(chosen, COLOR_RANGE_FIRST, COLOR_RANGE_LAST);
		const prior = chosen.map((fixtureId) => ({ fixtureId, whiteBlend: 0.33 }));
		await replaceProgrammingSelection(this.api, {
			surface: "api",
			showId: this.requiredShowId(),
			fixtures: selected,
		});
		await this.setWhiteBlend(prior);
		this.colorContract = { selected, range, prior };
	}

	/** One ordinary touch on White Blend sets every Color head to the same value. */
	private async setUniformColor(): Promise<void> {
		const contract = this.requiredColorContract();
		const dialog = await this.openColorDialog();
		await touchFader(this.page, whiteBlendFader(dialog), COLOR_UNIFORM);
		await expectWhiteBlend(
			this.api,
			contract.range.map(({ fixtureId }) => ({
				fixtureId,
				whiteBlend: COLOR_UNIFORM,
			})),
		);
		await this.closeColorDialog(dialog);
	}

	/**
	 * Keyboard Shift held for one drag from 80% to 20%: the press only marks endpoint 1 and
	 * writes nothing; reaching the last value completes the ordered range.
	 */
	private async applyColorRangeWithShift(): Promise<void> {
		const dialog = await this.openColorDialog();
		await this.page.keyboard.down("Shift");
		try {
			await this.touchRange(dialog, "drag");
		} finally {
			await this.page.keyboard.up("Shift");
		}
		await this.closeColorDialog(dialog);
	}

	/** A shifted first touch that is cancelled by the system leaves the Programmer untouched. */
	private async cancelColorRangeWithShift(): Promise<void> {
		const contract = this.requiredColorContract();
		const dialog = await this.openColorDialog();
		const fader = whiteBlendFader(dialog);
		await this.page.keyboard.down("Shift");
		try {
			const point = await faderPoint(fader, COLOR_RANGE_FIRST);
			await this.page.mouse.move(point.x, point.y);
			await this.page.mouse.down();
			await expect(pendingEndpoint(dialog)).toBeVisible();
			await fader.dispatchEvent("pointercancel", {
				pointerId: 1,
				pointerType: "mouse",
			});
			await this.page.mouse.up();
		} finally {
			await this.page.keyboard.up("Shift");
		}
		await expectWhiteBlend(this.api, contract.prior);
		await this.closeColorDialog(dialog);
	}

	/** The attached hardware Shift arms the same ordered range as the keyboard. */
	private async applyColorRangeWithHardwareShift(): Promise<void> {
		if (!this.hardware?.connected)
			throw new Error("Hardware Color range requires hardware.connect() first");
		const alias = "desk";
		const dialog = await this.openColorDialog();
		try {
			await this.hardware.send(`/light/${alias}/programmer/shift`, [true]);
			await this.touchRange(dialog, "touches");
			await this.hardware.send(`/light/${alias}/programmer/shift`, [false]);
		} finally {
			await this.hardware
				.send(`/light/${alias}/programmer/shift`, [false])
				.catch(() => undefined);
			await this.closeColorDialog(dialog);
		}
	}

	/**
	 * A shifted range on White Blend from 80% to 20%, as one drag or as two touches. Shift must
	 * already be held or armed.
	 */
	private async touchRange(dialog: Locator, mode: "drag" | "touches"): Promise<void> {
		const contract = this.requiredColorContract();
		const fader = whiteBlendFader(dialog);
		const first = await faderPoint(fader, COLOR_RANGE_FIRST);
		const last = await faderPoint(fader, COLOR_RANGE_LAST);
		await this.page.mouse.move(first.x, first.y);
		await this.page.mouse.down();
		if (mode === "touches") await this.page.mouse.up();
		await expect(pendingEndpoint(dialog)).toBeVisible();
		await expect(dialog.getByText("Shift-click the last value")).toBeVisible();
		// The first endpoint alone is never written.
		await expectWhiteBlend(this.api, contract.prior);
		if (mode === "drag") {
			await this.page.mouse.move(last.x, last.y, { steps: 6 });
			await this.page.mouse.up();
		} else await this.page.mouse.click(last.x, last.y);
		await expect(
			dialog.locator('output[aria-label="White Blend value"]'),
		).toHaveText(
			`${Math.round(COLOR_RANGE_FIRST * 100)}% → ${Math.round(COLOR_RANGE_LAST * 100)}%`,
		);
		for (const endpoint of ["1", "2"])
			await expect(
				whiteBlendField(dialog).locator(
					`.horizontal-range-handle[data-endpoint="${endpoint}"]`,
				),
			).toHaveCount(1);
		await expectWhiteBlend(this.api, contract.range);
	}

	/**
	 * The semantic Color Special Dialog (TL-550): compact in the encoder area when it fits, the
	 * full modal otherwise. White Blend is on the first page of both.
	 */
	private async openColorDialog(): Promise<Locator> {
		await this.desk.click(
			this.page.getByRole("button", { name: /^Color( \d+ of \d+)?$/ }).first(),
		);
		await this.desk.click(
			this.page.getByRole("button", { name: "Special Dialog", exact: true }),
		);
		const dialog = this.page.getByRole("dialog", {
			name: "Color Special Dialog",
			exact: true,
		});
		await expect(dialog).toBeVisible();
		await expect(whiteBlendFader(dialog)).toBeVisible();
		return dialog;
	}

	/** The modal closes with its close button; the compact page by tapping the Color tab. */
	private async closeColorDialog(dialog: Locator): Promise<void> {
		const close = dialog.getByRole("button", {
			name: "Close Special Dialog",
			exact: true,
		});
		if (await close.count()) await this.desk.click(close);
		else
			await this.desk.click(
				this.page.getByRole("button", { name: /^Color( \d+ of \d+)?$/ }).first(),
			);
		await expect(dialog).toBeHidden();
	}

	private async expectColorAssignments(key: "prior" | "range"): Promise<void> {
		await expectWhiteBlend(this.api, this.requiredColorContract()[key]);
	}

	private async expectColorSelection(): Promise<void> {
		await expect
			.poll(async () => (await currentProgrammer(this.api)).selected)
			.toEqual(this.requiredColorContract().selected);
	}

	/** A complete semantic white at the given White Blend, one programmer value per head. */
	private async setWhiteBlend(values: readonly WhiteBlendAssignment[]): Promise<void> {
		await batchProgrammerValues(this.api, {
			surface: "api",
			showId: this.requiredShowId(),
			mutations: values.map(({ fixtureId, whiteBlend }) => ({
				action: "set_fixture",
				fixtureId,
				attribute: "color",
				value: semanticWhite(whiteBlend) as never,
				timing: { fade: false, fadeMillis: null, delayMillis: null },
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
		// The card, not the heading's wrapper: the title bar nests its heading beside the
		// actions, so the element above the heading holds no dialog body at all.
		const dialog = this.page.locator(".modal-card").filter({
			has: this.page.getByRole("heading", {
				name: `${family} · Special Dialog`,
				exact: true,
			}),
		});
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
		await this.desk.click(
			dialog.getByRole("button", { name: "Close modal", exact: true }),
		);
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
const COLOR_UNIFORM = 0.6;
const COLOR_RANGE_FIRST = 0.8;
const COLOR_RANGE_LAST = 0.2;

/** Equal steps from the first to the last value over the heads, in selection order. */
function whiteBlendRange(
	fixtureIds: readonly string[],
	first: number,
	last: number,
): WhiteBlendAssignment[] {
	const steps = Math.max(1, fixtureIds.length - 1);
	return fixtureIds.map((fixtureId, index) => ({
		fixtureId,
		whiteBlend: first + ((last - first) * index) / steps,
	}));
}

/** The semantic default white (docs/help 05-color-intent) at the given White Blend. */
function semanticWhite(whiteBlend: number) {
	return {
		kind: "color_program",
		value: {
			kind: "semantic",
			intent: {
				base_xyz: { x: 0.95047, y: 1, z: 1.08883 },
				recipe: { version: 1, rgb: [1, 1, 1], amber: 0, approximate: false },
				white_blend: whiteBlend,
				white_target: { kelvin: 6500, duv: 0 },
				uv: { amount: 0 },
				relative_output: 1,
				allocation: "preserve_recipe",
			},
		},
	};
}

function whiteBlendField(dialog: Locator): Locator {
	return dialog.locator('.horizontal-range-field[data-control="white_blend"]');
}

function whiteBlendFader(dialog: Locator): Locator {
	return dialog.getByRole("slider", { name: "White Blend", exact: true });
}

/** The lone endpoint-1 marker a shifted first touch leaves. */
function pendingEndpoint(dialog: Locator): Locator {
	return whiteBlendField(dialog).locator(
		'.horizontal-range-handle[data-endpoint="1"]',
	);
}

/** The point on the fader's travel for `value` (0–1), below its label and readout. */
async function faderPoint(fader: Locator, value: number) {
	const box = await fader.boundingBox();
	if (!box) throw new Error("White Blend fader has no pointer box");
	return { x: box.x + box.width * value, y: box.y + box.height * 0.7 };
}

/** One touch (press and release) at `value` on a horizontal range fader. */
async function touchFader(page: Page, fader: Locator, value: number) {
	const point = await faderPoint(fader, value);
	await page.mouse.click(point.x, point.y);
}

/** The requested White Blend of one programmed semantic Color value, or null. */
function programmedWhiteBlend(value: unknown): number | null {
	const color = value as
		| { kind?: string; value?: { kind?: string; intent?: { white_blend?: unknown } } }
		| undefined;
	if (color?.kind !== "color_program" || color.value?.kind !== "semantic") return null;
	const whiteBlend = color.value.intent?.white_blend;
	return typeof whiteBlend === "number" ? whiteBlend : null;
}

async function expectWhiteBlend(
	api: ApiDriver,
	expected: readonly WhiteBlendAssignment[],
): Promise<void> {
	await expect
		.poll(async () => {
			const values = (await currentProgrammer(api)).values;
			return expected.every(({ fixtureId, whiteBlend }) => {
				const entry = values.find(
					(value) => value.fixture_id === fixtureId && value.attribute === "color",
				);
				const actual = programmedWhiteBlend(entry?.value);
				// One fader step is 1%; the requested value is exact to well below that.
				return actual !== null && Math.abs(actual - whiteBlend) < 0.0005;
			});
		})
		.toBe(true);
}

/** The requested Angles of one programmed semantic Position value, or null. */
function programmedAngles(value: unknown): { pan: number; tilt: number } | null {
	const position = value as
		| {
				kind?: string;
				value?: {
					kind?: string;
					pan_degrees?: { kind?: string; value?: unknown };
					tilt_degrees?: { kind?: string; value?: unknown };
				};
		  }
		| undefined;
	const angles = position?.kind === "position" ? position.value : undefined;
	if (angles?.kind !== "angles") return null;
	const pan = angles.pan_degrees?.value;
	const tilt = angles.tilt_degrees?.value;
	return typeof pan === "number" && typeof tilt === "number" ? { pan, tilt } : null;
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
