import { expect, type Page } from "@playwright/test";
import type { ApiDriver } from "../core/api";
import type { LightBench } from "../core/lightBench";

type ProgrammerState = {
	values: Array<{
		fixture_id: string;
		attribute: string;
		value: Parameters<typeof normalized>[0];
	}>;
	group_values: Record<string, unknown>;
};

/** Operator-level contracts for the encoder display shown with attached hardware. */
export class BrowserAttachedEncoders {
	constructor(
		private readonly api: ApiDriver,
		private readonly bench: LightBench,
		private readonly page: Page,
	) {}

	async expectNavigationAndSecondaryEncoder(): Promise<void> {
		await this.withHardware(async (hardware, alias) => {
			const navigate = async (
				value: "up" | "down" | "left" | "right",
				family: string,
			) => {
				await hardware.send(`/light/${alias}/nav`, [value]);
				await expect(
					// A family tab counts its pages when it has more than one (e.g. Direct Color).
					this.page.getByRole("button", { name: new RegExp(`^${family}( \\d+ of \\d+)?$`) }),
				).toHaveClass(/active/);
			};
			for (const family of [
				"Color",
				"Position",
				"Beam",
				"Shapers",
				"Focus",
				"Control",
				"Media",
				"Intensity",
			])
				await navigate("down", family);
			await navigate("up", "Media");
			await navigate("left", "Control");
			await navigate("right", "Media");
			await navigate("down", "Intensity");
			await navigate("down", "Color");
			await navigate("down", "Position");

			const tilt = this.page.getByRole("button", {
				name: /^Encoder 2: Tilt(?: · Resolved)?,/,
			});
			const displayedDegrees = async () =>
				(await tilt.locator("strong").first().textContent()) ?? "";
			// Tilt is an Angle in degrees (TL-552, help: Position encoders). The idle Profile Moving
			// Light sits at its declared default pose, +0.5° (DMX 128 on a nominal 270° travel), and
			// one coarse detent moves ten 1° steps.
			await hardware.send(`/light/${alias}/encode/2`, ["right"]);
			await expect.poll(displayedDegrees).toBe("10.5°");
			await hardware.send(`/light/${alias}/encode/2`, ["press"]);
			const dialog = this.page.getByRole("dialog", {
				name: "Encoder 2 value",
				exact: true,
			});
			await expect(dialog).toBeVisible();
			await expect(dialog.getByRole("heading")).toHaveText("Tilt");
		});
	}

	async expectTypedIntensitySpread(): Promise<void> {
		await this.expectSpread({
			family: "Intensity",
			label: "Dimmer",
			expression: ["0", "THRU", "5", "0"],
			fixtures: [1, 2, 3, 4, 5],
			attribute: "intensity",
			values: [0, 0.125, 0.25, 0.375, 0.5],
			display: "0%...50%",
		});
	}

	async expectMultiPointIntensitySpread(): Promise<void> {
		await this.expectSpread({
			family: "Intensity",
			label: "Dimmer",
			expression: ["1", "0", "0", "THRU", "0", "THRU", "1", "0", "0"],
			fixtures: [1, 2, 3, 4, 5],
			attribute: "intensity",
			values: [1, 0.5, 0, 0.5, 1],
			display: "0%...100%",
		});
	}

	/**
	 * Since the TL-552 cutover Pan is an Angle in degrees: the old 100 % THRU 0 % THRU 100 % is
	 * +270° THRU −270° THRU +270° over a 540° travel centred on home.
	 */
	async expectMultiPointPanSpread(): Promise<void> {
		await this.expectSpread({
			family: "Position",
			label: "Pan",
			expression: ["2", "7", "0", "THRU", "−", "2", "7", "0", "THRU", "2", "7", "0"],
			fixtures: [101, 102, 103, 104, 105],
			attribute: "position",
			values: [270, 0, -270, 0, 270],
			// A semantic readout never summarises differing requests as a range or an average:
			// the selection reads Mixed (TL-619 presentation, docs/testing/34 POSITION-CONTROLS-005).
			display: "Mixed",
		});
	}

	private async expectSpread(options: {
		family: string;
		label: string;
		expression: string[];
		fixtures: number[];
		attribute: string;
		values: number[];
		display: string;
	}): Promise<void> {
		await this.withHardware(async () => {
			await this.page
				.getByRole("button", { name: options.family, exact: true })
				.click();
			await this.page
				.getByRole("button", {
					name: new RegExp(`^Encoder 1: ${options.label}(?: · Resolved)?,`),
				})
				.click();
			const dialog = this.page.getByRole("dialog", {
				name: "Encoder 1 value",
				exact: true,
			});
			for (const key of options.expression)
				await dialog.getByRole("button", { name: key, exact: true }).click();
			await dialog.getByRole("button", { name: "ENTER", exact: true }).click();
			await expect(dialog).toBeHidden();
			await this.expectProgrammerValues(
				options.fixtures,
				options.attribute,
				options.values,
			);
			await expect(
				this.page
					.locator(".hardware-encoder-display")
					.filter({ hasText: options.label }),
			).toContainText(options.display);
		});
	}

	private async expectProgrammerValues(
		fixtureNumbers: number[],
		attribute: string,
		expected: number[],
	): Promise<void> {
		const bootstrap = await this.api.request<{
			active_show: { id: string } | null;
		}>("GET", "/api/v2/bootstrap", undefined, false);
		if (!bootstrap.active_show) throw new Error("No active Show");
		const fixtures = await this.api.showObjects<{
			fixture_number: number;
			fixture_id: string;
		}>(bootstrap.active_show.id, "patched_fixture");
		const ids = new Map(
			fixtures.map((fixture) => [
				fixture.body.fixture_number,
				fixture.body.fixture_id,
			]),
		);
		await expect
			.poll(async () => {
				const programmers = await this.api.request<ProgrammerState[]>(
					"GET",
					"/api/v2/programmers",
				);
				return programmers.some((programmer) => {
					if (
						programmer.values.length !== fixtureNumbers.length ||
						Object.keys(programmer.group_values).length !== 0
					)
						return false;
					return fixtureNumbers.every((number, index) => {
						const entries = programmer.values.filter(
							(value) =>
								value.fixture_id === ids.get(number) &&
								value.attribute === attribute,
						);
						return (
							entries.length === 1 &&
							normalized(entries[0]?.value) === expected[index]
						);
					});
				});
			})
			.toBe(true);
	}

	private async withHardware(
		action: (
			hardware: Awaited<ReturnType<LightBench["osc"]>>,
			alias: string,
		) => Promise<void>,
	): Promise<void> {
		const hardware = await this.bench.osc();
		const alias = "desk";
		if (!alias) throw new Error("Attached encoder scenario requires a desk alias");
		const clientId = `attached-encoder-${crypto.randomUUID()}`;
		try {
			await hardware.subscribe(clientId, alias);
			await expect
				.poll(
					async () =>
						(
							await this.api.request<{ hardware_connected: boolean }>(
								"GET",
								"/api/v2/bootstrap",
								undefined,
								false,
							)
						).hardware_connected,
				)
				.toBe(true);
			await action(hardware, alias);
		} finally {
			await hardware
				.send("/light/unsubscribe", [clientId])
				.catch(() => undefined);
			await hardware.close();
		}
	}
}

function normalized(
	value:
		| number
		| {
				kind?: string;
				value?: number | { kind?: string; pan_degrees?: { value?: number } };
				values?: Array<number | { value?: number }>;
		  }
		| undefined,
) {
	if (typeof value === "number") return value;
	// A semantic Position value reads as its Pan Angle in degrees.
	if (value?.kind === "position" && typeof value.value === "object")
		return value.value.kind === "angles"
			? value.value.pan_degrees?.value
			: undefined;
	const first = value?.values?.[0];
	if (typeof first === "number") return first;
	return first?.value ?? value?.value;
}
