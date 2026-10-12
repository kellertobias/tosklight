import { act, cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ScreenPlaybackFit } from "./ScreenPlaybackFit";

function Bank({ surface }: { surface: string }) {
	return (
		<div
			className="playback-fader-bank touch-layout"
			data-playback-surface={surface}
			style={{ gridTemplateRows: "500px", rowGap: 0, padding: 0 }}
		>
			<article style={{ padding: 2, border: "1px solid black" }} />
		</div>
	);
}
function Layout({ pane }: { pane: boolean }) {
	return (
		<div className="app-shell">
			<section className="control-section">
				<ScreenPlaybackFit>
					<Bank surface="main" />
				</ScreenPlaybackFit>
				{pane && (
					<ScreenPlaybackFit className="screen-bank-fit">
						<Bank surface="pane:mirror" />
					</ScreenPlaybackFit>
				)}
			</section>
		</div>
	);
}

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
	vi.unstubAllGlobals();
});
describe("Screen fit height ownership", () => {
	it("unmounting a mirrored pane retains the height measured by Main", () => {
		const frames = new Map<number, FrameRequestCallback>();
		let next = 0;
		vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
			frames.set(++next, callback);
			return next;
		});
		vi.stubGlobal("cancelAnimationFrame", (id: number) => frames.delete(id));
		vi.stubGlobal(
			"ResizeObserver",
			class {
				observe() {}
				disconnect() {}
			},
		);
		vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
			x: 0,
			y: 0,
			left: 0,
			top: 0,
			right: 900,
			bottom: 500,
			width: 900,
			height: 500,
			toJSON: () => ({}),
		});
		document.documentElement.style.setProperty("--control-height", "348px");
		const mounted = render(<Layout pane />);
		act(() => {
			for (const callback of [...frames.values()]) callback(0);
			frames.clear();
		});
		const shell = requireElement(mounted.container, ".app-shell");
		const height = shell.style.getPropertyValue(
			"--screen-playback-required-height",
		);
		expect(height).toBe("71px");
		expect(shell.style.getPropertyValue("--screen-playback-base-height")).toBe(
			"348px",
		);
		expect(
			mounted.container.querySelector(".screen-playback-fit.screen-bank-fit"),
		).not.toBeNull();
		mounted.rerender(<Layout pane={false} />);
		expect(
			shell.style.getPropertyValue("--screen-playback-required-height"),
		).toBe(height);
		expect(shell.style.getPropertyValue("--screen-playback-base-height")).toBe(
			"348px",
		);
		mounted.unmount();
		expect(
			shell.style.getPropertyValue("--screen-playback-required-height"),
		).toBe("");
		expect(shell.style.getPropertyValue("--screen-playback-base-height")).toBe(
			"",
		);
		document.documentElement.style.removeProperty("--control-height");
	});
});

function geometryHarness() {
	const frames = new Map<number, FrameRequestCallback>();
	let next = 0;
	vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
		frames.set(++next, callback);
		return next;
	});
	vi.stubGlobal("cancelAnimationFrame", (id: number) => frames.delete(id));
	const observed = new Set<Element>();
	let notify: () => void = () => {};
	vi.stubGlobal(
		"ResizeObserver",
		class {
			constructor(callback: () => void) {
				notify = callback;
			}
			observe(target: Element) {
				observed.add(target);
			}
			unobserve(target: Element) {
				observed.delete(target);
			}
			disconnect() {
				observed.clear();
			}
		},
	);
	const flush = () =>
		act(() => {
			const pending = [...frames.values()];
			frames.clear();
			for (const callback of pending) callback(0);
		});
	return {
		observed,
		notifyBankResize: (bank: Element) => {
			if (observed.has(bank)) notify();
			flush();
		},
		flush,
	};
}
function rectangle(height: number): DOMRect {
	return {
		x: 0,
		y: 0,
		left: 0,
		top: 0,
		right: 300,
		bottom: height,
		width: 300,
		height,
		toJSON: () => ({}),
	};
}

describe("Screen fit geometry", () => {
	it("validates the bank bounds after its required-height write changes layout", () => {
		const harness = geometryHarness();
		vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(
			function (this: HTMLElement) {
				const shell = this.closest<HTMLElement>(".app-shell");
				// The shell's height allocation reflows both the bank and its authored row.
				const height = shell?.style.getPropertyValue(
					"--screen-playback-required-height",
				)
					? 150
					: 50;
				return rectangle(
					this.matches(".playback-fader-bank, article") ? height : 200,
				);
			},
		);
		const mounted = render(<Layout pane={false} />);
		harness.flush();
		const bank = requireElement(mounted.container, ".playback-fader-bank");
		const host = requireElement(mounted.container, ".screen-playback-fit");
		expect(bank.getBoundingClientRect().height).toBe(150);
		expect(host.getBoundingClientRect().height).toBe(200);
		expect(bank.inert).toBe(false);
		expect(mounted.queryByRole("status")).toBeNull();
		expect(host.style.getPropertyValue("--screen-fit-height")).toBe("150px");
	});
	it("remeasures a bank resize even when its host retains the same size", () => {
		const harness = geometryHarness();
		let bankHeight = 80;
		vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(
			function (this: HTMLElement) {
				return rectangle(
					this.matches(".playback-fader-bank, article") ? bankHeight : 200,
				);
			},
		);
		const mounted = render(
			<ScreenPlaybackFit>
				<Bank surface="pane:mirror" />
			</ScreenPlaybackFit>,
		);
		harness.flush();
		const bank = requireElement(mounted.container, ".playback-fader-bank");
		expect(bank.inert).toBe(false);
		bankHeight = 40;
		harness.notifyBankResize(bank);
		expect(bank.inert).toBe(true);
		expect(mounted.getByRole("status").textContent).toContain(
			"44px touch controls",
		);
	});
	it("still blocks controls below 44px after the shell height allocation", () => {
		const harness = geometryHarness();
		vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
			rectangle(40),
		);
		const mounted = render(<Layout pane={false} />);
		harness.flush();
		const bank = requireElement(mounted.container, ".playback-fader-bank");
		expect(bank.inert).toBe(true);
		expect(mounted.getByRole("status").textContent).toContain(
			"44px touch controls",
		);
	});
});

function requireElement(container: HTMLElement, selector: string): HTMLElement {
	const element = container.querySelector<HTMLElement>(selector);
	if (!element) throw new Error(`Missing test element: ${selector}`);
	return element;
}
