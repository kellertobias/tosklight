import {
	act,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../../app/ToastContext";
import { anOutputConfiguration, stubServer } from "../../testing/server";
import { OutputEditor, OutputSettings } from "./OutputSettings";

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

function setup(state: "loading" | "stalled" | "failed", mode: "sound" | "dmx") {
	const output = anOutputConfiguration("inventory-output", "Main");
	output.soundOutputKind = "device";
	output.soundOutputName = "Saved desk output";
	output.soundOutputInventory = {
		state,
		hasSuccessfulSnapshot: false,
		error: state === "failed" ? "OS enumeration failed" : null,
	};
	const server = stubServer({ outputConfigurations: { [output.id]: output } });
	const baseFetch = globalThis.fetch;
	vi.stubGlobal(
		"fetch",
		vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
			if (
				String(input).endsWith(`/outputs/${output.id}/configuration/update`)
			) {
				const edit = JSON.parse(String(init?.body ?? "{}"));
				Object.assign(output, edit);
				return new Response(JSON.stringify(output), {
					headers: { "content-type": "application/json" },
				});
			}
			return baseFetch(input, init);
		}),
	);
	render(
		<ToastProvider>
			<OutputSettings
				outputId={output.id}
				outputName="Main"
				mode={mode}
				direct
			/>
		</ToastProvider>,
	);
	return server;
}

describe("optional output-device inventory", () => {
	it("keeps the focused Universe input through an acknowledged clear before continued typing", async () => {
		const server = setup("loading", "dmx");
		const universe = await screen.findByLabelText("Universe");
		await userEvent.clear(universe);
		await waitFor(() =>
			expect(server.outputConfigurations["inventory-output"].universe).toBe(0),
		);
		await waitFor(() =>
			expect(
				vi
					.mocked(fetch)
					.mock.calls.filter(
						([input, init]) =>
							String(input).endsWith(
								"/outputs/inventory-output/configuration",
							) && !init?.method,
					).length,
			).toBeGreaterThanOrEqual(2),
		);
		await act(async () => {
			await Promise.resolve();
		});
		expect(universe.isConnected).toBe(true);
		expect(screen.getByLabelText("Universe")).toBe(universe);
		expect(universe).toHaveFocus();
		await userEvent.keyboard("51");
		await userEvent.tab();
		await waitFor(() =>
			expect(server.outputConfigurations["inventory-output"].universe).toBe(51),
		);
		expect(
			server.outputConfigurations["inventory-output"].soundOutputName,
		).toBe("Saved desk output");
	});
	it("keeps newer focused typing when an earlier acknowledged configuration read arrives late", async () => {
		const server = setup("loading", "dmx");
		const universe = await screen.findByLabelText("Universe");
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
				if (
					firstRead &&
					String(input).endsWith("/outputs/inventory-output/configuration") &&
					!init?.method
				) {
					firstRead = false;
					observed();
					await held;
				}
				return response;
			}),
		);
		await userEvent.clear(universe);
		await started;
		expect(universe).toHaveFocus();
		await userEvent.keyboard("51");
		await waitFor(() =>
			expect(server.outputConfigurations["inventory-output"].universe).toBe(51),
		);
		await act(async () => {
			release();
			await Promise.resolve();
		});
		expect(screen.getByLabelText("Universe")).toBe(universe);
		expect(universe).toHaveFocus();
		expect(universe).toHaveValue("51");
	});
	it.each([
		false,
		true,
	])("retains a newer return to the prior Universe value when the older clear is acknowledged (after idle: %s)", async (releaseAfterIdle) => {
		const server = setup("loading", "dmx");
		const universe = await screen.findByLabelText("Universe");
		const prior = server.outputConfigurations["inventory-output"].universe;
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
				if (
					firstRead &&
					String(input).endsWith("/outputs/inventory-output/configuration") &&
					!init?.method
				) {
					firstRead = false;
					observed();
					await held;
				}
				return response;
			}),
		);
		await userEvent.clear(universe);
		await started;
		expect(server.outputConfigurations["inventory-output"].universe).toBe(0);
		vi.useFakeTimers();
		fireEvent.change(universe, { target: { value: String(prior) } });
		if (releaseAfterIdle) {
			await act(async () => {
				await vi.advanceTimersByTimeAsync(180);
			});
			expect(screen.getByText("Saved automatically")).toBeInTheDocument();
			expect(universe).toHaveValue(String(prior));
		}
		await act(async () => {
			release();
			await vi.advanceTimersByTimeAsync(0);
		});
		expect(screen.getByLabelText("Universe")).toBe(universe);
		expect(universe).toHaveFocus();
		expect(universe).toHaveValue(String(prior));
		await act(async () => {
			await vi.advanceTimersByTimeAsync(180);
		});
		expect(server.outputConfigurations["inventory-output"].universe).toBe(
			prior,
		);
	});
	it("adopts external untouched fields without writing or replacing a newer local draft", async () => {
		const output = anOutputConfiguration("inventory-output", "Main");
		const onSave = vi.fn();
		const renderEditor = (configuration: typeof output) => (
			<OutputEditor
				output={configuration}
				mode="dmx"
				busy={false}
				showActions={false}
				onSave={onSave}
				onCancel={() => {}}
			/>
		);
		const view = render(renderEditor(output));
		const universe = screen.getByLabelText("Universe");
		await userEvent.clear(universe);
		await userEvent.keyboard("51");
		const calls = onSave.mock.calls.length;
		view.rerender(
			renderEditor({
				...output,
				universe: 0,
				startAddress: 20,
				protocol: "sacn",
			}),
		);
		await waitFor(() =>
			expect(screen.getByLabelText("Start address")).toHaveValue("20"),
		);
		expect(screen.getByRole("button", { name: "sACN" })).toBeInTheDocument();
		expect(screen.getByLabelText("Universe")).toBe(universe);
		expect(universe).toHaveFocus();
		expect(universe).toHaveValue("51");
		expect(onSave).toHaveBeenCalledTimes(calls);
		await userEvent.click(screen.getByLabelText("Start address"));
		await userEvent.keyboard("{Control>}a{/Control}21");
		await userEvent.tab();
		expect(onSave.mock.calls.at(-1)?.[0]).toMatchObject({
			universe: 51,
			startAddress: 21,
			protocol: "sacn",
		});
		const savedCalls = onSave.mock.calls.length;
		const acknowledged = {
			...output,
			universe: 51,
			startAddress: 21,
			protocol: "sacn" as const,
		};
		view.rerender(renderEditor(acknowledged));
		view.rerender(renderEditor({ ...acknowledged, universe: 52 }));
		expect(universe).toHaveValue("52");
		expect(onSave).toHaveBeenCalledTimes(savedCalls);
	});
	it("hides a retired output while its replacement configuration is loading", async () => {
		const old = anOutputConfiguration("old-output", "Old");
		const next = anOutputConfiguration("next-output", "Next", { universe: 2 });
		stubServer({ outputConfigurations: { [old.id]: old, [next.id]: next } });
		const baseFetch = fetch;
		let release!: () => void;
		const held = new Promise<void>((resolve) => {
			release = resolve;
		});
		vi.stubGlobal(
			"fetch",
			vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
				if (
					String(input).endsWith("/outputs/next-output/configuration") &&
					!init?.method
				)
					await held;
				const id = String(input).match(
					/outputs\/(old-output|next-output)\/configuration\/update$/,
				)?.[1];
				if (id) {
					const current = id === old.id ? old : next;
					Object.assign(current, JSON.parse(String(init?.body ?? "{}")));
					return new Response(JSON.stringify(current), {
						headers: { "content-type": "application/json" },
					});
				}
				return baseFetch(input, init);
			}),
		);
		const settings = (id: string) => (
			<ToastProvider>
				<OutputSettings outputId={id} outputName={id} mode="dmx" direct />
			</ToastProvider>
		);
		const view = render(settings(old.id));
		await userEvent.clear(await screen.findByLabelText("Universe"));
		await userEvent.keyboard("51");
		view.rerender(settings(next.id));
		expect(screen.queryByLabelText("Universe")).not.toBeInTheDocument();
		await waitFor(() => expect(old.universe).toBe(51));
		expect(next.universe).toBe(2);
		await act(async () => {
			release();
			await Promise.resolve();
		});
		expect(await screen.findByLabelText("Universe")).toHaveValue("2");
		expect(next.universe).toBe(2);
	});
	it.each([
		"loading",
		"stalled",
		"failed",
	] as const)("keeps the DMX editor writable while discovery is %s", async (state) => {
		const server = setup(state, "dmx");
		const universe = await screen.findByLabelText("Universe");
		await userEvent.clear(universe);
		await userEvent.type(universe, "51");
		await userEvent.tab();
		await waitFor(() =>
			expect(server.outputConfigurations["inventory-output"].universe).toBe(51),
		);
		expect(
			server.outputConfigurations["inventory-output"].soundOutputName,
		).toBe("Saved desk output");
		expect(
			screen.queryByText(/Reading Main output settings/),
		).not.toBeInTheDocument();
	});
	it.each([
		"loading",
		"stalled",
		"failed",
	] as const)("retains a selected device without claiming it unavailable while discovery is %s", async (state) => {
		setup(state, "sound");
		expect(
			await screen.findByText("Saved desk output · saved selection"),
		).toBeInTheDocument();
		expect(
			screen.queryByText(/Saved desk output · unavailable/),
		).not.toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Refresh audio devices" }),
		).toBeEnabled();
		const notice =
			state === "loading"
				? /Discovering audio output/
				: state === "stalled"
					? /taking longer/
					: /could not be discovered/;
		expect(screen.getByText(notice)).toBeVisible();
	});
});
