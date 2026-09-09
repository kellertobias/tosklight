import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ControlDesk } from "../../api/types";
import { HardwareLightingSettings } from "./HardwareLightingSettings";

const mock = vi.hoisted(() => ({ connected: false, sessionId: "session-1", desk: null as ControlDesk | null, save: vi.fn() }));
vi.mock("../../features/deskSnapshot/DeskSnapshotState", () => ({ useHardwareConnected: () => mock.connected }));
vi.mock("../../features/screens/ScreensContext", () => ({ useScreens: () => ({ session: mock.desk ? { session_id: mock.sessionId, desk: mock.desk } : null, updateControlDesk: mock.save }) }));
beforeEach(() => {
	mock.connected = true;
	mock.sessionId = "session-1";
	mock.desk = { id: "desk-1", name: "Main", columns: 10, rows: 1, buttons: 3, hardware_led_brightness: 80, hardware_gooseneck_brightness: 60, hardware_gooseneck_color: 100 };
	mock.save.mockReset().mockResolvedValue(undefined);
});
afterEach(cleanup);

function move(label: string, value: number) {
	const slider = screen.getByRole("slider", { name: label });
	fireEvent.pointerDown(slider);
	fireEvent.input(slider, { target: { value: String(value) } });
	fireEvent.pointerUp(slider);
	return slider;
}
function deferred() {
	let resolve!: () => void;
	const promise = new Promise<void>((done) => { resolve = done; });
	return { promise, resolve };
}

describe("Hardware lighting settings", () => {
	it("only exposes faders while hardware and a desk session are connected", () => {
		mock.connected = false;
		const view = render(<HardwareLightingSettings />);
		expect(screen.queryByRole("heading", { name: "Hardware lighting" })).not.toBeInTheDocument();
		mock.connected = true;
		view.rerender(<HardwareLightingSettings />);
		expect(screen.getByRole("slider", { name: "LED brightness (%)" })).toHaveValue("80");
		expect(screen.queryByRole("button", { name: /apply/i })).not.toBeInTheDocument();
		mock.connected = false;
		view.rerender(<HardwareLightingSettings />);
		expect(screen.queryByRole("slider")).not.toBeInTheDocument();
		mock.connected = true;
		mock.desk = null;
		view.rerender(<HardwareLightingSettings />);
		expect(screen.queryByRole("heading", { name: "Hardware lighting" })).not.toBeInTheDocument();
	});
	it("saves during a drag and flushes the final position without an Apply button", async () => {
		render(<HardwareLightingSettings />);
		const slider = screen.getByRole("slider", { name: "LED brightness (%)" });
		fireEvent.pointerDown(slider);
		fireEvent.input(slider, { target: { value: "45" } });
		expect(mock.save).toHaveBeenCalledWith(mock.desk, { throwOnError: true, hardwareLighting: { hardware_led_brightness: 45 } });
		fireEvent.input(slider, { target: { value: "0" } });
		fireEvent.pointerUp(slider);
		await waitFor(() => expect(mock.save).toHaveBeenLastCalledWith(mock.desk, { throwOnError: true, hardwareLighting: { hardware_led_brightness: 0 } }));
		expect(slider).toHaveValue("0");
	});
	it("coalesces outstanding edits and ignores older snapshots without disabling faders", async () => {
		const first = deferred();
		mock.save.mockReturnValueOnce(first.promise);
		const view = render(<HardwareLightingSettings />);
		move("LED brightness (%)", 70);
		move("LED brightness (%)", 40);
		move("LED brightness (%)", 20);
		move("Gooseneck brightness (%)", 35);
		move("Gooseneck color (white %)", 0);
		expect(mock.save).toHaveBeenCalledTimes(1);
		mock.desk = { ...mock.desk!, hardware_led_brightness: 70 };
		view.rerender(<HardwareLightingSettings />);
		expect(screen.getByRole("slider", { name: "LED brightness (%)" })).toHaveValue("20");
		expect(screen.getByRole("slider", { name: "LED brightness (%)" })).not.toBeDisabled();
		await act(async () => first.resolve());
		expect(mock.save).toHaveBeenCalledTimes(2);
		expect(mock.save).toHaveBeenLastCalledWith(mock.desk, { throwOnError: true, hardwareLighting: { hardware_led_brightness: 20, hardware_gooseneck_brightness: 35, hardware_gooseneck_color: 0 } });
		mock.desk = { ...mock.desk!, hardware_led_brightness: 20, hardware_gooseneck_brightness: 35, hardware_gooseneck_color: 0 };
		view.rerender(<HardwareLightingSettings />);
		mock.desk = { ...mock.desk!, hardware_led_brightness: 25 };
		view.rerender(<HardwareLightingSettings />);
		expect(screen.getByRole("slider", { name: "LED brightness (%)" })).toHaveValue("25");
	});
	it.each(["disconnect", "session"])("drops queued changes after %s without replaying them", async (kind) => {
		const first = deferred();
		mock.save.mockReturnValueOnce(first.promise);
		const view = render(<HardwareLightingSettings />);
		move("LED brightness (%)", 70);
		move("LED brightness (%)", 20);
		if (kind === "disconnect") mock.connected = false;
		else mock.sessionId = "session-2";
		view.rerender(<HardwareLightingSettings />);
		await act(async () => first.resolve());
		mock.connected = true;
		view.rerender(<HardwareLightingSettings />);
		expect(mock.save).toHaveBeenCalledTimes(1);
		expect(screen.getByRole("slider", { name: "LED brightness (%)" })).toHaveValue("80");
	});
	it("shows failed saves and permits another adjustment", async () => {
		mock.save.mockRejectedValueOnce(new Error("The desk could not save hardware lighting."));
		render(<HardwareLightingSettings />);
		move("LED brightness (%)", 50);
		expect(await screen.findByRole("alert")).toHaveTextContent("The desk could not save hardware lighting.");
		expect(screen.getByRole("slider", { name: "LED brightness (%)" })).toHaveValue("50");
		move("LED brightness (%)", 51);
		await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
		expect(mock.save).toHaveBeenCalledTimes(2);
	});
});
