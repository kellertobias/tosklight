import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DeskConfiguration } from "../../api/types";
import type { SetupWindowController } from "./controller";
import { TimecodeSection } from "./GeneralSections";
import { OutputsSection } from "./OutputsSection";

vi.mock("../../features/dmxDiagnostics/DmxDiagnosticsContext", () => ({
	useDmxDiagnostics: () => null,
}));

vi.mock("../../components/setup/UsbDmxEndpointsSetup", () => ({
	useUsbDmxDiscovery: () => ({
		snapshot: null,
		devices: [],
		busy: false,
		error: null,
		scan: vi.fn(),
		provision: vi.fn(),
	}),
}));

vi.mock("../../components/setup/OutputRoutesSetup", () => ({
	OutputRoutesSetup: () => <div>Output routes</div>,
}));

vi.mock("../../features/timecode/TimecodeActionsContext", () => ({
	useTimecodeActions: () => ({
		api: {
			outputDevices: async () => ({ devices: ["Built-in Output"] }),
			internalAudioStatus: async () => ({ players: [], libraries: [] }),
		},
	}),
}));

afterEach(cleanup);

function setupController() {
	const draft = {
		frame_rate_hz: 44,
		output_bind_ip: "0.0.0.0",
		backup_retention: 20,
		timecode_source: { type: "internal" },
		timecode_frame_rate: null,
		art_timecode_bind: null,
		timecode_external_loss_policy: "continue_internal",
		timecode_external_loss_timeout_millis: 1000,
		timecode_audio_output_device: "Built-in Output",
		timecode_audio_latency_trim_micros_by_output: {
			"Built-in Output": 1250,
		},
		internal_audio_library_roots: { show: "/Volumes/Show/Audio" },
		internal_audio_output_devices: { main: "Built-in Output" },
	} as unknown as DeskConfiguration;
	const editDraft = vi.fn();
	return {
		draft,
		editDraft,
		controller: { draft, editDraft } as unknown as SetupWindowController,
	};
}

describe("Outputs audio configuration", () => {
	it("uses exactly the Output Engine, Routes, and Audio Output tabs", () => {
		const { controller } = setupController();
		render(<OutputsSection controller={controller} />);
		expect(
			screen.getAllByRole("tab").map((tab) => tab.textContent),
		).toEqual(["Output Engine", "Routes", "Audio Output"]);
		fireEvent.click(screen.getByRole("tab", { name: "Routes" }));
		expect(screen.getByText("Output routes")).toBeVisible();
	});

	it("shows and updates the preserved binding fields only under Audio Output", async () => {
		const { controller, draft, editDraft } = setupController();
		const view = render(<OutputsSection controller={controller} />);
		expect(screen.queryByLabelText("Audio library bindings")).toBeNull();

		fireEvent.click(screen.getByRole("tab", { name: "Audio Output" }));
		await waitFor(() =>
			expect(screen.getByLabelText("Audio library bindings")).toHaveValue(
				"show = /Volumes/Show/Audio",
			),
		);
		expect(screen.getByLabelText("Audio output bindings")).toHaveValue(
			"main = Built-in Output",
		);
		expect(screen.getByLabelText("Audio latency trim")).toHaveValue("1250");

		const libraries = screen.getByLabelText("Audio library bindings");
		fireEvent.change(libraries, {
			target: { value: "show = /Volumes/New Audio" },
		});
		fireEvent.blur(libraries);
		expect(editDraft).toHaveBeenCalledWith({
			...draft,
			internal_audio_library_roots: { show: "/Volumes/New Audio" },
		});

		view.unmount();
		render(<TimecodeSection controller={controller} />);
		expect(screen.queryByLabelText("Audio library bindings")).toBeNull();
		expect(screen.queryByLabelText("Audio output bindings")).toBeNull();
		expect(screen.queryByLabelText("Timecode audio output")).toBeNull();
		expect(screen.getByText("External source loss")).toBeVisible();
	});
});
