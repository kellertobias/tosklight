import { expect, type Page } from "@playwright/test";
import { HttpProgrammerValuesTransport } from "../../../src/api/ProgrammerValuesTransport";
import type { ProgrammerValuesScope } from "../../../src/features/programmerValues/contracts";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type { SimulatedHardware } from "../hardware/hardwareScenario";

export class BrowserOscEncoderRoute {
	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly hardware: SimulatedHardware,
	) {}

	async detents(
		family: string,
		label: string,
		operation: "add" | "subtract",
		steps: number,
	): Promise<void> {
		await this.desk.click(
			this.page.getByRole("button", { name: family, exact: true }),
		);
		const display = this.page.getByLabel(
			new RegExp(`^Encoder \\d+: ${escapeRegex(label)},`),
		);
		await expect(display).toBeVisible();
		const ariaLabel = await display.getAttribute("aria-label");
		const slot = ariaLabel?.match(/^Encoder (\d+):/)?.[1];
		if (!slot)
			throw new Error(
				`${family} ${label} did not expose a live attached-hardware encoder slot`,
			);
		const session = this.api.session;
		if (!session) throw new Error("Encoder action requires an API session");
		const values = new HttpProgrammerValuesTransport({
			baseUrl: this.api.baseUrl,
			sessionToken: session.token,
		});
		const scope = {
			showId: await this.activeShowId(),
			userId: session.user.id,
		};
		let revision = (await values.loadSnapshot(scope)).projection.revision;
		for (let index = 0; index < steps; index += 1) {
			await this.hardware.send(
				`/light/${session.desk.osc_alias}/encode/${slot}`,
				[operation === "add" ? "up" : "down"],
			);
			revision = await this.waitForProgrammerRevision(values, scope, revision);
		}
	}

	private async activeShowId(): Promise<string> {
		const bootstrap = await this.api.request<{
			active_show: { id: string } | null;
		}>("GET", "/api/v2/bootstrap");
		if (!bootstrap.active_show) throw new Error("No active Show");
		return bootstrap.active_show.id;
	}

	private async waitForProgrammerRevision(
		values: HttpProgrammerValuesTransport,
		scope: ProgrammerValuesScope,
		afterRevision: number,
	): Promise<number> {
		const deadline = Date.now() + 5_000;
		do {
			const revision = (await values.loadSnapshot(scope)).projection.revision;
			if (revision > afterRevision) return revision;
			await this.page.waitForTimeout(10);
		} while (Date.now() < deadline);
		throw new Error(
			`Timed out waiting for an OSC encoder detent after Programmer revision ${afterRevision}`,
		);
	}
}

function escapeRegex(value: string): string {
	return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
