import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { useState } from "react";
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
const loadExtensions = vi.fn();
const rescanExtensions = vi.fn();
vi.mock("../../features/extensions/ExtensionRuntimeActions", () => ({
	useExtensionRuntimeActions: () => ({
		load: loadExtensions,
		rescan: rescanExtensions,
	}),
}));

afterEach(cleanup);

describe("Network & Inputs settings", () => {
	it("renders extension status and rescans through the authenticated capability", async () => {
		loadExtensions.mockResolvedValueOnce(extensionSnapshot());
		rescanExtensions.mockResolvedValueOnce({
			...extensionSnapshot(),
			instances: [
				{
					...extensionSnapshot().instances[0],
					state: "running",
				},
			],
		});
		const controller = {
			networkTab: "control-server",
			draft: {
				osc_bind: "0.0.0.0:8000",
			},
			serverUrl: "http://127.0.0.1:5000",
			setServerUrl: vi.fn(),
			applyServerUrl: vi.fn(),
		} as unknown as SetupWindowController;
		render(<NetworkSection controller={controller} />);
		expect(screen.getByText("Native extensions")).toBeInTheDocument();
		await screen.findByText("1 package · 0 running instances");
		fireEvent.click(screen.getByRole("button", { name: "Rescan extensions" }));
		await waitFor(() => expect(rescanExtensions).toHaveBeenCalledOnce());
		await screen.findByText("1 package · 1 running instance");
		fireEvent.click(screen.getByText("Extension diagnostics"));
		expect(screen.getByText("/runtime/extensions")).toBeInTheDocument();
		expect(screen.getByText("Example controls")).toBeInTheDocument();

		expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent(
			"Network & Inputs",
		);
		expect(
			screen
				.getAllByRole("heading", { level: 3 })
				.map((heading) => heading.textContent),
		).toEqual(["ToskLight server connection", "Control inputs"]);
		expect(screen.queryByRole("heading", { name: "Sound input" })).toBeNull();
		expect(screen.queryByRole("heading", { name: "Matter bridge" })).toBeNull();
		expect(screen.queryByRole("heading", { name: "Inputs" })).toBeNull();
		expect(screen.queryByRole("heading", { name: "Services" })).toBeNull();
	});

	it("keeps each tab mounted so unsaved control state survives tab changes", () => {
		function Harness() {
			const [networkTab, setNetworkTab] = useState<
				"control-server" | "sound" | "bridges"
			>("control-server");
			const [serverUrl, setServerUrl] = useState("http://desk.local:5000");
			return (
				<>
					<button type="button" onClick={() => setNetworkTab("control-server")}>
						Control
					</button>
					<button type="button" onClick={() => setNetworkTab("sound")}>
						Sound tab
					</button>
					<button type="button" onClick={() => setNetworkTab("bridges")}>
						Bridges tab
					</button>
					<NetworkSection
						controller={
							{
								draft: { osc_bind: "0.0.0.0:8000" },
								networkTab,
								serverUrl,
								setServerUrl,
								applyServerUrl: vi.fn(),
							} as unknown as SetupWindowController
						}
					/>
				</>
			);
		}
		render(<Harness />);
		const input = screen.getByLabelText("Light server URL");
		fireEvent.change(input, { target: { value: "http://draft.local:5000" } });
		fireEvent.click(screen.getByRole("button", { name: "Sound tab" }));
		expect(screen.getByText("Sound settings")).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: "Bridges tab" }));
		expect(screen.getByText("Matter settings")).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: "Control" }));
		expect(screen.getByLabelText("Light server URL")).toHaveValue(
			"http://draft.local:5000",
		);
	});
});

function extensionSnapshot() {
	return {
		extensions_directory: "/runtime/extensions",
		configuration_path: "/data/extensions.json",
		configuration_diagnostic: null,
		packages: [
			{
				id: "de.tosklight.example",
				name: "Example controls",
				version: "1.0.0",
				directory: "/runtime/extensions/example",
				package_digest: "sha256:example",
				readiness: "ready",
				locally_approved_unsigned: true,
				diagnostics: [],
			},
		],
		instances: [
			{
				id: "controls-1",
				extension_id: "de.tosklight.example",
				package_digest: "sha256:example",
				executable: "/runtime/extensions/example/example",
				state: "stopped",
				last_error: null,
				launches: 0,
				crashes: 0,
				protocol_errors: 0,
				inbound_drops: 0,
				outbound_drops: 0,
			},
		],
		instance_diagnostics: [],
	};
}
