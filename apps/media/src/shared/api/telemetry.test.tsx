import { render, screen, waitFor } from "@testing-library/react";
import { act } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { sendTelemetryAction, useTelemetry } from "./telemetry";

/// A socket a test drives, standing in for the browser's.
class FakeSocket {
	static opened: FakeSocket[] = [];
	static OPEN = 1;
	readyState = 1;
	sent: string[] = [];
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
		this.readyState = 3;
		this.closed = true;
		this.onclose?.();
	}
	send(value: string) { this.sent.push(value); }
}

function Meter() {
	const { frame, connected } = useTelemetry();
	return (
		<p>
			{connected ? "connected" : "not connected"} ·{" "}
			{frame
				? `${frame.audio.bpm} BPM${frame.deskIdentity ? ` · ${frame.deskIdentity.showName}` : ""}`
				: "no frame"}
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
				data: JSON.stringify({
					audio: { bpm: 128, capturing: true },
					deskIdentity: { showName: "The Tempest" },
				}),
			}),
		);

		await waitFor(() =>
			expect(
				screen.getByText(/connected · 128 BPM · The Tempest/u),
			).toBeInTheDocument(),
		);
	});

	it("reports a dropped socket rather than leaving a meter looking live", async () => {
		vi.stubGlobal("WebSocket", FakeSocket);
		render(<Meter />);
		const socket = FakeSocket.opened[0];
		act(() => socket.onopen?.());
		await waitFor(() =>
			expect(screen.getByText(/^connected/u)).toBeInTheDocument(),
		);

		act(() => socket.close());
		await waitFor(() =>
			expect(screen.getByText(/^not connected/u)).toBeInTheDocument(),
		);
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

	it("correlates one live action acknowledgement without replacing the telemetry frame", async () => {
		vi.stubGlobal("WebSocket", FakeSocket);
		render(<Meter />);
		const socket = FakeSocket.opened[0];
		act(() => socket.onopen?.());
		act(() => socket.onmessage?.({ data: JSON.stringify({ audio: { bpm: 128, capturing: true } }) }));
		const promise = sendTelemetryAction({ type: "automation-set-enabled", outputId: "main", enabled: true });
		const action = JSON.parse(socket.sent[0]);
		expect(action).toMatchObject({ type: "automation-set-enabled", outputId: "main", enabled: true, actionId: expect.any(String) });
		act(() => socket.onmessage?.({ data: JSON.stringify({ type: "automation-action-result", actionId: action.actionId, ok: true, message: null }) }));
		await expect(promise).resolves.toBeUndefined();
		expect(screen.getByText(/128 BPM/)).toBeVisible();
		expect(socket.sent).toHaveLength(1);
	});

	it("reports live action refusals instead of claiming they succeeded", async () => {
		vi.stubGlobal("WebSocket", FakeSocket);
		render(<Meter />);
		const socket = FakeSocket.opened[0];
		act(() => socket.onopen?.());
		const promise = sendTelemetryAction({ type: "automation-apply-now", outputId: "main" });
		const rejection = expect(promise).rejects.toThrow("DMX owns the output");
		const action = JSON.parse(socket.sent[0]);
		act(() => socket.onmessage?.({ data: JSON.stringify({ type: "automation-action-result", actionId: action.actionId, ok: false, message: "DMX owns the output" }) }));
		await rejection;
	});

	it("rejects disconnected actions and never queues them for a later connection", async () => {
		vi.stubGlobal("WebSocket", FakeSocket);
		render(<Meter />);
		const socket = FakeSocket.opened[0];
		await expect(sendTelemetryAction({ type: "automation-apply-now", outputId: "main" })).rejects.toThrow(/unavailable/);
		act(() => socket.onopen?.());
		expect(socket.sent).toEqual([]);
		const promise = sendTelemetryAction({ type: "automation-apply-now", outputId: "main" });
		const rejection = expect(promise).rejects.toThrow(/Check the current playback state/);
		act(() => socket.close());
		await rejection;
		expect(socket.sent).toHaveLength(1);
	});
});
