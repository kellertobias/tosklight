import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ModalProvider } from "@tosklight/ui/modals";
import { afterEach, expect, it, vi } from "vitest";
import { ToastProvider } from "../../app/ToastContext";
import { aNetwork, stubServer } from "../../testing/server";
import { SettingsPage } from "./SettingsPage";
import { NetworkEditor } from "./NetworkEditor";

afterEach(() => {
	vi.useRealTimers();
	window.history.replaceState(null, "", "/");
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

it.each([
	{ returnToPrior: false, releaseAfterIdle: false },
	{ returnToPrior: true, releaseAfterIdle: false },
	{ returnToPrior: true, releaseAfterIdle: true },
])("keeps newer focused network text across an older acknowledgement (%j)", async ({
	returnToPrior,
	releaseAfterIdle,
}) => {
	const server = stubSettingsServer();
	renderSettings();
	const art = await screen.findByLabelText("Art-Net");
	const newer = returnToPrior
		? server.network.stored.artNetListen
		: "127.0.0.1:16455";
	await userEvent.click(art);
	const baseFetch = fetch;
	let release!: () => void;
	let observed!: () => void;
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const started = new Promise<void>((resolve) => {
		observed = resolve;
	});
	let firstRead = true;
	vi.stubGlobal(
		"fetch",
		vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
			const response = await baseFetch(input, init);
			if (firstRead && String(input).endsWith("/network") && !init?.method) {
				firstRead = false;
				observed();
				await held;
			}
			return response;
		}),
	);
	vi.useFakeTimers();
	fireEvent.change(art, { target: { value: "127.0.0.1:16454" } });
	await act(async () => {
		await vi.advanceTimersByTimeAsync(180);
	});
	await started;
	expect(server.network.stored.artNetListen).toBe("127.0.0.1:16454");
	fireEvent.change(art, { target: { value: newer } });
	if (releaseAfterIdle) {
		await act(async () => {
			await vi.advanceTimersByTimeAsync(180);
		});
		const panel = art.closest("article")!;
		expect(within(panel).getByText("Saved automatically")).toBeInTheDocument();
		expect(art).toHaveValue(newer);
	}
	await act(async () => {
		release();
		await vi.advanceTimersByTimeAsync(0);
	});
	expect(screen.getByLabelText("Art-Net")).toBe(art);
	expect(art).toHaveFocus();
	expect(art).toHaveValue(newer);
	await act(async () => {
		await vi.advanceTimersByTimeAsync(180);
	});
	expect(server.network.stored.artNetListen).toBe(newer);
});

it("adopts untouched external network fields without authoring a prop-only edit", async () => {
	const network = aNetwork();
	const onSave = vi.fn();
	const editor = (value: typeof network) => (
		<NetworkEditor
			network={value}
			busy={false}
			onSave={onSave}
			showActions={false}
		/>
	);
	const view = render(editor(network));
	const art = screen.getByLabelText("Art-Net");
	await userEvent.click(art);
	fireEvent.change(art, { target: { value: "127.0.0.1:16455" } });
	const calls = onSave.mock.calls.length;
	view.rerender(
		editor(
			aNetwork({
				...network,
				stored: {
					...network.stored,
					artNetListen: "127.0.0.1:16454",
					sacnListen: "127.0.0.1:15568",
				},
			}),
		),
	);
	expect(screen.getByLabelText("Art-Net")).toBe(art);
	expect(art).toHaveFocus();
	expect(art).toHaveValue("127.0.0.1:16455");
	expect(screen.getByLabelText("sACN")).toHaveValue("127.0.0.1:15568");
	expect(onSave).toHaveBeenCalledTimes(calls);
	fireEvent.change(art, { target: { value: "127.0.0.1:16456" } });
	expect(onSave.mock.calls.at(-1)?.[0]).toMatchObject({
		artNetListen: "127.0.0.1:16456",
		sacnListen: "127.0.0.1:15568",
	});
	const savedCalls = onSave.mock.calls.length;
	const acknowledged = aNetwork({
		...network,
		stored: {
			...network.stored,
			artNetListen: "127.0.0.1:16456",
			sacnListen: "127.0.0.1:15568",
		},
	});
	view.rerender(editor(acknowledged));
	view.rerender(
		editor(
			aNetwork({
				...acknowledged,
				stored: { ...acknowledged.stored, artNetListen: "127.0.0.1:16457" },
			}),
		),
	);
	expect(art).toHaveValue("127.0.0.1:16457");
	expect(onSave).toHaveBeenCalledTimes(savedCalls);
});

it("keeps a network edit made30ms before leaving its tab", async () => {
	const server = stubSettingsServer();
	renderSettings();
	const art = await screen.findByLabelText("Art-Net");
	const original = server.network.stored.artNetListen;
	vi.useFakeTimers();
	fireEvent.change(art, { target: { value: "127.0.0.1:16454" } });
	await act(async () => {
		await vi.advanceTimersByTimeAsync(30);
	});
	expect(server.network.stored.artNetListen).toBe(original);
	fireEvent.click(screen.getByRole("tab", { name: "Picture" }));
	await act(async () => {
		await vi.advanceTimersByTimeAsync(400);
	});
	expect(server.network.stored.artNetListen).toBe("127.0.0.1:16454");
});

it("orders picture Revert after the pending draft", async () => {
	const output = stubOutputConfiguration({
		width: 3840,
		height: 2160,
		picturePendingRestart: true,
	});
	renderSettings();
	fireEvent.click(screen.getByRole("tab", { name: "Picture" }));
	await screen.findByRole("article", { name: "Main output settings" });
	await screen.findByLabelText("Width");
	vi.useFakeTimers();
	fireEvent.change(screen.getByLabelText("Width"), {
		target: { value: "4096" },
	});
	fireEvent.click(
		screen.getByRole("button", { name: "Revert to current settings" }),
	);
	await act(async () => {
		await vi.advanceTimersByTimeAsync(400);
	});
	expect(output.configuration.width).toBe(1920);
	expect(output.writes.at(-1)).toMatchObject({ width: 1920 });
});

it("keeps unrelated pending network fields when reverting restart-bound fields", async () => {
	const initial = aNetwork();
	const server = stubSettingsServer({
		network: aNetwork({
			stored: { ...initial.stored, citpListen: "127.0.0.1:14809" },
			pendingRestart: true,
		}),
	});
	renderSettings();
	const art = await screen.findByLabelText("Art-Net");
	vi.useFakeTimers();
	fireEvent.change(art, { target: { value: "127.0.0.1:16454" } });
	fireEvent.change(screen.getByLabelText("Speed Groups"), {
		target: { value: "127.0.0.1:14810" },
	});
	fireEvent.change(screen.getByLabelText("CITP"), {
		target: { value: "127.0.0.1:14811" },
	});
	fireEvent.click(
		screen.getByRole("button", { name: "Revert to current settings" }),
	);
	await act(async () => {
		await vi.advanceTimersByTimeAsync(400);
	});
	expect(server.network.stored).toMatchObject({
		artNetListen: "127.0.0.1:16454",
		speedGroupEndpoint: "127.0.0.1:14810",
		citpListen: initial.activeStored.citpListen,
	});
});

it("lets a later network edit follow Revert without resurrecting its old CITP draft", async () => {
	const initial = aNetwork();
	const server = stubSettingsServer({
		network: aNetwork({
			stored: { ...initial.stored, citpListen: "127.0.0.1:14809" },
			pendingRestart: true,
		}),
	});
	renderSettings();
	const art = await screen.findByLabelText("Art-Net");
	vi.useFakeTimers();
	fireEvent.change(art, { target: { value: "127.0.0.1:16454" } });
	fireEvent.click(
		screen.getByRole("button", { name: "Revert to current settings" }),
	);
	fireEvent.change(screen.getByLabelText("Art-Net"), {
		target: { value: "127.0.0.1:16455" },
	});
	await act(async () => {
		await vi.advanceTimersByTimeAsync(400);
	});
	expect(server.network.stored).toMatchObject({
		artNetListen: "127.0.0.1:16455",
		citpListen: initial.activeStored.citpListen,
	});
});

it("shows network queue progress and reports a refusal after navigation", async () => {
	const server = stubSettingsServer({
		refuseWrites: {
			code: "network-invalid",
			message: "The listener address was refused",
			status: 400,
		},
	});
	renderSettings();
	const art = await screen.findByLabelText("Art-Net");
	const stored = server.network.stored.artNetListen;
	vi.useFakeTimers();
	fireEvent.change(art, { target: { value: "127.0.0.1:16454" } });
	expect(
		within(screen.getByRole("article", { name: "Network" })).getByRole(
			"status",
		),
	).toHaveTextContent("Saving");
	fireEvent.click(screen.getByRole("tab", { name: "Picture" }));
	await act(async () => {
		await vi.advanceTimersByTimeAsync(400);
	});
	expect(screen.getByRole("alert")).toHaveTextContent(
		"The listener address was refused",
	);
	expect(server.network.stored.artNetListen).toBe(stored);
});

it("coalesces a network return to its stored value without opening writes", async () => {
	const server = stubSettingsServer();
	renderSettings();
	const art = await screen.findByLabelText("Art-Net");
	const writes = () =>
		vi
			.mocked(fetch)
			.mock.calls.filter(([input]) =>
				String(input).endsWith("/network/update"),
			);
	expect(writes()).toHaveLength(0);
	const stored = server.network.stored.artNetListen;
	vi.useFakeTimers();
	fireEvent.change(art, { target: { value: "127.0.0.1:16454" } });
	fireEvent.change(art, { target: { value: stored } });
	fireEvent.click(screen.getByRole("tab", { name: "Picture" }));
	await act(async () => {
		await vi.advanceTimersByTimeAsync(400);
	});
	expect(writes()).toHaveLength(1);
	expect(server.network.stored.artNetListen).toBe(stored);
});

it("reverts only DMX personality after its queued live address change", async () => {
	const output = stubOutputConfiguration({
		personality: "two-layers",
		dmxPendingRestart: true,
	});
	renderSettings();
	const article = await screen.findByRole("article", {
		name: "Main DMX input settings",
	});
	const universe = await within(article).findByLabelText("Universe");
	vi.useFakeTimers();
	fireEvent.change(universe, { target: { value: "7" } });
	fireEvent.click(
		within(article).getByRole("button", { name: "Revert to current settings" }),
	);
	await act(async () => {
		await vi.advanceTimersByTimeAsync(400);
	});
	expect(output.configuration).toMatchObject({
		personality: "eight-layers",
		universe: 7,
	});
	expect(output.writes.at(-1)).not.toHaveProperty("universe");
});

it("retains reverted DMX personality while a later universe edit is still unacknowledged", async () => {
	const output = stubOutputConfiguration({
		personality: "two-layers",
		dmxPendingRestart: true,
	});
	renderSettings();
	const universe = await screen.findByLabelText("Universe");
	vi.useFakeTimers();
	fireEvent.change(universe, { target: { value: "7" } });
	fireEvent.click(
		screen.getByRole("button", { name: "Revert to current settings" }),
	);
	fireEvent.change(screen.getByLabelText("Universe"), {
		target: { value: "8" },
	});
	await act(async () => {
		await vi.advanceTimersByTimeAsync(400);
	});
	expect(output.configuration).toMatchObject({
		personality: "eight-layers",
		universe: 8,
	});
});

function renderSettings() {
	return render(
		<ModalProvider>
			<ToastProvider>
				<SettingsPage />
			</ToastProvider>
		</ModalProvider>,
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
	tempoSource?: "playback-bpm-channel" | "speed-group";
	speedGroup?: number | null;
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
				// The DMX address is rerouted live; only the personality waits.
				configuration.dmxPendingRestart =
					configuration.personality !== configuration.active.personality;
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
	name: "Libraries" | "Picture" | "Sound" | "Network & DMX" | "Logs",
) {
	await userEvent.click(screen.getByRole("tab", { name }));
}
