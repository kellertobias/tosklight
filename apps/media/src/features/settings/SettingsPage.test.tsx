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
	it("shows what was configured beside what this run actually bound", async () => {
		stubServer({
			network: aNetwork({
				sameComputerPreset: true,
				resolved: {
					artNetListen: "127.0.0.1:6454",
					sacnListen: "127.0.0.1:5568",
					citpListen: "127.0.0.1:4811",
					httpListen: "127.0.0.1:8080",
					speedGroupEndpoint: null,
				},
			}),
		});
		render(<SettingsPage />);

		const row = (await screen.findByRole("rowheader", { name: "Art-Net" })).closest("tr");
		expect(row).not.toBeNull();
		expect(row).toHaveTextContent("0.0.0.0:6454");
		expect(row).toHaveTextContent("127.0.0.1:6454");
	});

	it("says a saved address is used on the next start rather than implying a rebind", async () => {
		stubServer();
		render(<SettingsPage />);

		expect(
			await screen.findByText(/used the next time this server starts/u),
		).toBeInTheDocument();
	});

	it("edits a listen address through the write path, with a request id", async () => {
		const server = stubServer();
		render(<SettingsPage />);

		await userEvent.click(await screen.findByRole("button", { name: "Change network settings" }));
		const artNet = screen.getByLabelText("Art-Net");
		await userEvent.clear(artNet);
		await userEvent.type(artNet, "192.168.1.40:6454");
		await userEvent.click(screen.getByRole("button", { name: "Save" }));

		await waitFor(() =>
			expect(server.network.stored.artNetListen).toBe("192.168.1.40:6454"),
		);
		expect(server.writes).toContain("/network/update");
	});

	it("keeps a destination separate from every listen address", async () => {
		const server = stubServer();
		render(<SettingsPage />);

		await userEvent.click(await screen.findByRole("button", { name: "Change network settings" }));
		// The destination lives under its own heading, not among the listeners.
		const sends = screen.getByRole("group", { name: "Where this server sends" });
		expect(sends).toBeInTheDocument();
		await userEvent.type(screen.getByLabelText("Speed Group stream"), "192.168.1.9:9000");
		await userEvent.click(screen.getByRole("button", { name: "Save" }));

		await waitFor(() =>
			expect(server.network.stored.speedGroupEndpoint).toBe("192.168.1.9:9000"),
		);
		expect(server.network.stored.artNetListen).toBe("0.0.0.0:6454");
	});

	it("clears a destination when the field is emptied", async () => {
		const server = stubServer({
			network: aNetwork({
				stored: {
					artNetListen: "0.0.0.0:6454",
					sacnListen: "0.0.0.0:5568",
					citpListen: "0.0.0.0:4811",
					httpListen: "127.0.0.1:8080",
					speedGroupEndpoint: "192.168.1.9:9000",
				},
			}),
		});
		render(<SettingsPage />);

		await userEvent.click(await screen.findByRole("button", { name: "Change network settings" }));
		await userEvent.clear(screen.getByLabelText("Speed Group stream"));
		await userEvent.click(screen.getByRole("button", { name: "Save" }));

		await waitFor(() => expect(server.network.stored.speedGroupEndpoint).toBeNull());
	});

	it("says why a refused change was not applied", async () => {
		stubServer({
			refuseWrites: {
				code: "network-invalid",
				message: "citpListen is not an address and port, such as 0.0.0.0:4811",
				status: 400,
			},
		});
		render(<SettingsPage />);

		await userEvent.click(await screen.findByRole("button", { name: "Change network settings" }));
		await userEvent.click(screen.getByRole("button", { name: "Save" }));

		expect(await screen.findByRole("alert")).toHaveTextContent("citpListen is not an address");
	});
});
