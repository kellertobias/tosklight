import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
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
