import { expect, type Locator, type Page } from "@playwright/test";
import type { AttributeValue } from "../../../src/api/types/playback";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import {
	BeamAttribute,
	ColorAttribute,
	EncoderGroup,
	FocusAttribute,
	IntensityAttribute,
	type ProgrammerExpression,
	PositionAttribute,
	ProgrammerToken,
	ShapersAttribute,
	encoderCatalogEntry,
	normalizedEncoderValue,
} from "./encoderCatalog";
import { BrowserOscEncoderRoute } from "./encoderOscScenario";
import type { SimulatedHardware } from "../hardware/hardwareScenario";
import {
	applyProgrammerSelectionValue,
	batchProgrammerValues,
	clearProgrammerValues,
} from "../programmer/programmerValues";
import { BrowserDiscreteEncoders } from "./discreteEncoderScenario";
import type { BrowserSelection } from "../command-selection/selectionScenario";

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
	constructor(
		private readonly encoder: NormalizedEncoder,
	) {}

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

class VisibleAbsoluteEncoderPort implements AbsoluteEncoderPort {
	constructor(private readonly encoder: NormalizedEncoder) {}

	set(value: number | ProgrammerExpression): Promise<void> {
		return this.encoder.execute("set", value, "ui");
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
		ui: new VisibleAbsoluteEncoderPort(this),
		osc: new OscRelativeEncoderPort(this),
	};

	constructor(
		private readonly owner: BrowserEncoders,
		readonly group: EncoderGroup,
		readonly key: string,
	) {}

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
			operation === "set"
				? normalizedEncoderValue(input)
				: (input as number);
		await this.desk.recordStep(
			"ENCODER",
			`${operation} ${catalog.familyLabel} ${catalog.label} through the ${route.toUpperCase()} route.`,
		);
		if (route === "ui") {
			if (operation !== "set")
				throw new Error(
					"Visible relative encoder detents require attached hardware and are not available through the software value-entry route",
				);
			await this.visibleSet(catalog.familyLabel, catalog.label, value as AttributeValue);
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
		await this.apiMutation(catalog.attribute, operation, value);
	}

	private async apiMutation(
		attribute: string,
		operation: EncoderOperation,
		value: AttributeValue | number,
	): Promise<void> {
		const [selection, bootstrap, configurationResponse] = await Promise.all([
			this.selection.observe(),
			this.api.request<{ active_show: { id: string } | null }>(
				"GET",
				"/api/v2/bootstrap",
			),
			this.api.request<{
				configuration?: { programmer_fade_millis: number };
				programmer_fade_millis?: number;
			}>("GET", "/api/v2/configuration"),
		]);
		if (!bootstrap.active_show) throw new Error("No active Show");
		if (selection.selected.length === 0)
			throw new Error("Encoder action requires a non-empty Fixture selection");
		const configuration =
			configurationResponse.configuration ?? configurationResponse;
		const fadeMillis = configuration.programmer_fade_millis ?? 0;
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
							delta:
								(operation === "add" ? 1 : -1) *
								(value as number) *
								0.01,
						},
			timing: {
				fade: fadeMillis > 0,
				fadeMillis: fadeMillis > 0 ? fadeMillis : null,
				delayMillis: null,
			},
		});
	}

	private async programmerContext(): Promise<{
		showId: string;
		fixtureIds: string[];
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
		value: AttributeValue,
	): Promise<void> {
		await this.desk.click(
			this.page.getByRole("button", { name: family, exact: true }),
		);
		const control = this.softwareControl(label);
		if (!(await control.isVisible()))
			throw new Error(
				`${family} ${label} is not present on the live software encoder page`,
			);
		await this.desk.click(
			control.getByRole("button", { name: "Set value", exact: true }),
		);
		const dialog = this.page.getByRole("dialog", {
			name: new RegExp(`^Enc \\d+ · ${escapeRegex(label)} value$`),
		});
		await expect(dialog).toBeVisible();
		for (const token of valueTokens(value))
			await this.desk.click(
				dialog.getByRole("button", { name: token, exact: true }),
			);
		await expect(dialog).toBeHidden();
	}

	private softwareControl(label: string): Locator {
		return this.page
			.locator(".vertical-touch-fader-stack")
			.filter({
				has: this.page.getByRole("slider", {
					name: new RegExp(`^Enc \\d+ · ${escapeRegex(label)}$`),
				}),
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

function valueTokens(value: AttributeValue): string[] {
	const points =
		value.kind === "normalized"
			? [value.value]
			: value.kind === "spread"
				? value.value
				: [];
	if (points.length === 0)
		throw new Error("Visible normalized encoder entry requires numeric points");
	return points.flatMap((point, index) => [
		...(index === 0 ? [] : [ProgrammerToken.Thru]),
		...percentageTokens(point * 100),
	]).concat("ENTER");
}

function percentageTokens(value: number): string[] {
	return String(value).split("").map((token) => (token === "." ? "." : token));
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

function escapeRegex(value: string): string {
	return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
