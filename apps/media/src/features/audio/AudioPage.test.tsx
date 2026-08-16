import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { anAudioPanel, stubServer } from "../../testing/server";
import { AudioPage } from "./AudioPage";

// The page opens a telemetry socket. These tests are about what it draws and what it saves, so the
// socket never connects — which is itself the case worth covering: the snapshot has to be enough.
beforeEach(() => {
	vi.stubGlobal("WebSocket", undefined);
});

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe("the audio monitor", () => {
	it("draws what the server is hearing", async () => {
		stubServer();
		render(<AudioPage />);

		expect(await screen.findByRole("meter", { name: "Bass" })).toHaveAttribute(
			"aria-valuenow",
			"60",
		);
		expect(screen.getByRole("meter", { name: "Peak" })).toHaveAttribute(
			"aria-valuenow",
			"80",
		);
		expect(screen.getByRole("img", { name: "Waveform" })).toBeInTheDocument();
		expect(screen.getByText("128.0 BPM")).toBeInTheDocument();
	});

	it("says a machine with no input is not capturing rather than showing a dead meter", async () => {
		const panel = anAudioPanel();
		panel.analysis = {
			...panel.analysis,
			capturing: false,
			device: "none",
			detail: "this server is not capturing audio",
			waveform: { points: [] },
			spectrum: [],
			bands: { bass: 0, mid: 0, treble: 0 },
			energy: 0,
			peak: 0,
			beat: 0,
			bpm: 0,
		};
		stubServer({ audio: panel });
		render(<AudioPage />);

		expect(await screen.findByText("No input device")).toBeInTheDocument();
		expect(
			screen.getByText("this server is not capturing audio"),
		).toBeInTheDocument();
		expect(screen.getByText("not enough beats yet")).toBeInTheDocument();
	});

	it("reports that frames have stopped arriving instead of freezing quietly", async () => {
		stubServer();
		render(<AudioPage />);

		// The socket never connected in these tests, so a capturing device is not receiving.
		expect(await screen.findByText("Not receiving")).toBeInTheDocument();
	});

	it("persists a touch-adjusted gain live without an explicit submit", async () => {
		const server = stubServer();
		render(<AudioPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: "Change audio settings" }),
		);
		expect(document.querySelector(".ui-window-info-section")).toContainElement(
			screen.getByLabelText("Gain"),
		);
		const gain = screen.getByLabelText("Gain");
		fireEvent.pointerDown(gain, { pointerType: "touch" });
		fireEvent.input(gain, { target: { value: "2" } });
		fireEvent.pointerUp(gain, { pointerType: "touch" });

		await waitFor(() => expect(server.audio.settings.inputGain).toBe(2));
		expect(server.writes).toContain("/audio/update");
		expect(
			screen.queryByRole("button", { name: "Save" }),
		).not.toBeInTheDocument();
		expect(document.querySelector(".ui-window-info-section")).not.toBeNull();
	});

	it("uses touch faders for every live tuning value", async () => {
		const server = stubServer();
		render(<AudioPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: "Change audio settings" }),
		);

		const values = [
			["Gain", "1.25"],
			["Beat sensitivity", "2.5"],
			["Bass", "3.75"],
			["Mid", "4"],
			["Treble", "5.5"],
		] as const;
		for (const [label, value] of values) {
			const fader = screen.getByRole("slider", { name: label });
			fireEvent.pointerDown(fader, {
				pointerType: label === "Gain" ? "touch" : "mouse",
			});
			fireEvent.input(fader, { target: { value } });
			fireEvent.pointerUp(fader, {
				pointerType: label === "Gain" ? "touch" : "mouse",
			});
		}

		await waitFor(() =>
			expect(server.audio.settings).toMatchObject({
				inputGain: 1.25,
				beatSensitivity: 2.5,
				eqBass: 3.75,
				eqMid: 4,
				eqTreble: 5.5,
			}),
		);
		expect(server.writes).toContain("/audio/update");
	});

	it("offers the inputs this machine has, and says a device change waits for a start", async () => {
		stubServer();
		render(<AudioPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: "Change audio settings" }),
		);
		await userEvent.click(
			screen.getByRole("button", { name: "This machine's default input" }),
		);
		expect(
			screen.getByRole("option", { name: "Desk feed" }),
		).toBeInTheDocument();
		expect(
			screen.getByText(
				/Choosing a different input opens a\s+different stream/u,
			),
		).toBeInTheDocument();
	});

	it("says why a refused change was not applied", async () => {
		stubServer({
			refuseWrites: {
				code: "audio-invalid",
				message: "inputGain must be between 0 and 10",
				status: 400,
			},
		});
		render(<AudioPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: "Change audio settings" }),
		);
		fireEvent.input(screen.getByRole("slider", { name: "Gain" }), {
			target: { value: "2" },
		});

		expect(await screen.findByRole("alert")).toHaveTextContent(
			"inputGain must be between",
		);
	});
});
