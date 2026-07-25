import { expect, type Page } from "@playwright/test";
import type { ApiDriver } from "../core/api";
import type { LightBench } from "../core/lightBench";

type ProgrammerState = {
	values: Array<{
		fixture_id: string;
		attribute: string;
		value:
			| number
			| { value?: number; values?: Array<number | { value?: number }> };
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
					this.page.getByRole("button", { name: family, exact: true }),
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
				name: /^Encoder 2: Tilt,/,
			});
			const displayedPercent = async () =>
				Number.parseInt(
					(await tilt.locator("strong").first().textContent()) ?? "",
					10,
				);
			await hardware.send(`/light/${alias}/encode/2`, ["right"]);
			await expect.poll(displayedPercent).toBe(60);
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

	async expectMultiPointPanSpread(): Promise<void> {
		await this.expectSpread({
			family: "Position",
			label: "Pan",
			expression: ["1", "0", "0", "THRU", "0", "THRU", "1", "0", "0"],
			fixtures: [101, 102, 103, 104, 105],
			attribute: "pan",
			values: [1, 0.5, 0, 0.5, 1],
			display: "0%...100%",
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
					name: new RegExp(`^Encoder 1: ${options.label},`),
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
		const alias = this.api.session?.desk.osc_alias;
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
		| { value?: number; values?: Array<number | { value?: number }> }
		| undefined,
) {
	if (typeof value === "number") return value;
	const first = value?.values?.[0];
	if (typeof first === "number") return first;
	return first?.value ?? value?.value;
}
