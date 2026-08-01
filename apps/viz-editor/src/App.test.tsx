import { ModalProvider } from "@tosklight/ui/modals";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";

/** One profile, shaped as the document core returns it. */
const PROFILE_ID = "11111111-1111-4111-8111-111111111111";
const MODE_ID = "22222222-2222-4222-8222-222222222222";
const SHOW_ID = "33333333-3333-4333-8333-333333333333";

const document = {
	showId: SHOW_ID,
	name: "Planning show",
	path: "/tmp/planning.show",
	fixtureCount: 0,
};

const snapshot = {
	showId: SHOW_ID,
	showRevision: 1,
	patchRevision: 1,
	cursor: 0,
	fixtures: [],
	profileRevisions: [],
};

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
	open: vi.fn(),
	save: vi.fn(),
}));

function libraryProfile() {
	return {
		id: PROFILE_ID,
		revision: 1,
		manufacturer: "Acme",
		name: "Planning Wash",
		// Shaped as `light_fixture::FixtureProfile` serializes, which is what the command returns.
		profile: {
			schema_version: 2,
			id: PROFILE_ID,
			revision: 1,
			manufacturer: "Acme",
			name: "Planning Wash",
			short_name: "Wash",
			fixture_type: "wash",
			patch_policy: "dmx",
			notes: "",
			physical: {},
			hazardous: false,
			model_asset: null,
			stage_icon_asset: null,
			direct_control_protocols: [],
			signal_loss_policy: "hold",
			modes: [
				{
					id: MODE_ID,
					name: "Default",
					notes: "",
					splits: [{ number: 1, footprint: 1 }],
					heads: [],
					channels: [],
					color_systems: [],
				},
			],
		},
	};
}

function renderApp(children: ReactNode = <App />) {
	return render(<ModalProvider>{children}</ModalProvider>);
}

beforeEach(() => {
	invoke.mockReset();
	invoke.mockImplementation((command: string) => {
		switch (command) {
			case "document_summary":
				return Promise.resolve(document);
			case "library_profiles":
				return Promise.resolve([libraryProfile()]);
			case "patch_snapshot":
				return Promise.resolve(snapshot);
			case "discovered_desks":
				return Promise.resolve([]);
			case "patch_layers":
				return Promise.resolve([
					{ id: "house", name: "House", order: 0 },
					{ id: "floor", name: "Floor", order: 1 },
				]);
			default:
				return Promise.reject(new Error(`unexpected command ${command}`));
		}
	});
});

describe("the Viz editor window", () => {
	it("shows the desk's patch sheet over the open document", async () => {
		renderApp();
		// The sheet's own header, rendered by the shared window kit.
		expect(await screen.findByText("Show Patch")).toBeInTheDocument();
		expect(
			await screen.findByText("0 fixtures · 2 layers"),
		).toBeInTheDocument();
		expect(
			screen.getByRole("heading", { name: "Planning show" }),
		).toBeInTheDocument();
	});

	it("carries no desk furniture", async () => {
		renderApp();
		await screen.findByText("Show Patch");

		// The three things this window deliberately does not have.
		expect(
			screen.queryByRole("button", { name: /Preview Stage/i }),
			"no second renderer: the visualizer window is the picture",
		).not.toBeInTheDocument();
		expect(document_root()?.querySelector(".encoder-frame")).toBeNull();
		expect(document_root()?.querySelector(".command-line")).toBeNull();
	});

	it("offers the file actions the planning workflow needs", async () => {
		renderApp();
		await screen.findByText("Show Patch");
		for (const label of ["New", "Open", "Save As", "Import MVR", "Export MVR"]) {
			expect(screen.getByRole("button", { name: label })).toBeInTheDocument();
		}
	});

	it("shows the layers the document itself carries", async () => {
		renderApp();
		// The sheet counts the layers it was given: a document written on a desk arrives with
		// its own, and its fixtures belong to them.
		expect(await screen.findByText("0 fixtures · 2 layers")).toBeInTheDocument();
	});

	it("offers the desk it finds on the network, and opens what that desk sends", async () => {
		const found = [
			{
				instance: "desk-foh",
				name: "front-of-house",
				show: "Summer Tour",
				address: "10.0.0.4:5000",
			},
		];
		const loaded = { ...document, name: "Summer Tour", path: "/tmp/Summer Tour.show" };
		invoke.mockImplementation((command: string) => {
			switch (command) {
				case "discovered_desks":
					return Promise.resolve(found);
				case "load_from_desk":
					return Promise.resolve(loaded);
				case "document_summary":
					return Promise.resolve(document);
				case "patch_snapshot":
					return Promise.resolve(snapshot);
				default:
					return Promise.resolve([]);
			}
		});
		renderApp();
		const load = await screen.findByRole("button", {
			name: "Load from Desk · front-of-house: Summer Tour",
		});
		expect(load).toHaveAttribute("title", "front-of-house at 10.0.0.4:5000");

		fireEvent.click(load);
		await waitFor(() =>
			expect(invoke).toHaveBeenCalledWith("load_from_desk", {
				instance: "desk-foh",
			}),
		);
		expect(
			await screen.findByText("Loaded Summer Tour from front-of-house"),
		).toBeInTheDocument();
	});

	it("offers no desk when there is none on the network", async () => {
		renderApp();
		await screen.findByText("Show Patch");
		expect(screen.queryByText(/Load from Desk/)).not.toBeInTheDocument();
	});

	it("waits for a document before mounting the sheet", async () => {
		invoke.mockImplementation((command: string) =>
			command === "document_summary"
				? Promise.resolve(null)
				: Promise.resolve([]),
		);
		renderApp();
		expect(
			await screen.findByRole("heading", { name: "No show open" }),
		).toBeInTheDocument();
		expect(screen.queryByText("Show Patch")).not.toBeInTheDocument();
	});

	it("reports a document that will not open instead of failing silently", async () => {
		invoke.mockImplementation((command: string) =>
			command === "document_summary"
				? Promise.reject(new Error("show file is not readable"))
				: Promise.resolve([]),
		);
		renderApp();
		await waitFor(() =>
			expect(screen.getByText(/show file is not readable/i)).toBeInTheDocument(),
		);
	});
});

function document_root() {
	return globalThis.document.body;
}
