import { afterEach, describe, expect, it, vi } from "vitest";
import { ControllableHostedFilePickerDriver } from "../../e2e/bench/hostedFilePicker";
import { HOSTED_PICKER_TEST_CONTROL } from "../windows/fileManagerWindow/controllableHostedPicker";

afterEach(() => {
	delete (window as unknown as Record<string, unknown>)[
		HOSTED_PICKER_TEST_CONTROL
	];
});

describe("ControllableHostedFilePickerDriver", () => {
	it("installs only an init script and requires navigation before use", async () => {
		const page = fakePage();
		const driver = new ControllableHostedFilePickerDriver(page.value);

		await expect(driver.open({ target: "files" })).rejects.toThrow(
			"driver is not installed",
		);
		await driver.install();
		expect(page.addInitScript).toHaveBeenCalledOnce();
		expect(windowControl()).toBeUndefined();

		page.runInitScript();
		await expect(driver.open({ target: "files" })).rejects.toThrow(
			"host is not mounted",
		);
	});

	it("disposal cancels and settles the active request exactly once", async () => {
		const page = fakePage();
		const driver = new ControllableHostedFilePickerDriver(page.value);
		await driver.install();
		page.runInitScript();
		let settle!: (value: unknown) => void;
		const cancel = vi.fn(() => settle({ status: "cancelled" }));
		const control = requiredWindowControl();
		control.attach(() => ({
			outcome: new Promise((resolve) => {
				settle = resolve;
			}),
			cancel,
		}));

		const pending = driver.open({ target: "folders" });
		await Promise.resolve();
		expect(driver.pendingRequests).toBe(1);
		await driver.dispose();

		await expect(pending).resolves.toEqual({ status: "cancelled" });
		expect(cancel).toHaveBeenCalledOnce();
		expect(driver.pendingRequests).toBe(0);
		await expect(driver.open({ target: "files" })).rejects.toThrow(
			"driver was disposed",
		);
	});

	it("rejects malformed outcomes returned across the browser boundary", async () => {
		const page = fakePage();
		const driver = new ControllableHostedFilePickerDriver(page.value);
		await driver.install();
		page.runInitScript();
		const control = requiredWindowControl();
		control.attach(() => ({
			outcome: Promise.resolve({ status: "cancelled", stale: true }),
			cancel: vi.fn(),
		}));

		await expect(driver.open({ target: "either" })).rejects.toThrow(
			"Invalid cancelled hosted-picker outcome fields",
		);
	});
});

function fakePage() {
	let initScript: (() => void) | null = null;
	const addInitScript = vi.fn(
		async (script: (name: string) => void, name: string) => {
			initScript = () => script(name);
		},
	);
	const evaluate = vi.fn(
		async <T, A>(script: (argument: A) => T | Promise<T>, argument: A) =>
			script(argument),
	);
	return {
		addInitScript,
		runInitScript() {
			if (!initScript)
				throw new Error("No hosted-picker init script installed");
			initScript();
		},
		value: {
			addInitScript,
			evaluate,
			isClosed: () => false,
		} as never,
	};
}

interface BrowserHostedPickerControl {
	attach(handler: (request: unknown) => unknown): () => void;
	request(request: unknown): Promise<unknown>;
	dispose(): void;
}

function windowControl(): BrowserHostedPickerControl | undefined {
	return (window as unknown as Record<string, unknown>)[
		HOSTED_PICKER_TEST_CONTROL
	] as BrowserHostedPickerControl | undefined;
}

function requiredWindowControl(): BrowserHostedPickerControl {
	const control = windowControl();
	if (!control) throw new Error("Hosted-picker control was not installed");
	return control;
}
