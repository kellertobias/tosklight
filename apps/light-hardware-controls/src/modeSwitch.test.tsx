// @vitest-environment jsdom

import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { hardwareSettingsKey } from "./controller/settings";
import type { DeviceStatus, FeedbackMessage } from "./controller/types";
import { nativeStatusIntervalMs } from "./controller/useHardwareController";
import type { NativeHardwareBridge } from "./transport/nativeBridge";
import type { OscBridge } from "./transport/oscBridge";

vi.mock("@tauri-apps/api/core", () => ({
	invoke: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("@tauri-apps/api/event", () => ({
	listen: vi.fn().mockResolvedValue(() => undefined),
}));

let saved: string | null = null;
const storage = {
	getItem: vi.fn((key: string) => (key === hardwareSettingsKey ? saved : null)),
	setItem: vi.fn((key: string, value: string) => {
		if (key === hardwareSettingsKey) saved = value;
	}),
	removeItem: vi.fn(),
	clear: vi.fn(),
	key: vi.fn().mockReturnValue(null),
	length: 0,
};

let order: string[] = [];

beforeEach(() => {
	saved = null;
	order = [];
	vi.stubGlobal("localStorage", storage);
});

afterEach(() => {
	cleanup();
	vi.useRealTimers();
	vi.unstubAllGlobals();
	vi.clearAllMocks();
});

function stubOsc() {
	let feedback: ((message: FeedbackMessage) => void) | undefined;
	const bridge = {
		connect: vi.fn(async () => {
			order.push("osc:connect");
		}),
		disconnect: vi.fn(async () => {
			order.push("osc:disconnect");
		}),
		send: vi.fn().mockResolvedValue(undefined),
		listenFeedback: vi.fn(async (listener) => {
			feedback = listener;
			return () => undefined;
		}),
	} satisfies OscBridge;
	return {
		bridge,
		feedback: (message: FeedbackMessage) => act(() => feedback?.(message)),
	};
}

function stubNative(initial: DeviceStatus) {
	let current = initial;
	const bridge = {
		open: vi.fn(async () => {
			order.push("native:open");
		}),
		status: vi.fn(async () => current),
		close: vi.fn(async () => {
			order.push("native:close");
		}),
	} satisfies NativeHardwareBridge;
	return {
		bridge,
		report: (next: DeviceStatus) => {
			current = next;
		},
	};
}

const linkStatus = () =>
	screen.getByRole("status", { name: "Link status" }).textContent ?? "";
const modeButton = (name: string) => screen.getByRole("button", { name });
const pressed = (name: string) => modeButton(name).getAttribute("aria-pressed");

describe("hardware test application input modes", () => {
	it("starts in OSC mode and reports desk connection and input", async () => {
		const osc = stubOsc();
		const native = stubNative({ state: "connected", name: "Wing" });
		render(<App bridge={osc.bridge} nativeBridge={native.bridge} />);

		await waitFor(() => expect(osc.bridge.connect).toHaveBeenCalledTimes(1));
		expect(pressed("OSC")).toBe("true");
		expect(pressed("Native Hardware")).toBe("false");
		expect(native.bridge.open).not.toHaveBeenCalled();
		expect(linkStatus()).toBe("OSC○ Connecting to desk…No OSC input yet");

		osc.feedback({ address: "/light/main/feedback/page", arguments: [3] });
		fireEvent.click(screen.getByRole("button", { name: "Encoder 2 up" }));
		const sentPath = osc.bridge.send.mock.calls[0][0];
		expect(linkStatus()).toBe(
			`OSC● Desk connected · page 3Last OSC input: ${sentPath}`,
		);
	});

	it("switches to Native Hardware after tearing down the OSC link and reports the device", async () => {
		const osc = stubOsc();
		const native = stubNative({
			state: "connected",
			name: "Test Wing",
			message: null,
		});
		render(<App bridge={osc.bridge} nativeBridge={native.bridge} />);
		await waitFor(() => expect(osc.bridge.connect).toHaveBeenCalledTimes(1));
		order = [];

		fireEvent.click(modeButton("Native Hardware"));
		await waitFor(() =>
			expect(linkStatus()).toContain("Device connected · Test Wing"),
		);
		expect(order).toEqual([
			"native:close",
			"osc:disconnect",
			"osc:connect",
			"native:open",
		]);
		expect(native.bridge.open).toHaveBeenCalledWith(
			expect.objectContaining({ host: "127.0.0.1", serverPort: 5000 }),
		);
		expect(pressed("Native Hardware")).toBe("true");
		expect(pressed("OSC")).toBe("false");
		expect(linkStatus()).toContain("Native Hardware");
		expect(JSON.parse(saved ?? "{}")).toMatchObject({ mode: "native" });

		// The device is the only input path: on-screen presses are not sent twice.
		fireEvent.click(screen.getByRole("button", { name: "Encoder 2 up" }));
		expect(osc.bridge.send).not.toHaveBeenCalled();
		expect(linkStatus()).toContain(
			"Input from the attached device · on-screen controls mirror only",
		);
	});

	it("keeps reporting device availability and errors while Native Hardware is active", async () => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		saved = JSON.stringify({ mode: "native", serverPort: 5010 });
		const osc = stubOsc();
		const native = stubNative({
			state: "unavailable",
			message: "no native hardware extension is configured on this desk",
		});
		render(<App bridge={osc.bridge} nativeBridge={native.bridge} />);
		await waitFor(() =>
			expect(linkStatus()).toContain(
				"No device available · no native hardware extension is configured on this desk",
			),
		);
		expect(native.bridge.open).toHaveBeenCalledWith(
			expect.objectContaining({ serverPort: 5010 }),
		);

		native.report({
			state: "error",
			name: "Wing",
			message: "Wing: device unplugged",
		});
		await act(() => vi.advanceTimersByTimeAsync(nativeStatusIntervalMs));
		expect(linkStatus()).toContain(
			"Device error · Wing · Wing: device unplugged",
		);
	});

	it("reports an unreachable desk instead of a silent native mode", async () => {
		saved = JSON.stringify({ mode: "native" });
		const osc = stubOsc();
		const native = stubNative({ state: "connected" });
		native.bridge.open.mockRejectedValueOnce(new Error("Load failed"));
		render(<App bridge={osc.bridge} nativeBridge={native.bridge} />);
		await waitFor(() =>
			expect(linkStatus()).toContain(
				"Device error · cannot reach the desk at 127.0.0.1:5000: Load failed",
			),
		);
		expect(native.bridge.status).not.toHaveBeenCalled();
	});

	it("reports a failed OSC link in either mode instead of connecting forever", async () => {
		const osc = stubOsc();
		osc.bridge.connect.mockRejectedValueOnce(
			new Error("invalid socket address"),
		);
		const native = stubNative({ state: "connected" });
		render(<App bridge={osc.bridge} nativeBridge={native.bridge} />);
		await waitFor(() =>
			expect(linkStatus()).toContain(
				"✕ cannot open the OSC link to 127.0.0.1:9000: invalid socket address",
			),
		);

		osc.bridge.connect.mockRejectedValueOnce(
			new Error("invalid socket address"),
		);
		fireEvent.click(modeButton("Native Hardware"));
		await waitFor(() =>
			expect(linkStatus()).toContain(
				"Device error · no desk link✕ cannot open the OSC link to 127.0.0.1:9000",
			),
		);
		expect(native.bridge.open).not.toHaveBeenCalled();
		expect(JSON.parse(saved ?? "{}")).toMatchObject({ mode: "native" });
	});

	it("switching back to OSC closes the native session and stops status polling", async () => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		saved = JSON.stringify({ mode: "native" });
		const osc = stubOsc();
		const native = stubNative({ state: "connected", name: "Wing" });
		render(<App bridge={osc.bridge} nativeBridge={native.bridge} />);
		await waitFor(() =>
			expect(linkStatus()).toContain("Device connected · Wing"),
		);
		order = [];

		fireEvent.click(modeButton("OSC"));
		await waitFor(() => expect(osc.bridge.connect).toHaveBeenCalledTimes(2));
		expect(order).toEqual(["native:close", "osc:disconnect", "osc:connect"]);
		const polls = native.bridge.status.mock.calls.length;
		await act(() => vi.advanceTimersByTimeAsync(nativeStatusIntervalMs * 3));
		expect(native.bridge.status).toHaveBeenCalledTimes(polls);
		expect(native.bridge.open).toHaveBeenCalledTimes(1);

		expect(linkStatus()).not.toContain("Device");
		expect(pressed("OSC")).toBe("true");
		expect(pressed("Native Hardware")).toBe("false");
		expect(JSON.parse(saved ?? "{}")).toMatchObject({ mode: "osc" });
		fireEvent.click(screen.getByRole("button", { name: "Encoder 2 up" }));
		expect(osc.bridge.send).toHaveBeenCalledTimes(1);
	});

	it("drops a status that arrives after the mode changed", async () => {
		saved = JSON.stringify({ mode: "native" });
		const osc = stubOsc();
		const native = stubNative({ state: "connected", name: "Late" });
		let release: (() => void) | undefined;
		native.bridge.status.mockImplementationOnce(
			() =>
				new Promise<DeviceStatus>((resolve) => {
					release = () => resolve({ state: "connected", name: "Late" });
				}),
		);
		render(<App bridge={osc.bridge} nativeBridge={native.bridge} />);
		await waitFor(() => expect(release).toBeDefined());
		fireEvent.click(modeButton("OSC"));
		await waitFor(() => expect(osc.bridge.connect).toHaveBeenCalledTimes(2));
		await act(async () => release?.());
		expect(linkStatus()).not.toContain("Late");
		expect(linkStatus()).toContain("OSC");
	});

	it("offers the desk HTTP port in Settings and names the active mode", async () => {
		const osc = stubOsc();
		const native = stubNative({ state: "connected" });
		render(<App bridge={osc.bridge} nativeBridge={native.bridge} />);
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		expect(screen.getByText(/^OSC mode · /)).toBeTruthy();
		fireEvent.change(screen.getByLabelText("Desk HTTP port"), {
			target: { value: "5050" },
		});
		fireEvent.click(modeButton("Native Hardware"));
		await waitFor(() =>
			expect(native.bridge.open).toHaveBeenCalledWith(
				expect.objectContaining({ serverPort: 5050 }),
			),
		);
		expect(screen.getByText(/^Native Hardware mode · /)).toBeTruthy();
	});
});
