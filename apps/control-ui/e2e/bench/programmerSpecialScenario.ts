import { expect, type Locator, type Page } from "@playwright/test";
import type { PatchedFixture } from "../../src/api/types";
import type { ApiDriver } from "./api";
import type { DeskDriver } from "./desk";
import type { BrowserSelection } from "./selectionScenario";

export type PositionAlignMode = "out" | "center" | "left" | "right";
export type ControlSemantic =
	| "lamp_on"
	| "lamp_off"
	| "reset"
	| "fan_auto"
	| "fan_low"
	| "fan_high"
	| "fan_max";
export type BeamSpecialFamily = "Beam" | "Shapers";

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
	readonly position = {
		returnHome: () => this.returnHome(),
		align: (mode: PositionAlignMode) => this.align(mode),
		alignViaApi: (mode: PositionAlignMode) => this.alignViaApi(mode),
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
	) {}

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
		const dialog = await this.openDialog("Position");
		const action = dialog.getByRole("button", {
			name: "Return Home",
			exact: true,
		});
		await expect(action).toBeEnabled();
		await this.desk.click(action);
		await this.closeDialog(dialog);
	}

	private async align(mode: PositionAlignMode): Promise<void> {
		await this.chooseFamily("Position");
		const order: PositionAlignMode[] = ["out", "center", "left", "right"];
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
		await this.api.alignProgrammerSelection("pan", mode);
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

	private async controlActionViaApi(
		semantic: ControlSemantic,
	): Promise<void> {
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
		family: BeamSpecialFamily | "Position" | "Control",
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
	const y = box.y + 8 + (1 - percentage / 100) * Math.max(1, box.height - 16);
	await page.mouse.move(x, box.y + box.height - 8);
	await page.mouse.down();
	await page.mouse.move(x, y, { steps: 8 });
	await page.mouse.up();
}

function title(value: string): string {
	return value[0].toUpperCase() + value.slice(1);
}
