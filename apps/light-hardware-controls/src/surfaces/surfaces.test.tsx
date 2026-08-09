// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import { initialFeedbackState } from "../controller/types";
import { GridSurface } from "./GridSurface";
import { PlaybackSurface } from "./PlaybackSurface";
import { ProgrammerSurface } from "./ProgrammerSurface";
import { NavigationRail } from "./playback/NavigationRail";
import { SettingsSurface } from "./SettingsSurface";

const send = () => undefined;

afterEach(cleanup);

describe("hardware controller surfaces", () => {
	it("keeps the playback and programmer console labels", () => {
		const playback = renderToStaticMarkup(
			<PlaybackSurface topRowVisible levels={{}} lamps={{}} send={send} />,
		);
		const programmer = renderToStaticMarkup(
			<ProgrammerSurface
				updateArmed
				lamps={{}}
				highlight={initialFeedbackState.highlight}
				send={send}
			/>,
		);

		expect(playback).toContain("Encoder 1 up");
		expect(playback).toContain(">21<");
		expect(playback).toContain("FADER");
		expect(programmer).toContain("UPDATE");
		expect(programmer).toContain("PRELOAD GO");
		expect(programmer).toContain("HIGH");
		expect(programmer).toContain("Prog Fade");
		expect(programmer).toContain("Cue Fade");
	});

	it("keeps the expanded grid and speed-group surfaces", () => {
		const markup = renderToStaticMarkup(
			<GridSurface levels={{}} lamps={{}} speedBpms={{}} send={send} />,
		);

		expect(markup).toContain(">41<");
		expect(markup).toContain(">90<");
		expect(markup).toContain("Playbacks 91–96");
		expect(markup).toContain("Speed groups");
		expect(markup).toContain("120 BPM");
	});

	it("keeps the OSC settings wording and reconnect action", () => {
		const markup = renderToStaticMarkup(
			<SettingsSurface
				connected
				settings={{ host: "light.local", port: 9000, desk: "wing", top: true }}
				updateSettings={() => undefined}
				connect={async () => undefined}
			/>,
		);

		expect(markup).toContain("OSC connection");
		expect(markup).toContain("Save and reconnect");
		expect(markup).toContain("Connected to wing on light.local:9000");
	});

	it("keeps hardware PREV NEXT and ALL active-only while HIGH remains available", () => {
		HTMLElement.prototype.setPointerCapture = vi.fn();
		const sendControl = vi.fn();
		const { rerender } = render(
			<ProgrammerSurface
				updateArmed={false}
				lamps={{}}
				highlight={{ active: false, canNext: true, canPrevious: true }}
				send={sendControl}
			/>,
		);

		expect(
			(screen.getByRole("button", { name: "HIGH" }) as HTMLButtonElement)
				.disabled,
		).toBe(false);
		for (const name of ["PREV", "NEXT", "ALL"]) {
			const button = screen.getByRole("button", { name }) as HTMLButtonElement;
			expect(button.disabled).toBe(true);
			fireEvent.pointerDown(button, { pointerId: 1 });
		}
		expect(sendControl).not.toHaveBeenCalled();

		rerender(
			<ProgrammerSurface
				updateArmed={false}
				lamps={{}}
				highlight={{ active: true, canNext: true, canPrevious: true }}
				send={sendControl}
			/>,
		);
		const next = screen.getByRole("button", { name: "NEXT" });
		fireEvent.pointerDown(next, { pointerId: 2 });
		fireEvent.pointerUp(next, { pointerId: 2 });
		expect(sendControl.mock.calls).toEqual([
			["highlight/next", [true]],
			["highlight/next", [false]],
		]);
	});

	it("sends attached ALIGN as one canonical press/release gesture", () => {
		HTMLElement.prototype.setPointerCapture = vi.fn();
		const sendControl = vi.fn();
		render(<NavigationRail page={1} send={sendControl} />);
		const align = screen.getByRole("button", { name: "ALIGN" });

		fireEvent.pointerDown(align, { pointerId: 3 });
		fireEvent.pointerUp(align, { pointerId: 3 });

		expect(sendControl).toHaveBeenCalledTimes(2);
		expect(sendControl.mock.calls[0]?.[0]).toBe("programmer/align");
		expect(sendControl.mock.calls[0]?.[1]?.[0]).toBe(true);
		expect(sendControl.mock.calls[1]?.[0]).toBe("programmer/align");
		expect(sendControl.mock.calls[1]?.[1]?.[0]).toBe(false);
		expect(sendControl.mock.calls[1]?.[1]?.[1]).toBe(
			sendControl.mock.calls[0]?.[1]?.[1],
		);
	});
});
