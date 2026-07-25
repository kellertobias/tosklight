import type { ApiDriver } from "./api";
import type { ClockDuration } from "./clockScenario";
import { parseClockDuration } from "./clockScenario";
import type { DeskDriver } from "./desk";

export class BrowserCueFade {
	constructor(
		private readonly api: ApiDriver,
		private readonly desk: DeskDriver,
	) {}

	async set(duration: ClockDuration) {
		const millis = parseClockDuration(duration);
		if (millis > 60_000)
			throw new Error("Cue Fade cannot exceed the desk's 60 second range");
		await this.desk.recordStep(
			"CUE FADE",
			`Set the Cue Fade fallback to ${duration}.`,
		);
		await this.write(millis);
	}

	async double() {
		await this.write(Math.min(60_000, (await this.currentMillis()) * 2));
	}

	async half() {
		await this.write(Math.floor((await this.currentMillis()) / 2));
	}

	async off() {
		await this.write(0);
	}

	async currentMillis() {
		const response = await this.configuration();
		return response.sequence_master_fade_millis ?? 0;
	}

	async expectMillis(expected: number) {
		const deadline = Date.now() + 5_000;
		do {
			if ((await this.currentMillis()) === expected) return;
			await new Promise((resolve) => setTimeout(resolve, 10));
		} while (Date.now() < deadline);
		throw new Error(`Timed out waiting for authoritative Cue Fade ${expected} ms`);
	}

	private async write(millis: number) {
		await this.api.request("PUT", "/api/v2/configuration", {
			...(await this.configuration()),
			sequence_master_fade_millis: millis,
		});
		await this.expectMillis(millis);
	}

	private async configuration() {
		const response = await this.api.request<any>("GET", "/api/v2/configuration");
		return response.configuration ?? response;
	}
}
