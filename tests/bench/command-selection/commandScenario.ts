import { expect, type Locator, type Page } from "@playwright/test";
import type { SoftwareKey } from "@tosklight/ui/programmer-keypad";
import type { ApiDriver, CommandOperationResponse } from "../core/api";
import type { DeskDriver } from "../core/desk";

type KeypadDigit = "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9";

export type KeypadKey =
	| Exclude<SoftwareKey, `${number}`>
	| KeypadDigit
	| "HIGH"
	| "PREV"
	| "NEXT"
	| "ALL";

export type CommandExpectation = string | RegExp;

class CommandActionSurface {
	constructor(private readonly adapter: CommandAdapter) {}

	execute(text: string): Promise<void> {
		return this.adapter.execute(text);
	}

	type(text: string): Promise<void> {
		return this.adapter.type(text);
	}

	clear(): Promise<void> {
		return this.adapter.clear();
	}

	expect(text: CommandExpectation): Promise<void> {
		return this.adapter.expect(text);
	}
}

export class BrowserCommands {
	private readonly ui: CommandAdapter;
	private readonly apiRoute: CommandAdapter;
	readonly via: { ui: CommandActionSurface; api: CommandActionSurface };
	readonly history: BrowserCommandHistory;

	constructor(api: ApiDriver, desk: DeskDriver, page?: Page) {
		this.ui = new CommandAdapter("ui", api, desk, page);
		this.apiRoute = new CommandAdapter("api", api, desk, page);
		this.history = new BrowserCommandHistory(api, desk, page);
		this.via = {
			ui: new CommandActionSurface(this.ui),
			api: new CommandActionSurface(this.apiRoute),
		};
	}

	execute(text: string): Promise<void> {
		return this.via.ui.execute(text);
	}

	type(text: string): Promise<void> {
		return this.via.ui.type(text);
	}

	clear(): Promise<void> {
		return this.via.ui.clear();
	}

	expect(text: CommandExpectation): Promise<void> {
		return this.ui.expect(text);
	}
}

type CommandHistoryEntry = {
	command: string;
	status: "accepted" | "rejected";
	feedback: string;
};

export class BrowserCommandHistory {
	constructor(
		private readonly api: ApiDriver,
		private readonly desk: DeskDriver,
		private readonly page?: Page,
	) {}

	async expectAcceptedAndRejected(): Promise<void> {
		const page = this.browser();
		await this.enter("FIXTURE 1 AT 25");
		await this.enter("FIXTURE 1 AT 101");
		await expect.poll(async () => (await this.entries()).length).toBe(2);

		const input = page.getByRole("textbox", {
			name: "Command line",
			exact: true,
		});
		await this.desk.click(input);
		const panel = page.getByRole("dialog", {
			name: "Command line history",
			exact: true,
		});
		await expect(panel).toBeVisible();
		await expect(panel.locator(".command-history-entry")).toHaveCount(2);
		const entries = await this.entries();
		expect(
			entries.map(({ command, status }) => ({ command, status })),
		).toEqual([
			{ command: "FIXTURE 1 AT 101", status: "rejected" },
			{ command: "FIXTURE 1 AT 25", status: "accepted" },
		]);
		expect(entries[0]?.feedback).toMatch(/within 0-100/i);
		expect(entries[1]?.feedback).toBe("Applied to 1 target(s)");
	}

	private async enter(value: string): Promise<void> {
		const page = this.browser();
		const historyLength = (await this.entries()).length;
		const input = page.getByRole("textbox", {
			name: "Command line",
			exact: true,
		});
		if (
			await input.evaluate((element) => element.classList.contains("completed"))
		)
			await this.desk.click(page.locator(".command-escape:visible").first());
		await input.fill(value);
		await input.press("Enter");
		await expect
			.poll(async () => (await this.entries()).length)
			.toBe(historyLength + 1);
	}

	private entries(): Promise<CommandHistoryEntry[]> {
		return this.api.request("GET", "/api/v2/command-history");
	}

	private browser(): Page {
		if (!this.page)
			throw new Error("Command history requires a browser page");
		return this.page;
	}
}

export class BrowserKeypad {
	constructor(
		private readonly desk: DeskDriver,
		private readonly page?: Page,
	) {}

	async press(keys: readonly KeypadKey[]): Promise<void> {
		if (keys.length === 0)
			throw new Error("keypad.press requires at least one desk key");
		await this.desk.recordStep(
			"KEYPAD",
			`Press ${keys.map((key) => `[${key}]`).join(" ")} in exact order.`,
		);
		for (const key of keys)
			await this.desk.click(keypadLocator(this.browser(), key));
	}

	private browser(): Page {
		if (!this.page)
			throw new Error("The visible keypad route requires a browser page");
		return this.page;
	}
}

export class CommandAdapter {
	constructor(
		private readonly route: "ui" | "api",
		private readonly api: ApiDriver,
		private readonly desk: DeskDriver,
		private readonly page?: Page,
	) {}

	async execute(text: string): Promise<void> {
		const command = requiredCommand(text);
		const historyLength =
			this.route === "ui"
				? (
						await this.api.request<CommandHistoryEntry[]>(
							"GET",
							"/api/v2/command-history",
						)
					).length
				: null;
		await this.desk.recordStep(
			"COMMAND EXECUTE",
			`Enter "${command}" and execute it through the ${this.route} command-line route.`,
		);
		await this.replace(command);
		if (this.route === "api") {
			assertAccepted(await this.api.executeCommandLine(), "ENT");
			return;
		}
		// Deliberately click the desk key: raw keyboard Enter is not the UI acceptance route.
		await this.desk.click(keypadLocator(this.browser(), "ENT"));
		await expect
			.poll(
				async () => {
					const choice = this.browser().getByRole("dialog", { name: /choice/i });
					if (await choice.isVisible().catch(() => false)) return "choice";
					const length = (
						await this.api.request<CommandHistoryEntry[]>(
							"GET",
							"/api/v2/command-history",
						)
					).length;
					return length > (historyLength ?? -1) ? "history" : "pending";
				},
				{
					message: `Command "${command}" should reach authoritative history or a typed choice`,
				},
			)
			.toMatch(/^(?:choice|history)$/);
	}

	async type(text: string): Promise<void> {
		const command = requiredCommand(text);
		await this.desk.recordStep(
			"COMMAND TYPE",
			`Enter "${command}" without executing it through the ${this.route} command-line route.`,
		);
		await this.replace(command);
	}

	async clear(): Promise<void> {
		await this.desk.recordStep(
			"COMMAND CLEAR",
			`Restore the command line's persistent target through the ${this.route} route.`,
		);
		if (this.route === "api") {
			assertAccepted(await this.api.sendCommandKey("ESC"), "ESC");
		} else {
			await this.desk.click(keypadLocator(this.browser(), "ESC"));
		}
		await this.expect(/^(?:FIXTURE|GROUP)$/);
	}

	async expect(expected: CommandExpectation): Promise<void> {
		if (this.route === "ui") {
			await expect(this.commandLine()).toHaveValue(expected);
			return;
		}
		const projection = expect.poll(
			async () => (await this.api.getCommandLine()).commandLine.text,
		);
		if (typeof expected === "string") await projection.toBe(expected);
		else await projection.toMatch(expected);
	}

	private async replace(text: string): Promise<void> {
		if (this.route === "api") {
			await this.api.setCommandLineText(text);
			await this.expect(text);
		} else {
			const page = this.browser();
			const groupKey = keypadLocator(page, "GRP");
			if (!(await groupKey.isVisible())) {
				await this.desk.click(page.locator(".mode-toggle"));
				await expect(groupKey).toBeVisible();
			}
			await this.desk.click(keypadLocator(page, "ESC"));
			const target = await this.commandLine().inputValue();
			for (const key of commandKeys(text, target))
				await this.desk.click(keypadLocator(page, key));
		}
	}

	private commandLine(): Locator {
		return this.browser().getByRole("textbox", {
			name: "Command line",
			exact: true,
		});
	}

	private browser(): Page {
		if (!this.page)
			throw new Error("The visible command-line route requires a browser page");
		return this.page;
	}
}

function keypadLocator(page: Page, key: KeypadKey | SoftwareKey): Locator {
	if (key === "REC")
		return page.locator(".global-store-button:visible").first();
	if (key === "ESC") return page.locator(".command-escape:visible").first();
	if (key === "PRE") return page.locator(".preload-button:visible").first();
	return page
		.locator(`.programmer-number-block [data-keypad-key="${key}"]:visible`)
		.first();
}

function requiredCommand(text: string): string {
	if (!text.trim())
		throw new Error("Command text must not be empty; use command.clear()");
	return text;
}

const TOKEN_ALIASES: Readonly<Record<string, readonly SoftwareKey[]>> = {
	GROUP: ["GRP"],
	THRU: ["TRU"],
	RECORD: ["REC"],
	DELETE: ["DEL"],
	MOVE: ["MOV"],
	COPY: ["CPY"],
	UNDO: ["UND"],
	PRELOAD: ["PRE"],
	DEGRP: ["GRP", "GRP"],
	DEGROUP: ["GRP", "GRP"],
};

const NAMED_COMMAND_KEYS = new Set<SoftwareKey>([
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
	"SHIFT",
	"TIME",
	"SELECT",
	"+",
	"-",
	".",
]);

/** Converts complete operator command text into the exact desk keys used by the visible UI. */
export function commandKeys(
	command: string,
	currentTarget: string,
): SoftwareKey[] {
	const tokens = requiredCommand(command).trim().split(/\s+/);
	const target = /GROUP/i.test(currentTarget) ? "GROUP" : "FIXTURE";
	const keys: SoftwareKey[] = [];
	const requestedTarget = tokens[0]?.toUpperCase();
	if (requestedTarget === "FIXTURE" || requestedTarget === "GROUP") {
		if (requestedTarget !== target) keys.push("GRP");
		tokens.shift();
	}
	for (let index = 0; index < tokens.length; index += 1) {
		const token = tokens[index];
		if (!token) continue;
		if (
			token.toUpperCase() === "SPD" &&
			tokens[index + 1]?.toUpperCase() === "GRP"
		) {
			keys.push("SHIFT", "TIME");
			index += 1;
			continue;
		}
		keys.push(...keysForToken(token));
	}
	return keys;
}

function keysForToken(token: string): SoftwareKey[] {
	const normalized = token.toUpperCase();
	const alias = TOKEN_ALIASES[normalized];
	if (alias) return [...alias];
	if (/^\d+(?:[.,]\d+)?$/.test(normalized))
		return [...normalized.replace(",", ".")] as SoftwareKey[];
	if (NAMED_COMMAND_KEYS.has(normalized as SoftwareKey))
		return [normalized as SoftwareKey];
	throw new Error(`Unsupported command token "${token}"`);
}

function assertAccepted(
	response: CommandOperationResponse,
	key: SoftwareKey,
): void {
	if (response.outcome === "rejected")
		throw new Error(`Command key ${key} was rejected: ${response.error}`);
}
