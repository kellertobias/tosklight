import { act, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ScreenPlaybackFit } from "./ScreenPlaybackFit";

function Bank({ surface }: { surface: string }) {
	return <div className="playback-fader-bank touch-layout" data-playback-surface={surface}
		style={{ gridTemplateRows: "500px", rowGap: 0, padding: 0 }}>
		<article style={{ padding: 2, border: "1px solid black" }} />
	</div>;
}
function Layout({ pane }: { pane: boolean }) {
	return <div className="app-shell"><section className="control-section">
		<ScreenPlaybackFit><Bank surface="main" /></ScreenPlaybackFit>
		{pane && <ScreenPlaybackFit className="screen-bank-fit"><Bank surface="pane:mirror" /></ScreenPlaybackFit>}
	</section></div>;
}

afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });
describe("Screen fit height ownership", () => {
	it("unmounting a mirrored pane retains the height measured by Main", () => {
		const frames = new Map<number, FrameRequestCallback>();
		let next = 0;
		vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
			frames.set(++next, callback);
			return next;
		});
		vi.stubGlobal("cancelAnimationFrame", (id: number) => frames.delete(id));
		vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
		vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
			x: 0, y: 0, left: 0, top: 0, right: 900, bottom: 500,
			width: 900, height: 500, toJSON: () => ({}),
		});
		document.documentElement.style.setProperty("--control-height", "348px");
		const mounted = render(<Layout pane />);
		act(() => { for (const callback of [...frames.values()]) callback(0); frames.clear(); });
		const shell = mounted.container.querySelector<HTMLElement>(".app-shell")!;
		const height = shell.style.getPropertyValue("--screen-playback-required-height");
		expect(height).toBe("71px");
		expect(shell.style.getPropertyValue("--screen-playback-base-height")).toBe("348px");
		expect(mounted.container.querySelector(".screen-playback-fit.screen-bank-fit")).not.toBeNull();
		mounted.rerender(<Layout pane={false} />);
		expect(shell.style.getPropertyValue("--screen-playback-required-height")).toBe(height);
		expect(shell.style.getPropertyValue("--screen-playback-base-height")).toBe("348px");
		mounted.unmount();
		expect(shell.style.getPropertyValue("--screen-playback-required-height")).toBe("");
		expect(shell.style.getPropertyValue("--screen-playback-base-height")).toBe("");
		document.documentElement.style.removeProperty("--control-height");
	});
});
