import { render, screen, waitFor } from "@testing-library/react";
import { act } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useTelemetry } from "./telemetry";

/// A socket a test drives, standing in for the browser's.
class FakeSocket {
	static opened: FakeSocket[] = [];
	readonly url: string;
	onopen: (() => void) | undefined;
	onclose: (() => void) | undefined;
	onerror: (() => void) | undefined;
	onmessage: ((event: { data: string }) => void) | undefined;
	closed = false;

	constructor(url: string) {
		this.url = url;
		FakeSocket.opened.push(this);
	}

	close() {
		this.closed = true;
		this.onclose?.();
	}
}

function Meter() {
	const { frame, connected } = useTelemetry();
	return (
		<p>
			{connected ? "connected" : "not connected"} ·{" "}
			{frame ? `${frame.audio.bpm} BPM` : "no frame"}
		</p>
	);
}

afterEach(() => {
	FakeSocket.opened = [];
	vi.unstubAllGlobals();
});

describe("pushed telemetry", () => {
	it("draws the frames the server pushes", async () => {
		vi.stubGlobal("WebSocket", FakeSocket);
		render(<Meter />);

		const socket = FakeSocket.opened[0];
		expect(socket.url).toMatch(/\/api\/v2\/telemetry$/u);

		act(() => socket.onopen?.());
		act(() =>
			socket.onmessage?.({
				data: JSON.stringify({ audio: { bpm: 128, capturing: true } }),
			}),
		);

		await waitFor(() => expect(screen.getByText(/connected · 128 BPM/u)).toBeInTheDocument());
	});

	it("reports a dropped socket rather than leaving a meter looking live", async () => {
		vi.stubGlobal("WebSocket", FakeSocket);
		render(<Meter />);
		const socket = FakeSocket.opened[0];
		act(() => socket.onopen?.());
		await waitFor(() => expect(screen.getByText(/^connected/u)).toBeInTheDocument());

		act(() => socket.close());
		await waitFor(() => expect(screen.getByText(/^not connected/u)).toBeInTheDocument());
	});

	it("survives a frame it cannot read", async () => {
		vi.stubGlobal("WebSocket", FakeSocket);
		render(<Meter />);
		const socket = FakeSocket.opened[0];
		act(() => socket.onopen?.());
		act(() => socket.onmessage?.({ data: "not json" }));

		// Still connected, still no frame — the socket was not torn down over one bad message.
		expect(screen.getByText(/connected · no frame/u)).toBeInTheDocument();
		expect(socket.closed).toBe(false);
	});

	it("does nothing at all where there is no WebSocket", () => {
		vi.stubGlobal("WebSocket", undefined);
		render(<Meter />);

		expect(screen.getByText(/not connected · no frame/u)).toBeInTheDocument();
	});
});
