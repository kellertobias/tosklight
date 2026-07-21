import type { Page } from "@playwright/test";
import {
	type ControllableHostedPickerOutcome,
	decodeHostedPickerOutcome,
	HOSTED_PICKER_TEST_CONTROL,
} from "../../src/windows/fileManagerWindow/controllableHostedPicker";
import type { FileManagerPickerRequest } from "../../src/windows/fileManagerWindow/hostedPickerContract";

export class ControllableHostedFilePickerDriver {
	private installed = false;
	private disposed = false;
	private pending = 0;

	constructor(private readonly page: Page) {}

	get pendingRequests(): number {
		return this.pending;
	}

	async install(): Promise<void> {
		if (this.disposed)
			throw new Error("Controllable hosted-picker driver was disposed");
		if (this.installed) return;
		this.installed = true;
		await this.page.addInitScript((controlName) => {
			let handler: ((request: unknown) => unknown) | null = null;
			let disposed = false;
			let generation = 0;
			const active = new Set<{ cancel: () => void }>();
			const port = {
				attach(next: (request: unknown) => unknown) {
					if (disposed)
						throw new Error("Controllable hosted-picker port was disposed");
					if (handler)
						throw new Error("Controllable hosted-picker host already attached");
					if (typeof next !== "function")
						throw new Error("Invalid controllable hosted-picker handler");
					const attachment = ++generation;
					handler = next;
					return () => {
						if (generation === attachment) handler = null;
					};
				},
				request(request: unknown) {
					if (disposed)
						return Promise.reject(
							new Error("Controllable hosted-picker port was disposed"),
						);
					if (!handler)
						return Promise.reject(
							new Error("Controllable hosted-picker host is not mounted"),
						);
					let operation: unknown;
					try {
						operation = handler(request);
					} catch (error) {
						return Promise.reject(error);
					}
					if (!operation || typeof operation !== "object")
						return Promise.reject(
							new Error("Invalid controllable hosted-picker operation"),
						);
					const candidate = operation as {
						outcome?: unknown;
						cancel?: unknown;
					};
					if (
						!(candidate.outcome instanceof Promise) ||
						typeof candidate.cancel !== "function"
					)
						return Promise.reject(
							new Error("Invalid controllable hosted-picker operation"),
						);
					const tracked = { cancel: candidate.cancel as () => void };
					active.add(tracked);
					return candidate.outcome.finally(() => active.delete(tracked));
				},
				dispose() {
					if (disposed) return;
					disposed = true;
					handler = null;
					for (const operation of active) operation.cancel();
					active.clear();
				},
			};
			Object.defineProperty(window, controlName, {
				configurable: true,
				value: port,
			});
		}, HOSTED_PICKER_TEST_CONTROL);
	}

	async open(
		request: FileManagerPickerRequest,
	): Promise<ControllableHostedPickerOutcome> {
		if (!this.installed)
			throw new Error("Controllable hosted-picker driver is not installed");
		if (this.disposed)
			throw new Error("Controllable hosted-picker driver was disposed");
		this.pending += 1;
		try {
			const outcome = await this.page.evaluate(
				({ controlName, request }) => {
					const control = (window as unknown as Record<string, unknown>)[
						controlName
					] as { request?: (value: unknown) => Promise<unknown> } | undefined;
					if (typeof control?.request !== "function")
						throw new Error("Controllable hosted-picker port is unavailable");
					return control.request(request);
				},
				{ controlName: HOSTED_PICKER_TEST_CONTROL, request },
			);
			return decodeHostedPickerOutcome(outcome);
		} finally {
			this.pending -= 1;
		}
	}

	async dispose(): Promise<void> {
		if (this.disposed) return;
		this.disposed = true;
		if (!this.installed || this.page.isClosed()) return;
		await this.page.evaluate((controlName) => {
			const control = (window as unknown as Record<string, unknown>)[
				controlName
			] as { dispose?: () => void } | undefined;
			control?.dispose?.();
		}, HOSTED_PICKER_TEST_CONTROL);
	}
}
