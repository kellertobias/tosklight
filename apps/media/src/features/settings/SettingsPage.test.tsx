import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../../app/ToastContext";
import { aNetwork, stubServer } from "../../testing/server";
import { SettingsPage } from "./SettingsPage";

afterEach(() => {
	window.history.replaceState(null, "", "/");
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe("the settings page", () => {
	it("uses dedicated title tabs and opens the network form by default", async () => {
		stubSettingsServer();
		renderSettings();

		const tabs = screen.getByRole("tablist");
		expect(tabs).toHaveTextContent("LibrariesPictureSoundNetworkDMXLogs");
		expect(tabs).not.toHaveTextContent("Audio");
		expect(await screen.findByLabelText("Art-Net")).toBeVisible();
		expect(
			screen.queryByRole("button", { name: "Change network settings" }),
		).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: /Save settings/ }),
		).not.toBeInTheDocument();
		expect(screen.getByRole("status")).toHaveTextContent("Saved automatically");
	});

	it("saves network settings automatically", async () => {
		const server = stubSettingsServer();
		renderSettings();
		expect(
			screen.queryByText(/used the next time this server starts/u),
		).not.toBeInTheDocument();
		const artNet = await screen.findByLabelText("Art-Net");
		await userEvent.clear(artNet);
		await userEvent.type(artNet, "192.168.1.40:6454");
		await waitFor(() =>
			expect(server.network.stored.artNetListen).toBe("192.168.1.40:6454"),
		);
		expect(
			screen.getByText(/used the next time this server starts/u),
		).toBeVisible();
	});

	it("renders the Libraries path explanation as regular information", async () => {
		stubSettingsServer();
		renderSettings();
		await openSettings("Libraries");
		const explanation = await screen.findByText(/library\.root/u);
		expect(explanation).not.toHaveClass("media-state");
		expect(explanation).not.toHaveClass("is-notice");
	});

	it("offers actual monitors on a direct picture-output form and saves only picture fields", async () => {
		const output = stubOutputConfiguration();
		renderSettings();
		await openSettings("Picture");
		await screen.findByRole("article", { name: "Main output settings" });
		expect(
			screen.queryByText(/Saved output changes take effect/u),
		).not.toBeInTheDocument();
		await choose("Off-screen (no window)", "Monitor");
		await choose(
			"Display 1 · Built-in Display · 2560 × 1600",
			"Display 2 · Stage Projector · 3840 × 2160",
		);
		await choose("1080p · 1920 × 1080", "Take from monitor");

		await waitFor(() => expect(output.writes).toHaveLength(1));
		expect(output.writes[0]).toMatchObject({
			targetKind: "monitor",
			monitorBy: "index",
			monitorValue: "1",
			width: 3840,
			height: 2160,
		});
		expect(output.writes[0]).not.toHaveProperty("soundOutputKind");
		expect(output.writes[0]).not.toHaveProperty("personality");
		expect(
			await screen.findByText(/Saved output changes take effect/u),
		).toBeVisible();
	});

	it("offers manual dimensions and broadcast fixed frame rates", async () => {
		const output = stubOutputConfiguration();
		renderSettings();
		await openSettings("Picture");
		await screen.findByRole("article", { name: "Main output settings" });

		expect(screen.queryByLabelText("Width")).not.toBeInTheDocument();
		await choose("1080p · 1920 × 1080", "Manual");
		await replaceNumber("Width", "2048");
		await replaceNumber("Height", "858");
		await choose("Display synchronized", "Fixed frame rate");
		await choose("60 fps", "59.94 fps · NTSC");

		await waitFor(() => expect(output.writes).toHaveLength(1));
		expect(output.writes[0]).toMatchObject({
			width: 2048,
			height: 858,
			presentation: "fixed-fps",
			framesPerSecond: 59.94,
		});
	});

	it("keeps an unavailable configured monitor without offering its missing size", async () => {
		stubOutputConfiguration({
			targetKind: "monitor",
			monitorBy: "name",
			monitorValue: "Disconnected projector",
			availableMonitors: [],
		});
		renderSettings();
		await openSettings("Picture");

		expect(
			await screen.findByRole("button", {
				name: "Configured monitor · Disconnected projector",
			}),
		).toBeVisible();
		await userEvent.click(
			screen.getByRole("button", { name: "1080p · 1920 × 1080" }),
		);
		expect(
			screen.getByRole("option", { name: "Take from monitor" }),
		).toBeDisabled();
	});

	it("offers actual audio devices on a separate sound-output form", async () => {
		const output = stubOutputConfiguration();
		renderSettings();
		await openSettings("Sound");
		await screen.findByRole("article", { name: "Main output settings" });
		expect(
			screen.queryByText(/Saved output changes take effect/u),
		).not.toBeInTheDocument();
		await choose("Muted", "Named device");
		await choose("Display 2", "USB Audio CODEC");

		await waitFor(() => expect(output.writes).toHaveLength(1));
		expect(output.writes[0]).toMatchObject({
			soundOutputKind: "device",
			soundOutputName: "USB Audio CODEC",
		});
		expect(output.writes[0]).not.toHaveProperty("width");
		expect(output.writes[0]).not.toHaveProperty("personality");
		expect(
			await screen.findByText(/Saved output changes take effect/u),
		).toBeVisible();
	});

	it("opens DMX directly and saves only the DMX intent", async () => {
		const output = stubOutputConfiguration();
		renderSettings();
		await openSettings("DMX");
		await screen.findByRole("article", { name: "Main DMX input settings" });
		expect(
			screen.queryByText(/Saved output changes take effect/u),
		).not.toBeInTheDocument();
		await choose("8 layers (279 slots)", "2 layers (75 slots)");
		await choose("Art-Net", "sACN");
		await replaceNumber("Universe", "12");
		await replaceNumber("Start address", "101");

		await waitFor(() => expect(output.writes).toHaveLength(1));
		expect(output.writes[0]).toMatchObject({
			personality: "two-layers",
			protocol: "sacn",
			universe: 12,
			startAddress: 101,
		});
		expect(output.writes[0]).not.toHaveProperty("targetKind");
		expect(
			await screen.findByText(/Saved output changes take effect/u),
		).toBeVisible();
	});

	it("opens DMX input from the diagnostics settings link", async () => {
		window.history.replaceState(null, "", "/settings?section=dmx");
		stubOutputConfiguration();
		renderSettings();

		expect(screen.getByRole("tab", { name: "DMX" })).toHaveAttribute(
			"aria-selected",
			"true",
		);
		expect(
			await screen.findByRole("heading", { name: "DMX input" }),
		).toBeVisible();
	});

	it("keeps Logs as its own settings tab without DMX diagnostics", async () => {
		stubSettingsServer();
		renderSettings();
		await openSettings("Logs");

		expect(await screen.findByText("Server log level")).toBeVisible();
		expect(
			screen.queryByRole("tab", { name: "DMX Diagnostics" }),
		).not.toBeInTheDocument();
	});

	it("hides the network restart notice until an address changes", async () => {
		stubSettingsServer();
		renderSettings();

		expect(
			screen.queryByText(/used the next time this server starts/u),
		).not.toBeInTheDocument();
	});

	it("places restart-aware status beside the content heading", async () => {
		stubSettingsServer();
		renderSettings();

		const network = await screen.findByRole("article", { name: "Network" });
		const heading = network.querySelector(".media-settings-section-heading");
		expect(heading).toContainElement(
			screen.getByRole("heading", { name: "Network" }),
		);
		expect(heading).toContainElement(screen.getByRole("status"));
		expect(screen.getByRole("status")).toHaveTextContent(
			"Saved automatically · Applies on restart",
		);
		expect(screen.getByRole("status").closest(".ui-window-header")).toBeNull();

		await openSettings("Libraries");
		expect(screen.getByRole("status")).toHaveTextContent("Saved automatically");
	});

	it("reverts stored network settings to the running values", async () => {
		const server = stubSettingsServer();
		renderSettings();
		const artNet = await screen.findByLabelText("Art-Net");
		await userEvent.clear(artNet);
		await userEvent.type(artNet, "192.168.1.40:6454");

		await userEvent.click(
			await screen.findByRole("button", { name: "Revert to current settings" }),
		);
		await waitFor(() => expect(server.network.pendingRestart).toBe(false));
		await waitFor(() =>
			expect(screen.getByLabelText("Art-Net")).toHaveValue("0.0.0.0:6454"),
		);
		expect(
			screen.queryByRole("button", { name: "Revert to current settings" }),
		).not.toBeInTheDocument();
	});

	it("reverts a changed picture section without touching sound or DMX", async () => {
		const output = stubOutputConfiguration();
		renderSettings();
		await openSettings("Picture");
		await choose("1080p · 1920 × 1080", "Manual");
		await replaceNumber("Width", "3840");

		await userEvent.click(
			await screen.findByRole("button", { name: "Revert to current settings" }),
		);
		await waitFor(() => expect(output.writes).toHaveLength(2));
		expect(output.writes[1]).toMatchObject({
			width: 1920,
			height: 1080,
			targetKind: "off-screen",
		});
		expect(output.writes[1]).not.toHaveProperty("soundOutputKind");
		expect(output.writes[1]).not.toHaveProperty("personality");
		await waitFor(() =>
			expect(
				screen.getByRole("button", { name: "1080p · 1920 × 1080" }),
			).toBeVisible(),
		);
		expect(
			screen.queryByRole("button", { name: "Revert to current settings" }),
		).not.toBeInTheDocument();
	});

	it("keeps a destination separate from every listen address", async () => {
		const server = stubSettingsServer();
		renderSettings();
		await screen.findByLabelText("Speed Group stream");
		// The destination lives under its own heading, not among the listeners.
		const sends = screen.getByRole("group", {
			name: "Where this server sends",
		});
		expect(sends).toBeInTheDocument();
		await userEvent.type(
			screen.getByLabelText("Speed Group stream"),
			"192.168.1.9:9000",
		);

		await waitFor(() =>
			expect(server.network.stored.speedGroupEndpoint).toBe("192.168.1.9:9000"),
		);
		expect(server.network.stored.artNetListen).toBe("0.0.0.0:6454");
	});

	it("clears a destination when the field is emptied", async () => {
		const server = stubSettingsServer({
			network: aNetwork({
				stored: {
					artNetListen: "0.0.0.0:6454",
					sacnListen: "0.0.0.0:5568",
					citpListen: "0.0.0.0:4809",
					httpListen: "127.0.0.1:8080",
					speedGroupEndpoint: "192.168.1.9:9000",
				},
			}),
		});
		renderSettings();
		await screen.findByLabelText("Speed Group stream");
		await userEvent.clear(screen.getByLabelText("Speed Group stream"));

		await waitFor(() =>
			expect(server.network.stored.speedGroupEndpoint).toBeNull(),
		);
	});

	it("says why a refused change was not applied", async () => {
		stubSettingsServer({
			refuseWrites: {
				code: "network-invalid",
				message: "citpListen is not an address and port, such as 0.0.0.0:4809",
				status: 400,
			},
		});
		renderSettings();
		const artNet = await screen.findByLabelText("Art-Net");
		await userEvent.clear(artNet);
		await userEvent.type(artNet, "invalid");

		const alert = await screen.findByRole("alert");
		expect(alert).toHaveTextContent("citpListen is not an address");
		expect(alert).toHaveClass("media-toast");
		expect(alert.closest(".ui-window-header")).toBeNull();
		expect(screen.getByRole("status")).toHaveTextContent("Not saved");
	});
});

function renderSettings() {
	return render(
		<ToastProvider>
			<SettingsPage />
		</ToastProvider>,
	);
}

type OutputConfigurationValues = {
	targetKind: "monitor" | "off-screen";
	monitorBy: "index" | "name" | null;
	monitorValue: string | null;
	fullscreen: boolean;
	width: number;
	height: number;
	presentation: "display-synchronized" | "fixed-fps" | "unlocked";
	framesPerSecond: number | null;
	soundOutputKind: "disabled" | "system-default" | "device";
	soundOutputName: string | null;
	personality: "two-layers" | "eight-layers";
	protocol: "art-net" | "sacn";
	universe: number;
	startAddress: number;
};

type OutputConfiguration = OutputConfigurationValues & {
	id: string;
	name: string;
	availableMonitors: Array<{
		index: number;
		name: string;
		width: number;
		height: number;
		refreshMillihertz: number | null;
	}>;
	availableSoundOutputs: string[];
	takesEffectOnRestart: boolean;
	active: OutputConfigurationValues;
	picturePendingRestart: boolean;
	soundPendingRestart: boolean;
	dmxPendingRestart: boolean;
};

function stubOutputConfiguration(overrides: Partial<OutputConfiguration> = {}) {
	stubServer();
	const active = activeOutputConfiguration();
	const configuration: OutputConfiguration = {
		id: "11111111-1111-4111-8111-111111111111",
		name: "Main",
		...active,
		availableMonitors: [
			{
				index: 0,
				name: "Built-in Display",
				width: 2560,
				height: 1600,
				refreshMillihertz: 60_000,
			},
			{
				index: 1,
				name: "Stage Projector",
				width: 3840,
				height: 2160,
				refreshMillihertz: 59_940,
			},
		],
		availableSoundOutputs: ["Display 2", "USB Audio CODEC"],
		takesEffectOnRestart: true,
		active,
		picturePendingRestart: false,
		soundPendingRestart: false,
		dmxPendingRestart: false,
		...overrides,
	};
	return installOutputConfiguration(configuration);
}

function stubSettingsServer(overrides: Parameters<typeof stubServer>[0] = {}) {
	const server = stubServer(overrides);
	const active = activeOutputConfiguration();
	installOutputConfiguration({
		id: "11111111-1111-4111-8111-111111111111",
		name: "Main",
		...active,
		availableMonitors: [
			{
				index: 0,
				name: "Built-in Display",
				width: 2560,
				height: 1600,
				refreshMillihertz: 60_000,
			},
			{
				index: 1,
				name: "Stage Projector",
				width: 3840,
				height: 2160,
				refreshMillihertz: 59_940,
			},
		],
		availableSoundOutputs: ["Display 2", "USB Audio CODEC"],
		takesEffectOnRestart: true,
		active,
		picturePendingRestart: false,
		soundPendingRestart: false,
		dmxPendingRestart: false,
	});
	return server;
}

function installOutputConfiguration(configuration: OutputConfiguration) {
	const baseFetch = globalThis.fetch;
	const writes: Array<Record<string, unknown>> = [];

	vi.stubGlobal(
		"fetch",
		vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
			const path = String(input).replace("/api/v2", "");
			if (path === `/outputs/${configuration.id}/configuration`) {
				return new Response(JSON.stringify(configuration), {
					status: 200,
					headers: { "content-type": "application/json" },
				});
			}
			if (path === `/outputs/${configuration.id}/configuration/update`) {
				const body = JSON.parse(String(init?.body ?? "{}"));
				writes.push(body);
				Object.assign(configuration, body);
				configuration.picturePendingRestart = pictureValuesDiffer(
					configuration,
					configuration.active,
				);
				configuration.soundPendingRestart =
					configuration.soundOutputKind !==
						configuration.active.soundOutputKind ||
					configuration.soundOutputName !==
						configuration.active.soundOutputName;
				configuration.dmxPendingRestart =
					configuration.personality !== configuration.active.personality ||
					configuration.protocol !== configuration.active.protocol ||
					configuration.universe !== configuration.active.universe ||
					configuration.startAddress !== configuration.active.startAddress;
				return new Response(JSON.stringify(configuration), {
					status: 200,
					headers: { "content-type": "application/json" },
				});
			}
			return baseFetch(input, init);
		}),
	);

	return { configuration, writes };
}

function activeOutputConfiguration(): OutputConfigurationValues {
	return {
		targetKind: "off-screen",
		monitorBy: null,
		monitorValue: null,
		fullscreen: false,
		width: 1920,
		height: 1080,
		presentation: "display-synchronized",
		framesPerSecond: null,
		soundOutputKind: "disabled",
		soundOutputName: null,
		personality: "eight-layers",
		protocol: "art-net",
		universe: 0,
		startAddress: 1,
	};
}

function pictureValuesDiffer(
	configuration: OutputConfigurationValues,
	active: OutputConfigurationValues,
) {
	return (
		configuration.targetKind !== active.targetKind ||
		configuration.monitorBy !== active.monitorBy ||
		configuration.monitorValue !== active.monitorValue ||
		configuration.fullscreen !== active.fullscreen ||
		configuration.width !== active.width ||
		configuration.height !== active.height ||
		configuration.presentation !== active.presentation ||
		configuration.framesPerSecond !== active.framesPerSecond
	);
}

async function choose(current: string, next: string) {
	await userEvent.click(screen.getByRole("button", { name: current }));
	await userEvent.click(screen.getByRole("option", { name: next }));
}

async function replaceNumber(label: string, value: string) {
	const field = screen.getByLabelText(label);
	await userEvent.clear(field);
	await userEvent.type(field, value);
}

async function openSettings(
	name: "Libraries" | "Picture" | "Sound" | "Network" | "DMX" | "Logs",
) {
	await userEvent.click(screen.getByRole("tab", { name }));
}
