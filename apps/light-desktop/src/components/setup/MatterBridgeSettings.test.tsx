import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MatterBridgeSettings } from "./MatterBridgeSettings";

const mocks = vi.hoisted(() => ({
	enabled: true,
	matter: null as null | Record<string, unknown>,
	saveConfiguration: vi.fn(),
}));

vi.mock("../../features/configuration/ConfigurationActionsProvider", () => ({
	useConfigurationActions: () => ({
		saveConfiguration: mocks.saveConfiguration,
	}),
}));
vi.mock("../../features/configuration/ConfigurationState", () => ({
	useDeskConfiguration: () => ({ matter_enabled: mocks.enabled }),
	useMatterEnabled: () => mocks.enabled,
}));
vi.mock("../../features/mediaServers/MediaServersContext", () => ({
	useMediaServers: () => ({ matter: mocks.matter }),
}));

beforeEach(() => {
	mocks.enabled = true;
	mocks.matter = {
		enabled: true,
		transport: "running",
		commissionable: false,
		commissioned: false,
		lights: [
			{
				endpoint_id: 1,
				page: 1,
				playback: 1,
				playback_number: 10,
				name: "Page 1 Playback 1: Front group",
				on: true,
				level: 127,
				kind: "color",
				color_active: true,
			},
			{
				endpoint_id: 2,
				page: 1,
				playback: 2,
				playback_number: 20,
				name: "Page 1 Playback 2: Main cuelist",
				on: true,
				level: 254,
				kind: "dimmable",
				color_active: false,
			},
		],
	};
	vi.clearAllMocks();
});

afterEach(cleanup);

describe("MatterBridgeSettings", () => {
	it("lists every exposed playback with its truthful Matter kind", () => {
		render(<MatterBridgeSettings />);

		expect(
			screen.getByText("2 assigned playbacks exposed: 1 dimmable, 1 color."),
		).toBeVisible();
		const list = screen.getByRole("list", { name: "Exposed Matter playbacks" });
		expect(list).toHaveTextContent("Page 1 Playback 1: Front group");
		expect(list).toHaveTextContent("Color");
		expect(list).toHaveTextContent("Group color active");
		expect(list).toHaveTextContent("Page 1 Playback 2: Main cuelist");
		expect(list).toHaveTextContent("Dimmable");
	});

	it("explains which assignment targets are omitted", () => {
		render(<MatterBridgeSettings />);

		expect(
			screen.getByText(/Only assigned Cuelist, Dynamic, and Group Master/),
		).toHaveTextContent("other playback targets are omitted");
	});
});
