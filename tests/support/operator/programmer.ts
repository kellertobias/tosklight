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

export interface ProgrammerCommandOptions extends ProgrammerStepOptions {
	/** Clear only the desk command line before entering the command. */
	reset?: boolean;
	/** Optional post-execution command-line assertion; omit it when the scenario observes state. */
	expectedCompletion?: string | RegExp | null;
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

/** Enters and executes one readable command through the explicitly named surface. */
export async function executeProgrammerCommand(
	surface: ProgrammerSurface,
	command: string,
	options: ProgrammerCommandOptions = {},
): Promise<void> {
	if (options.reset !== false) await doProgrammerStep(surface, ["ESC"]);
	await doProgrammerStep(surface, programmerKeysForCommand(command), {
		expectedCommandLine: options.expectedCommandLine,
	});
	const expectedCompletion = options.expectedCompletion ?? null;
	await doProgrammerStep(
		surface,
		["ENT"],
		expectedCompletion === null
			? {}
			: { expectedCommandLine: expectedCompletion },
	);
}

/** Converts operator command text into the physical logical keys used by UI and OSC tests. */
export function programmerKeysForCommand(command: string): SoftwareKey[] {
	const trimmed = command.trim();
	if (!trimmed) throw new Error("Programmer command must not be empty");
	return trimmed.split(/\s+/).flatMap(keysForToken);
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
		try {
			await hardware.send("/light/unsubscribe", [clientId]);
		} catch {
			// Cleanup must not hide the operator scenario's original failure.
		} finally {
			await hardware.close();
		}
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

const TOKEN_ALIASES: Readonly<Record<string, readonly SoftwareKey[]>> = {
	GROUP: ["GRP"],
	DEGRP: ["GRP", "GRP"],
	THRU: ["TRU"],
	RECORD: ["REC"],
	DELETE: ["DEL"],
	MOVE: ["MOV"],
	COPY: ["CPY"],
	UNDO: ["UND"],
	PRELOAD: ["PRE"],
};

const NAMED_KEYS = new Set<SoftwareKey>([
	"SET",
	"GRP",
	"CUE",
	"UND",
	"CLR",
	"DEL",
	"MOV",
	"CPY",
	"TRU",
	"DIV",
	"BACKSPACE",
	"AT",
	"PRE",
	"REC",
	"ESC",
	"SHIFT",
	"TIME",
	"SELECT",
	"+",
	"-",
	".",
]);

function keysForToken(token: string): SoftwareKey[] {
	const normalized = token.toUpperCase();
	const alias = TOKEN_ALIASES[normalized];
	if (alias) return [...alias];
	if (/^\d+(?:[.,]\d+)?$/.test(normalized))
		return [...normalized.replace(",", ".")] as SoftwareKey[];
	if (NAMED_KEYS.has(normalized as SoftwareKey))
		return [normalized as SoftwareKey];
	throw new Error(`Unsupported Programmer command token: ${token}`);
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
	const alias = surface.api.session.desk.osc_alias;
	const address = `/light/${alias}/programmer/${action}`;
	await sendOscPhase(surface.hardware, alias, address, true);
	await sendOscPhase(surface.hardware, alias, address, false);
}

async function sendOscPhase(
	hardware: OscHardware,
	alias: string,
	address: string,
	pressed: boolean,
) {
	const mark = hardware.mark();
	await hardware.send(address, [pressed]);
	await hardware.expectAfter(mark, `/light/${alias}/feedback/command-line`);
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
