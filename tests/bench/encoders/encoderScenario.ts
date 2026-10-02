import { expect, type Locator, type Page } from "@playwright/test";
import type { AttributeValue } from "../../../apps/light-desktop/src/api/types/playback";
import type { BrowserSelection } from "../command-selection/selectionScenario";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type { SimulatedHardware } from "../hardware/hardwareScenario";
import {
	applyProgrammerSelectionValue,
	batchProgrammerValues,
	clearProgrammerValues,
} from "../programmer/programmerValues";
import { BrowserDiscreteEncoders } from "./discreteEncoderScenario";
import {
	BeamAttribute,
	ColorAttribute,
	degreesEncoderValue,
	EncoderGroup,
	type EncoderCatalogEntry,
	encoderCatalogEntry,
	FocusAttribute,
	IntensityAttribute,
	normalizedEncoderValue,
	PositionAttribute,
	type ProgrammerExpression,
	ProgrammerToken,
	ShapersAttribute,
} from "./encoderCatalog";
import { BrowserOscEncoderRoute } from "./encoderOscScenario";

type EncoderRoute = "api" | "ui" | "osc";
type EncoderOperation = "set" | "add" | "subtract";

export interface EncoderRouteReport {
	seed: string;
	actionIndex: number;
	operation: EncoderOperation;
	group: EncoderGroup;
	attribute: string;
	candidates: readonly EncoderRoute[];
	selected: EncoderRoute;
}

export interface NormalizedEncoderPort {
	set(value: number | ProgrammerExpression): Promise<void>;
	add(steps: number): Promise<void>;
	subtract(steps: number): Promise<void>;
	release(): Promise<void>;
}

export interface AbsoluteEncoderPort {
	set(value: number | ProgrammerExpression): Promise<void>;
}

export interface RelativeEncoderPort {
	add(steps: number): Promise<void>;
	subtract(steps: number): Promise<void>;
}

class ApiNormalizedEncoderPort implements NormalizedEncoderPort {
	constructor(private readonly encoder: NormalizedEncoder) {}

	set(value: number | ProgrammerExpression): Promise<void> {
		return this.encoder.execute("set", value, "api");
	}

	add(steps: number): Promise<void> {
		return this.encoder.execute("add", steps, "api");
	}

	subtract(steps: number): Promise<void> {
		return this.encoder.execute("subtract", steps, "api");
	}

	release(): Promise<void> {
		return this.encoder.release();
	}
}

class VisibleEncoderPort implements NormalizedEncoderPort {
	constructor(private readonly encoder: NormalizedEncoder) {}

	set(value: number | ProgrammerExpression): Promise<void> {
		return this.encoder.execute("set", value, "ui");
	}

	add(steps: number): Promise<void> {
		return this.encoder.execute("add", steps, "ui");
	}

	subtract(steps: number): Promise<void> {
		return this.encoder.execute("subtract", steps, "ui");
	}

	release(): Promise<void> {
		return this.encoder.release();
	}

	drag(direction: "add" | "subtract", rate: "slow" | "fast"): Promise<void> {
		return this.encoder.drag(direction, rate);
	}
}

class OscRelativeEncoderPort implements RelativeEncoderPort {
	constructor(private readonly encoder: NormalizedEncoder) {}

	add(steps: number): Promise<void> {
		return this.encoder.execute("add", steps, "osc");
	}

	subtract(steps: number): Promise<void> {
		return this.encoder.execute("subtract", steps, "osc");
	}
}

export class NormalizedEncoder implements NormalizedEncoderPort {
	readonly via = {
		api: new ApiNormalizedEncoderPort(this),
		ui: new VisibleEncoderPort(this),
		osc: new OscRelativeEncoderPort(this),
	};

	constructor(
		private readonly owner: BrowserEncoders,
		readonly group: EncoderGroup,
		readonly key: string,
	) {}

	drag(direction: "add" | "subtract", rate: "slow" | "fast"): Promise<void> {
		return this.owner.drag(this, direction, rate);
	}

	set(value: number | ProgrammerExpression): Promise<void> {
		return this.owner.unqualified(this, "set", value);
	}

	add(steps: number): Promise<void> {
		return this.owner.unqualified(this, "add", steps);
	}

	subtract(steps: number): Promise<void> {
		return this.owner.unqualified(this, "subtract", steps);
	}

	release(): Promise<void> {
		return this.owner.releaseAttribute(this);
	}

	execute(
		operation: EncoderOperation,
		value: number | ProgrammerExpression,
		route: EncoderRoute,
	): Promise<void> {
		return this.owner.execute(this, operation, value, route);
	}
}

type EncoderGroupTree<T extends string> = Record<T, NormalizedEncoder>;

export class BrowserEncoders {
	readonly intensity: EncoderGroupTree<IntensityAttribute>;
	readonly color: EncoderGroupTree<ColorAttribute>;
	readonly position: EncoderGroupTree<PositionAttribute>;
	readonly beam: EncoderGroupTree<BeamAttribute>;
	readonly shapers: EncoderGroupTree<ShapersAttribute>;
	readonly focus: EncoderGroupTree<FocusAttribute>;
	readonly discrete: BrowserDiscreteEncoders;
	readonly routeReports: EncoderRouteReport[] = [];
	private actionIndex = 0;
	private readonly osc: BrowserOscEncoderRoute;

	constructor(
		private readonly api: ApiDriver,
		private readonly selection: BrowserSelection,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly hardware: SimulatedHardware,
		private readonly seed: string,
	) {
		this.intensity = this.group(EncoderGroup.Intensity, IntensityAttribute);
		this.color = this.group(EncoderGroup.Color, ColorAttribute);
		this.position = this.group(EncoderGroup.Position, PositionAttribute);
		this.beam = this.group(EncoderGroup.Beam, BeamAttribute);
		this.shapers = this.group(EncoderGroup.Shapers, ShapersAttribute);
		this.focus = this.group(EncoderGroup.Focus, FocusAttribute);
		this.discrete = new BrowserDiscreteEncoders(api, selection, desk, page);
		this.osc = new BrowserOscEncoderRoute(api, page, desk, hardware);
	}

	async releaseAttribute(encoder: NormalizedEncoder): Promise<void> {
		const catalog = encoderCatalogEntry(encoder.group, encoder.key);
		const context = await this.programmerContext();
		await batchProgrammerValues(this.api, {
			surface: "api",
			showId: context.showId,
			mutations: context.fixtureIds.map((fixtureId) => ({
				action: "release_fixture",
				fixtureId,
				attribute: catalog.attribute,
			})),
		});
	}

	async clear(): Promise<void> {
		const context = await this.programmerContext();
		await clearProgrammerValues(this.api, {
			surface: "api",
			showId: context.showId,
		});
	}

	async drag(
		encoder: NormalizedEncoder,
		direction: "add" | "subtract",
		rate: "slow" | "fast",
	): Promise<void> {
		const catalog = encoderCatalogEntry(encoder.group, encoder.key);
		await this.desk.recordStep(
			"ENCODER DRAG",
			`${rate} continuous ${direction} on ${catalog.familyLabel} ${catalog.label}.`,
		);
		await this.activateFamily(catalog.familyLabel);
		const control = this.softwareControl(catalog.label);
		await expect(control).toBeVisible();
		const box = await control.boundingBox();
		if (!box) throw new Error("Visible touch encoder has no pointer box");
		const start = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
		const displacement = rate === "slow" ? 20 : 90;
		await this.page.mouse.move(start.x, start.y);
		await this.page.mouse.down();
		await this.page.mouse.move(
			start.x,
			start.y + (direction === "add" ? -displacement : displacement),
		);
		await this.page.waitForTimeout(120);
		await this.page.mouse.up();
	}

	async unqualified(
		encoder: NormalizedEncoder,
		operation: EncoderOperation,
		value: number | ProgrammerExpression,
	): Promise<void> {
		const candidates: EncoderRoute[] =
			operation === "set"
				? ["api", "ui"]
				: this.hardware.connected
					? ["api", "osc"]
					: ["api"];
		const actionIndex = this.actionIndex++;
		const selected =
			candidates[stableIndex(`${this.seed}:${actionIndex}`, candidates.length)];
		this.routeReports.push({
			seed: this.seed,
			actionIndex,
			operation,
			group: encoder.group,
			attribute: encoder.key,
			candidates,
			selected,
		});
		await this.execute(encoder, operation, value, selected);
	}

	async execute(
		encoder: NormalizedEncoder,
		operation: EncoderOperation,
		input: number | ProgrammerExpression,
		route: EncoderRoute,
	): Promise<void> {
		const catalog = encoderCatalogEntry(encoder.group, encoder.key);
		if (!catalog.normalized)
			throw new Error(`${catalog.label} requires a typed discrete value`);
		if (operation !== "set") assertPositiveSteps(input);
		const value =
			operation !== "set"
				? (input as number)
				: catalog.semantic?.unit === "degrees"
					? degreesAttributeValue(input)
					: normalizedEncoderValue(input);
		await this.desk.recordStep(
			"ENCODER",
			`${operation} ${catalog.familyLabel} ${catalog.label} through the ${route.toUpperCase()} route.`,
		);
		if (route === "ui") {
			if (operation === "set")
				await this.visibleSet(
					catalog.familyLabel,
					catalog.label,
					value as AttributeValue | DegreesValue,
				);
			else
				await this.visibleStep(
					catalog.familyLabel,
					catalog.label,
					operation,
					value as number,
				);
			return;
		}
		if (route === "osc") {
			if (operation === "set")
				throw new Error(
					"OSC encoder turns are relative; use the explicit API or visible value-entry route for absolute values",
				);
			await this.osc.detents(
				catalog.familyLabel,
				catalog.label,
				operation,
				value as number,
			);
			return;
		}
		if (catalog.semantic) {
			await this.semanticApiMutation(catalog, operation, input);
			return;
		}
		await this.apiMutation(
			catalog.attribute,
			operation,
			value as AttributeValue | number,
		);
	}

	/**
	 * Color and Position are semantic families at programming contract 1 (TL-552): an encoder edits
	 * one component of the selection's whole Color or Angles with `component_edits`, exactly as the
	 * software encoders do. Color components are recipe percentages; Pan and Tilt are degrees, one
	 * relative step is one percent or one degree.
	 */
	private async semanticApiMutation(
		catalog: EncoderCatalogEntry,
		operation: EncoderOperation,
		input: number | ProgrammerExpression,
	): Promise<void> {
		const semantic = catalog.semantic;
		if (!semantic) throw new Error(`${catalog.label} is not a semantic encoder`);
		const context = await this.programmerContext();
		const degrees = semantic.unit === "degrees";
		const scalar =
			operation === "set"
				? {
						kind: "set" as const,
						value: degrees
							? degreesEncoderValue(input)
							: scalarPercentages(normalizedEncoderValue(input)),
					}
				: {
						kind: "relative" as const,
						value:
							(operation === "add" ? 1 : -1) *
							(input as number) *
							(degrees ? 1 : 0.01),
					};
		const edit = { kind: "scalar", component: semantic.component, operation: scalar };
		if (semantic.owner === "position")
			await this.seedAngles(context);
		await applyProgrammerSelectionValue(this.api, {
			surface: "api",
			showId: context.showId,
			fixtureIds: context.fixtureIds,
			attribute: semantic.owner,
			operation: { type: "component_edits", edits: [edit] } as never,
			timing: { fade: false, fadeMillis: null, delayMillis: null },
		});
	}

	/**
	 * A Pan or Tilt edit changes one axis of the fixture's Angles. A lamp without a Position
	 * physical graph has no displayed pose to start from, so a selected fixture that holds no
	 * Programmer Angles yet starts at home (0°, 0°), the centre of travel a legacy untouched
	 * axis rested at. Fixtures already holding Angles keep their other axis.
	 */
	private async seedAngles(
		context: { showId: string; fixtureIds: readonly string[] },
	): Promise<void> {
		const snapshot = await this.api.request<{
			projection: {
				fixture_values?: Array<{
					fixture_id: string;
					attribute: string;
					value: { kind: string; value?: { kind?: string } };
				}>;
			};
		}>("GET", "/api/v2/programmer/values/snapshot");
		const holding = new Set(
			(snapshot.projection.fixture_values ?? [])
				.filter(
					(entry) =>
						entry.attribute === "position" &&
						entry.value.kind === "position" &&
						entry.value.value?.kind === "angles",
				)
				.map((entry) => entry.fixture_id),
		);
		const missing = context.fixtureIds.filter((id) => !holding.has(id));
		if (missing.length === 0) return;
		const home = { kind: "value", value: 0 };
		await batchProgrammerValues(this.api, {
			surface: "api",
			showId: context.showId,
			mutations: missing.map((fixtureId) => ({
				action: "set_fixture",
				fixtureId,
				attribute: "position",
				value: {
					kind: "position",
					value: { kind: "angles", pan_degrees: home, tilt_degrees: home },
				} as never,
				timing: { fade: false, fadeMillis: null, delayMillis: null },
			})),
		});
	}

	private async apiMutation(
		attribute: string,
		operation: EncoderOperation,
		value: AttributeValue | number,
	): Promise<void> {
		const [selection, bootstrap] = await Promise.all([
			this.selection.observe(),
			this.api.request<{ active_show: { id: string } | null }>(
				"GET",
				"/api/v2/bootstrap",
			),
		]);
		if (!bootstrap.active_show) throw new Error("No active Show");
		if (selection.selected.length === 0)
			throw new Error("Encoder action requires a non-empty Fixture selection");
		await applyProgrammerSelectionValue(this.api, {
			surface: "api",
			showId: bootstrap.active_show.id,
			fixtureIds: selection.selected,
			attribute,
			operation:
				operation === "set"
					? { type: "absolute_set", value: value as AttributeValue }
					: {
							type: "relative_step",
							delta: (operation === "add" ? 1 : -1) * (value as number) * 0.01,
						},
			timing: {
				fade: false,
				fadeMillis: null,
				delayMillis: null,
			},
		});
	}

	private async programmerContext(): Promise<{
		showId: string;
		fixtureIds: readonly string[];
	}> {
		const [selection, bootstrap] = await Promise.all([
			this.selection.observe(),
			this.api.request<{ active_show: { id: string } | null }>(
				"GET",
				"/api/v2/bootstrap",
			),
		]);
		if (!bootstrap.active_show) throw new Error("No active Show");
		if (selection.selected.length === 0)
			throw new Error("Encoder action requires a non-empty Fixture selection");
		return {
			showId: bootstrap.active_show.id,
			fixtureIds: selection.selected,
		};
	}

	private async visibleSet(
		family: string,
		label: string,
		value: AttributeValue | DegreesValue,
	): Promise<void> {
		await this.activateFamily(family);
		const control = this.softwareControl(label);
		await expect(
			control,
			`${family} ${label} should appear on the live software encoder page`,
		).toBeVisible();
		await control
			.getByRole("button", {
				name: new RegExp(`^Set Enc \\d+ · ${escapeRegex(label)}${READOUT_SUFFIX} value$`),
			})
			.click();
		const dialog = this.page.getByRole("dialog", {
			name: new RegExp(`^Enc \\d+ · ${escapeRegex(label)}${READOUT_SUFFIX} value$`),
		});
		await expect(dialog).toBeVisible();
		for (const token of valueTokens(value))
			await this.desk.click(
				dialog.getByRole("button", { name: token, exact: true }),
			);
		await expect(dialog).toBeHidden();
	}

	private async visibleStep(
		family: string,
		label: string,
		operation: "add" | "subtract",
		steps: number,
	): Promise<void> {
		await this.activateFamily(family);
		const control = this.softwareControl(label);
		await expect(control).toBeVisible();
		for (let remaining = steps; remaining > 0; remaining -= 1) {
			await control.dispatchEvent("wheel", {
				deltaY: operation === "add" ? -1 : 1,
				shiftKey: true,
			});
			await this.page.waitForTimeout(20);
		}
	}

	private async activateFamily(family: string): Promise<void> {
		const familyButton = this.page.getByRole("button", {
			name: family,
			exact: true,
		});
		if (!(await familyButton.isVisible())) {
			const fixtures = this.page
				.locator("[aria-label='Built-ins']")
				.getByRole("button", { name: "Fixtures", exact: true });
			if (!(await fixtures.isVisible()))
				await this.page
					.getByRole("button", {
						name: "Desktops / Built-ins",
						exact: true,
					})
					.click();
			await expect(fixtures).toBeVisible();
			await fixtures.click();
			await expect(this.page.locator(".fixture-window")).toBeVisible();
		}
		await expect(familyButton).toBeVisible();
		await this.desk.click(familyButton);
	}

	private softwareControl(label: string): Locator {
		return this.page.getByRole("group", {
			name: new RegExp(`^Enc \\d+ · ${escapeRegex(label)}${READOUT_SUFFIX}$`),
		});
	}

	private group<T extends string>(
		group: EncoderGroup,
		values: Record<string, T>,
	): EncoderGroupTree<T> {
		return Object.fromEntries(
			Object.values(values).map((key) => [
				key,
				new NormalizedEncoder(this, group, key),
			]),
		) as EncoderGroupTree<T>;
	}
}

function valueTokens(value: AttributeValue | DegreesValue): string[] {
	const degrees = value.kind === "degrees" || value.kind === "degrees_spread";
	const points =
		value.kind === "normalized" || value.kind === "degrees"
			? [value.value]
			: value.kind === "spread" || value.kind === "degrees_spread"
				? value.value
				: [];
	if (points.length === 0)
		throw new Error("Visible normalized encoder entry requires numeric points");
	return points
		.flatMap((point, index) => [
			...(index === 0 ? [] : [ProgrammerToken.Thru]),
			...numberTokens(degrees ? point : point * 100),
		])
		.concat("ENTER");
}

/** Typed Position entry: the encoder value dialog takes degrees, with its − key for a sign. */
type DegreesValue =
	| { kind: "degrees"; value: number }
	| { kind: "degrees_spread"; value: number[] };

function degreesAttributeValue(
	value: number | ProgrammerExpression,
): DegreesValue {
	const intent = degreesEncoderValue(value);
	return intent.kind === "value"
		? { kind: "degrees", value: intent.value }
		: { kind: "degrees_spread", value: intent.value };
}

function scalarPercentages(
	value: AttributeValue,
): { kind: "value"; value: number } | { kind: "spread"; value: number[] } {
	if (value.kind === "normalized") return { kind: "value", value: value.value };
	if (value.kind === "spread") return { kind: "spread", value: value.value };
	throw new Error("Semantic Color encoder entry requires numeric points");
}

function numberTokens(value: number): string[] {
	const text = String(Math.abs(value)).split("");
	return value < 0 ? ["−", ...text] : text;
}

function assertPositiveSteps(value: number | ProgrammerExpression): void {
	if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1)
		throw new Error("Relative encoder steps must be a positive safe integer");
}

function stableIndex(value: string, length: number): number {
	let hash = 2166136261;
	for (const character of value) {
		hash ^= character.charCodeAt(0);
		hash = Math.imul(hash, 16777619);
	}
	return (hash >>> 0) % length;
}

/**
 * A semantic Pan/Tilt slot read back from the displayed output says so (`Pan · Resolved`, TL-549);
 * it is still the same encoder.
 */
const READOUT_SUFFIX = "(?: · Resolved)?";

function escapeRegex(value: string): string {
	return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
