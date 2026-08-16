import { describe, expect, it, vi } from "vitest";
import type {
	ApiDriver,
	CommandOperationResponse,
	RevisionedCommandLine,
} from "../core/api";
import { BrowserCommands, BrowserKeypad, commandKeys } from "./commandScenario";
import type { DeskDriver } from "../core/desk";

describe("command scenario primitives", () => {
	it("types through logical desk keys and executes with the visible ENT key", async () => {
		const harness = commandHarness();

		await harness.commands.execute("FIXTURE 1 AT 50");

		expect(harness.click.mock.calls.map(([locator]) => locator)).toEqual([
			harness.escapeKey,
			harness.keys["1"],
			harness.keys.AT,
			harness.keys["5"],
			harness.keys["0"],
			harness.ent,
		]);
		expect(harness.keyboardPress).not.toHaveBeenCalled();
		expect(harness.record).toHaveBeenCalledWith(
			"COMMAND EXECUTE",
			expect.stringContaining("ui command-line route"),
		);
	});

	it("types without execution and clears through the visible ESC control", async () => {
		const harness = commandHarness();
		await harness.commands.type("GROUP 4");
		expect(harness.click.mock.calls.map(([locator]) => locator)).toEqual([
			harness.escapeKey,
			harness.keys.GRP,
			harness.keys["4"],
		]);

		await harness.commands.clear();
		expect(harness.click.mock.calls.at(-1)?.[0]).toBe(harness.escapeKey);
	});

	it("uses typed command-line API operations without raw command execution", async () => {
		const harness = commandHarness();
		await harness.commands.via.api.execute("FIXTURE 2");
		expect(harness.setCommandLineText).toHaveBeenCalledWith("FIXTURE 2");
		expect(harness.executeCommandLine).toHaveBeenCalledWith();

		harness.apiText.value = "FIXTURE";
		await harness.commands.via.api.clear();
		expect(harness.sendCommandKey).toHaveBeenCalledWith("ESC");
	});

	it("asserts command text through UI and API projections", async () => {
		const harness = commandHarness();
		await harness.commandLine.fill("F1 + F2");
		await harness.commands.expect("F1 + F2");
		harness.apiText.value = "GROUP";
		await harness.commands.via.api.expect(/^(FIXTURE|GROUP)$/);
	});

	it("rejects empty complete commands", async () => {
		const harness = commandHarness();
		await expect(harness.commands.execute("   ")).rejects.toThrow(
			/use command.clear/,
		);
		expect(harness.commandLine.fill).not.toHaveBeenCalled();
	});

	it("maps command aliases, numbers, and the persistent target to desk keys", () => {
		expect(commandKeys("FIXTURE 12 THRU 14", "FIXTURE")).toEqual([
			"1",
			"2",
			"TRU",
			"1",
			"4",
		]);
		expect(commandKeys("GROUP 3 AT 50", "FIXTURE")).toEqual([
			"GRP",
			"3",
			"AT",
			"5",
			"0",
		]);
		expect(commandKeys("RECORD + GROUP 3", "GROUP")).toEqual([
			"REC",
			"+",
			"GRP",
			"3",
		]);
		expect(commandKeys("DEGRP 4", "FIXTURE")).toEqual(["GRP", "GRP", "4"]);
		expect(commandKeys("SPD GRP 1 AT SPD GRP 3", "FIXTURE")).toEqual([
			"SHIFT",
			"TIME",
			"SHIFT",
			"1",
			"AT",
			"SHIFT",
			"TIME",
			"SHIFT",
			"3",
		]);
		expect(commandKeys("GO TO PBK 2 . 6 CUE 2.1", "FIXTURE")).toEqual([
			"SHIFT",
			"DIV",
			"SHIFT",
			"PLAYBACK",
			"2",
			".",
			"6",
			"CUE",
			"2",
			".",
			"1",
		]);
		expect(commandKeys("LOAD VPBK 1001 CUE 2.0.15", "FIXTURE")).toEqual([
			"SHIFT",
			"DIV",
			"DIV",
			"SHIFT",
			"PLAYBACK",
			"PLAYBACK",
			"1",
			"0",
			"0",
			"1",
			"CUE",
			"2",
			".",
			"0",
			".",
			"1",
			"5",
		]);
	});

	it("presses exact keypad keys in order through DeskDriver", async () => {
		const harness = commandHarness();
		await harness.keypad.press(["GRP", "1", "+", "2", "ENT"]);
		expect(harness.click.mock.calls.map(([locator]) => locator)).toEqual([
			harness.keys.GRP,
			harness.keys["1"],
			harness.keys["+"],
			harness.keys["2"],
			harness.ent,
		]);
		expect(harness.record).toHaveBeenCalledWith(
			"KEYPAD",
			"Press [GRP] [1] [+] [2] [ENT] in exact order.",
		);
	});

	it("owns visible special-key paths and rejects an empty key sequence", async () => {
		const harness = commandHarness();
		await harness.keypad.press([
			"REC",
			"PRE",
			"ESC",
			"HIGH",
			"PREV",
			"NEXT",
			"ALL",
		]);
		expect(harness.click.mock.calls.map(([locator]) => locator)).toEqual([
			harness.recordKey,
			harness.preload,
			harness.escapeKey,
			harness.keys.HIGH,
			harness.keys.PREV,
			harness.keys.NEXT,
			harness.keys.ALL,
		]);
		await expect(harness.keypad.press([])).rejects.toThrow(/at least one/);
	});
});

function commandHarness() {
	const commandLine = new FakeLocator("command-line", "FIXTURE");
	const ent = new FakeLocator("ENT");
	const escapeKey = new FakeLocator("ESC");
	const recordKey = new FakeLocator("REC");
	const preload = new FakeLocator("PRE");
	const keys = Object.fromEntries(
		[
			"GRP",
			"1",
			"2",
			"4",
			"5",
			"0",
			"AT",
			"+",
			"HIGH",
			"PREV",
			"NEXT",
			"ALL",
		].map((key) => [key, new FakeLocator(key)]),
	) as Record<string, FakeLocator>;
	keys.ENT = ent;
	const keyboardPress = vi.fn();
	const page = {
		getByRole: vi.fn(() => commandLine),
		locator: vi.fn((selector: string) => {
			if (selector.includes("global-store-button")) return recordKey;
			if (selector.includes("command-escape")) return escapeKey;
			if (selector.includes("preload-button")) return preload;
			const key = /\[data-keypad-key="([^"]+)"\]/.exec(selector)?.[1];
			if (!key) return new FakeLocator(selector);
			keys[key] ??= new FakeLocator(key);
			return keys[key];
		}),
		keyboard: { press: keyboardPress },
	};
	// Authoritative command history, as the desk's own would grow. The UI route deliberately
	// polls it rather than trusting the click, so a stub that never grows would hang the poll.
	const history: { command: string; status: string; feedback: string }[] = [];
	const click = vi.fn(async (target: FakeLocator) => {
		if (target === escapeKey) commandLine.value = "FIXTURE";
		if (target === ent) {
			history.push({
				command: commandLine.value,
				status: "accepted",
				feedback: "",
			});
		}
	});
	const record = vi.fn(async () => undefined);
	const apiText = { value: "FIXTURE" };
	const setCommandLineText = vi.fn(async (text: string) => {
		apiText.value = text;
		return revisioned(text);
	});
	const executeCommandLine = vi.fn(async () => accepted("ENT"));
	const sendCommandKey = vi.fn(async () => {
		apiText.value = "FIXTURE";
		return accepted("ESC");
	});
	const request = vi.fn(async (_method: string, path: string) => {
		if (path.includes("command-history")) return history;
		throw new Error(`the command harness has no stub for ${path}`);
	});
	const api = {
		setCommandLineText,
		executeCommandLine,
		sendCommandKey,
		request,
		getCommandLine: vi.fn(async () => revisioned(apiText.value)),
	} as unknown as ApiDriver;
	const desk = {
		click,
		recordStep: record,
	} as unknown as DeskDriver;
	return {
		commands: new BrowserCommands(api, desk, page as never),
		keypad: new BrowserKeypad(desk, page as never),
		commandLine,
		ent,
		escapeKey,
		recordKey,
		preload,
		keys,
		click,
		record,
		apiText,
		setCommandLineText,
		executeCommandLine,
		sendCommandKey,
		request,
		history,
		keyboardPress,
	};
}

class FakeLocator {
	readonly _apiName = "Locator";
	readonly fill = vi.fn(async (value: string) => {
		this.value = value;
	});

	constructor(
		readonly name: string,
		public value = "",
	) {}

	first(): FakeLocator {
		return this;
	}

	async inputValue(): Promise<string> {
		return this.value;
	}

	async isVisible(): Promise<boolean> {
		return true;
	}

	toString(): string {
		return `FakeLocator(${this.name})`;
	}

	async _expect(
		_expression: string,
		options: {
			expectedText: Array<{
				string?: string;
				regexSource?: string;
				regexFlags?: string;
			}>;
		},
	) {
		const expected = options.expectedText[0];
		const matches =
			expected.string !== undefined
				? this.value === expected.string
				: new RegExp(
						expected.regexSource ?? "",
						expected.regexFlags ?? "",
					).test(this.value);
		return {
			matches,
			received: { value: this.value },
			log: [],
			timedOut: false,
		};
	}
}

function revisioned(text: string): RevisionedCommandLine {
	return {
		commandLine: {
			text,
			target: text.startsWith("GROUP") ? "GROUP" : "FIXTURE",
			pristine: text === "FIXTURE" || text === "GROUP",
			revision: 1,
			pending_choice: null,
		},
		etag: '"1"',
	};
}

function accepted(key: string): CommandOperationResponse {
	return {
		request_id: key,
		outcome: "accepted",
		action: key,
		command_line: revisioned("FIXTURE").commandLine,
	};
}
