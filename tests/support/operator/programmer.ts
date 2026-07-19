import type {
	ApiDriver,
	CommandOperationResponse,
} from "../../../apps/control-ui/e2e/bench/api";
import type { LightBench } from "../../../apps/control-ui/e2e/bench/lightBench";
import type { OscHardware } from "../../../apps/control-ui/e2e/bench/protocols";
import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";
import type { Page } from "../../../apps/control-ui/node_modules/@playwright/test/index.js";
import {
	oscProgrammerActionForKey,
	type SoftwareKey,
} from "../../../apps/shared/programmerKeypad";

export type ProgrammerSurface =
	| { via: "command-line"; api: ApiDriver }
	| { via: "software"; page: Page }
	| { via: "osc"; api: ApiDriver; hardware: OscHardware };

type OscProgrammerSurface = Extract<ProgrammerSurface, { via: "osc" }>;

export interface ProgrammerStepOptions {
	expectedCommandLine?: string | RegExp;
}

/**
 * Performs one intent-level programmer step through the named public surface.
 *
 * The caller supplies logical desk keys, so a scenario can reuse the same intent while retaining
 * an explicit command-line, visible software, or OSC interaction path.
 */
export async function doProgrammerStep(
	surface: ProgrammerSurface,
	keys: readonly SoftwareKey[],
	options: ProgrammerStepOptions = {},
): Promise<void> {
	for (const key of keys) await pressProgrammerKey(surface, key);
	if (options.expectedCommandLine !== undefined)
		await expectCommandLine(surface, options.expectedCommandLine);
}

/** Runs an OSC programmer interaction with subscription ownership and cleanup kept in one place. */
export async function withOscProgrammer<T>(
	api: ApiDriver,
	bench: LightBench,
	action: (surface: OscProgrammerSurface) => Promise<T>,
): Promise<T> {
	if (!api.session) throw new Error("OSC programmer surface requires an API session");
	const hardware = await bench.osc();
	const clientId = `operator-programmer-${crypto.randomUUID()}`;
	await hardware.subscribe(clientId, api.session.desk.osc_alias);
	try {
		return await action({ via: "osc", api, hardware });
	} finally {
		await hardware.send("/light/unsubscribe", [clientId]).catch(() => undefined);
	}
}

async function pressProgrammerKey(
	surface: ProgrammerSurface,
	key: SoftwareKey,
): Promise<void> {
	switch (surface.via) {
		case "command-line":
			assertAccepted(await surface.api.sendCommandKey(key), key);
			return;
		case "software":
			await softwareKey(surface.page, key).click();
			return;
		case "osc":
			await tapOscKey(surface, key);
	}
}

function assertAccepted(result: CommandOperationResponse, key: SoftwareKey): void {
	if (result.outcome === "rejected")
		throw new Error(`Programmer key ${key} was rejected: ${result.error}`);
}

function softwareKey(page: Page, key: SoftwareKey) {
	if (key === "REC") return page.locator(".global-store-button");
	if (key === "ESC") return page.locator(".command-escape");
	return page
		.locator(`.programmer-number-block [data-keypad-key="${key}"]`)
		.filter({ visible: true })
		.first();
}

async function tapOscKey(
	surface: OscProgrammerSurface,
	key: SoftwareKey,
): Promise<void> {
	if (!surface.api.session) throw new Error("OSC programmer surface lost its API session");
	const action = key === "REC" ? "record" : oscProgrammerActionForKey(key);
	const address = `/light/${surface.api.session.desk.osc_alias}/programmer/${action}`;
	await surface.hardware.send(address, [true]);
	await surface.hardware.send(address, [false]);
}

async function expectCommandLine(
	surface: ProgrammerSurface,
	expected: string | RegExp,
): Promise<void> {
	if (surface.via === "software") {
		await expect(surface.page.getByLabel("Command line")).toHaveValue(expected);
		return;
	}
	const commandLine = expect.poll(
		async () => (await surface.api.getCommandLine()).commandLine.text,
	);
	if (typeof expected === "string") await commandLine.toBe(expected);
	else await commandLine.toMatch(expected);
}
