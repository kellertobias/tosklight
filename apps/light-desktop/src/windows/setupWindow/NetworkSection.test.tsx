import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SetupWindowController } from "./controller";
import { NetworkSection } from "./NetworkSection";

vi.mock("../../api/client/serverLocation", () => ({
	configuredServerUrl: () => "http://127.0.0.1:5000",
}));
vi.mock("../../components/setup/SoundInputSettings", () => ({
	SoundInputSettings: () => <div>Sound settings</div>,
}));
vi.mock("../../components/setup/MatterBridgeSettings", () => ({
	MatterBridgeSettings: () => <div>Matter settings</div>,
}));

afterEach(cleanup);

describe("Network & Inputs settings", () => {
	it("renders the existing responsibilities as peer groups in operator order", () => {
		const controller = {
			draft: {
				midi_inputs: ["Desk MIDI"],
				osc_bind: "0.0.0.0:8000",
				rtp_midi_bind: null,
			},
			serverUrl: "http://127.0.0.1:5000",
			setServerUrl: vi.fn(),
			applyServerUrl: vi.fn(),
		} as unknown as SetupWindowController;
		render(<NetworkSection controller={controller} />);

		expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent(
			"Network & Inputs",
		);
		expect(
			screen
				.getAllByRole("heading", { level: 3 })
				.map((heading) => heading.textContent),
		).toEqual([
			"ToskLight server connection",
			"Control inputs",
			"Sound input",
			"Matter bridge",
		]);
		expect(screen.queryByRole("heading", { name: "Inputs" })).toBeNull();
		expect(screen.queryByRole("heading", { name: "Services" })).toBeNull();
	});
});
