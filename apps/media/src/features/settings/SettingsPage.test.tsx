import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { aNetwork, stubServer } from "../../testing/server";
import { SettingsPage } from "./SettingsPage";

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe("the settings page", () => {
	it("shows the stored output target, picture, DMX patch, and restart boundary", async () => {
		stubOutputConfiguration({
			targetKind: "monitor",
			monitorBy: "name",
			monitorValue: "Stage projector",
			fullscreen: true,
			width: 3840,
			height: 2160,
			presentation: "fixed-fps",
			framesPerSecond: 50,
			personality: "two-layers",
			protocol: "sacn",
			universe: 12,
			startAddress: 101,
		});
		render(<SettingsPage />);
		await openSettings("Outputs");

		const settings = await screen.findByRole("article", {
			name: "Main output settings",
		});
		expect(settings).toHaveTextContent(
			"monitor named Stage projector, full-screen",
		);
		expect(settings).toHaveTextContent("3840 × 2160");
		expect(settings).toHaveTextContent("Fixed at 50 frames per second");
		expect(settings).toHaveTextContent("Muted");
		expect(settings).toHaveTextContent(/next time this server starts/u);
		expect(screen.queryByText(outputIdPattern)).not.toBeInTheDocument();

		await openSettings("Network & Inputs");
		await openSettingsTab("DMX");
		const dmx = await screen.findByRole("article", {
			name: "Main DMX input settings",
		});
		expect(dmx).toHaveTextContent("2 layers");
		expect(dmx).toHaveTextContent("sACN, universe 12, address 101");
	});

	it("edits the complete output identity and makes the deferred effect explicit", async () => {
		const output = stubOutputConfiguration();
		render(<SettingsPage />);
		await openSettings("Outputs");

		await userEvent.click(
			await screen.findByRole("button", { name: "Change output settings" }),
		);
		expect(
			screen.getByText(/output running now\s+stays as it is/u),
		).toBeInTheDocument();

		await choose("Off-screen (no window)", "Monitor");
		await choose("Monitor number", "Monitor name");
		await userEvent.clear(screen.getByLabelText("Monitor name"));
		await userEvent.type(
			screen.getByLabelText("Monitor name"),
			"Stage projector",
		);
		await userEvent.click(screen.getByLabelText("Full-screen"));

		await replaceNumber("Width", "3840");
		await replaceNumber("Height", "2160");
		await choose("Display synchronized", "Fixed frame rate");
		await replaceNumber("Frames per second", "50");
		await choose("Muted", "Named device");
		await userEvent.type(screen.getByLabelText("Device name"), "Display 2");
		await userEvent.click(
			screen.getByRole("button", { name: "Save output settings" }),
		);

		await waitFor(() => expect(output.writes).toHaveLength(1));
		expect(output.writes[0]).toMatchObject({
			targetKind: "monitor",
			monitorBy: "name",
			monitorValue: "Stage projector",
			fullscreen: true,
			width: 3840,
			height: 2160,
			presentation: "fixed-fps",
			framesPerSecond: 50,
			soundOutputKind: "device",
			soundOutputName: "Display 2",
			personality: "eight-layers",
			protocol: "art-net",
			universe: 0,
			startAddress: 1,
		});
		expect(output.writes[0]?.requestId).toEqual(expect.any(String));
	});

	it("omits monitor-only settings when an output becomes off-screen", async () => {
		const output = stubOutputConfiguration({
			targetKind: "monitor",
			monitorBy: "index",
			monitorValue: "2",
			fullscreen: true,
		});
		render(<SettingsPage />);
		await openSettings("Outputs");

		await userEvent.click(
			await screen.findByRole("button", { name: "Change output settings" }),
		);
		await choose("Monitor", "Off-screen (no window)");
		await userEvent.click(
			screen.getByRole("button", { name: "Save output settings" }),
		);

		await waitFor(() => expect(output.writes).toHaveLength(1));
		expect(output.writes[0]).toMatchObject({
			targetKind: "off-screen",
		});
		expect(output.writes[0]).not.toHaveProperty("monitorBy");
		expect(output.writes[0]).not.toHaveProperty("monitorValue");
		expect(output.writes[0]).not.toHaveProperty("fullscreen");
		expect(output.writes[0]).not.toHaveProperty("framesPerSecond");
	});

	it("edits DMX personality and address under Network & Inputs without changing the picture", async () => {
		const output = stubOutputConfiguration({
			targetKind: "monitor",
			monitorBy: "name",
			monitorValue: "Stage projector",
			fullscreen: true,
		});
		render(<SettingsPage />);
		await openSettings("Network & Inputs");
		await openSettingsTab("DMX");

		await userEvent.click(
			await screen.findByRole("button", { name: "Change DMX input" }),
		);
		await choose("8 layers (279 slots)", "2 layers (75 slots)");
		await choose("Art-Net", "sACN");
		await replaceNumber("Universe", "12");
		await replaceNumber("Start address", "101");
		await userEvent.click(
			screen.getByRole("button", { name: "Save DMX input" }),
		);

		await waitFor(() => expect(output.writes).toHaveLength(1));
		expect(output.writes[0]).toMatchObject({
			personality: "two-layers",
			protocol: "sacn",
			universe: 12,
			startAddress: 101,
			targetKind: "monitor",
			monitorBy: "name",
			monitorValue: "Stage projector",
			fullscreen: true,
		});
	});

	it("separates Network, DMX, and Audio into window title tabs", async () => {
		stubSettingsServer();
		render(<SettingsPage />);
		await openSettings("Network & Inputs");

		const tabs = screen.getByRole("radiogroup", {
			name: "Network and input settings",
		});
		expect(tabs).toHaveTextContent("NetworkDMXAudio");
		expect(
			await screen.findByRole("article", { name: "Network" }),
		).toBeVisible();

		await openSettingsTab("DMX");
		expect(
			await screen.findByRole("article", { name: "Main DMX input settings" }),
		).toBeVisible();
		expect(
			screen.queryByRole("article", { name: "Network" }),
		).not.toBeInTheDocument();

		await openSettingsTab("Audio");
		expect(
			await screen.findByRole("article", { name: "Audio monitor" }),
		).toBeVisible();
		expect(
			screen.queryByRole("article", { name: "Main DMX input settings" }),
		).not.toBeInTheDocument();
	});

	it("puts DMX Diagnostics in its own Logs title tab", async () => {
		stubSettingsServer();
		render(<SettingsPage />);
		await openSettings("Logs");

		const tabs = screen.getByRole("radiogroup", {
			name: "Logs and diagnostics",
		});
		expect(tabs).toHaveTextContent("LogsDMX Diagnostics");
		await openSettingsTab("DMX Diagnostics");
		expect(
			await screen.findByRole("heading", { name: "GDTF fixtures" }),
		).toBeVisible();
		expect(screen.getByLabelText("Main DMX")).toBeVisible();
		expect(screen.queryByText("Server log level")).not.toBeInTheDocument();
	});

	it("shows what was configured beside what this run actually bound", async () => {
		stubSettingsServer({
			network: aNetwork({
				sameComputerPreset: true,
				resolved: {
					artNetListen: "127.0.0.1:6454",
					sacnListen: "127.0.0.1:5568",
					citpListen: "127.0.0.1:4809",
					httpListen: "127.0.0.1:8080",
					speedGroupEndpoint: null,
				},
			}),
		});
		render(<SettingsPage />);
		await openSettings("Network & Inputs");

		const row = (
			await screen.findByRole("rowheader", { name: "Art-Net" })
		).closest("tr");
		expect(row).not.toBeNull();
		expect(row).toHaveTextContent("0.0.0.0:6454");
		expect(row).toHaveTextContent("127.0.0.1:6454");
	});

	it("says a saved address is used on the next start rather than implying a rebind", async () => {
		stubSettingsServer();
		render(<SettingsPage />);
		await openSettings("Network & Inputs");

		expect(
			await screen.findByText(/used the next time this server starts/u),
		).toBeInTheDocument();
	});

	it("edits a listen address through the write path, with a request id", async () => {
		const server = stubSettingsServer();
		render(<SettingsPage />);
		await openSettings("Network & Inputs");

		await userEvent.click(
			await screen.findByRole("button", { name: "Change network settings" }),
		);
		const artNet = screen.getByLabelText("Art-Net");
		await userEvent.clear(artNet);
		await userEvent.type(artNet, "192.168.1.40:6454");
		await userEvent.click(
			screen.getByRole("button", { name: "Save network settings" }),
		);

		await waitFor(() =>
			expect(server.network.stored.artNetListen).toBe("192.168.1.40:6454"),
		);
		expect(server.writes).toContain("/network/update");
	});

	it("keeps a destination separate from every listen address", async () => {
		const server = stubSettingsServer();
		render(<SettingsPage />);
		await openSettings("Network & Inputs");

		await userEvent.click(
			await screen.findByRole("button", { name: "Change network settings" }),
		);
		// The destination lives under its own heading, not among the listeners.
		const sends = screen.getByRole("group", {
			name: "Where this server sends",
		});
		expect(sends).toBeInTheDocument();
		await userEvent.type(
			screen.getByLabelText("Speed Group stream"),
			"192.168.1.9:9000",
		);
		await userEvent.click(
			screen.getByRole("button", { name: "Save network settings" }),
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
		render(<SettingsPage />);
		await openSettings("Network & Inputs");

		await userEvent.click(
			await screen.findByRole("button", { name: "Change network settings" }),
		);
		await userEvent.clear(screen.getByLabelText("Speed Group stream"));
		await userEvent.click(
			screen.getByRole("button", { name: "Save network settings" }),
		);

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
		render(<SettingsPage />);
		await openSettings("Network & Inputs");

		await userEvent.click(
			await screen.findByRole("button", { name: "Change network settings" }),
		);
		await userEvent.click(
			screen.getByRole("button", { name: "Save network settings" }),
		);

		expect(await screen.findByRole("alert")).toHaveTextContent(
			"citpListen is not an address",
		);
	});
});

const outputIdPattern = /11111111-1111-4111-8111-111111111111/u;

type OutputConfiguration = {
	id: string;
	name: string;
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
	takesEffectOnRestart: boolean;
};

function stubOutputConfiguration(overrides: Partial<OutputConfiguration> = {}) {
	stubServer();
	const configuration: OutputConfiguration = {
		id: "11111111-1111-4111-8111-111111111111",
		name: "Main",
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
		takesEffectOnRestart: true,
		...overrides,
	};
	return installOutputConfiguration(configuration);
}

function stubSettingsServer(overrides: Parameters<typeof stubServer>[0] = {}) {
	const server = stubServer(overrides);
	installOutputConfiguration({
		id: "11111111-1111-4111-8111-111111111111",
		name: "Main",
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
		takesEffectOnRestart: true,
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
	name: "Libraries" | "Outputs" | "Network & Inputs" | "Logs",
) {
	await userEvent.click(screen.getByRole("radio", { name }));
}

async function openSettingsTab(
	name: "Network" | "DMX" | "Audio" | "Logs" | "DMX Diagnostics",
) {
	await userEvent.click(screen.getByRole("radio", { name }));
}
