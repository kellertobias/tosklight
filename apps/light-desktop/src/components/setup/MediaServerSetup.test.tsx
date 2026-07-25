import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MediaServerFixture, PatchedFixture } from "../../api/types";
import {
	blankFixtureProfile,
	fixtureDefinitionsFromProfiles,
} from "./fixtureProfileModel";
import { MediaServerSetup } from "./MediaServerSetup";

const mocks = vi.hoisted(() => ({
	status: "ready" as "loading" | "ready",
	updateFixture: vi.fn(),
	usePatchView: vi.fn(),
	putObject: vi.fn(),
	deleteObject: vi.fn(),
	refresh: vi.fn(),
	refreshMediaPreview: vi.fn(),
	refreshMediaThumbnails: vi.fn(),
}));

const server = {
	mediaServers: [] as MediaServerFixture[],
	mediaPreviewUrls: {},
	refreshMediaPreview: mocks.refreshMediaPreview,
	refreshMediaThumbnails: mocks.refreshMediaThumbnails,
	putObject: mocks.putObject,
	deleteObject: mocks.deleteObject,
	refresh: mocks.refresh,
};

let fixture: PatchedFixture;
let patchFixtures: readonly PatchedFixture[];

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../../features/patch/PatchContext", () => ({
	usePatch: () => ({
		status: mocks.status,
		fixtures: patchFixtures,
		error: null,
		updateFixture: mocks.updateFixture,
	}),
	usePatchView: mocks.usePatchView,
}));

beforeEach(() => {
	vi.clearAllMocks();
	mocks.status = "ready";
	mocks.updateFixture.mockResolvedValue(true);
	server.mediaServers = [];
	fixture = mediaFixture();
	patchFixtures = [fixture];
});

afterEach(cleanup);

describe("Media server Patch authority", () => {
	it("hides retained fixtures and refuses configuration while Patch loads", () => {
		mocks.status = "loading";
		render(<MediaServerSetup />);

		expect(screen.getByText("Patch authority loading…")).toBeInTheDocument();
		expect(screen.queryByRole("button", { name: "Disable CITP" })).toBeNull();
		expect(mocks.updateFixture).not.toHaveBeenCalled();
		expect(mocks.usePatchView).toHaveBeenCalledWith(true);
	});

	it("saves an endpoint through one typed Patch action and no generic mutation", async () => {
		render(<MediaServerSetup />);
		fireEvent.change(screen.getByLabelText("Acme Media One CITP IP address"), {
			target: { value: "192.168.1.50" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Save endpoint" }));

		await waitFor(() =>
			expect(mocks.updateFixture).toHaveBeenCalledWith("fixture-media", {
				direct_control: {
					protocol: "citp",
					ip_address: "192.168.1.50",
					port: 4811,
				},
			}),
		);
		expect(mocks.updateFixture).toHaveBeenCalledOnce();
		expect(mocks.putObject).not.toHaveBeenCalled();
		expect(mocks.deleteObject).not.toHaveBeenCalled();
		expect(mocks.refresh).not.toHaveBeenCalled();
	});

	it("disables CITP through one typed Patch action", async () => {
		fixture = {
			...fixture,
			direct_control: {
				protocol: "citp",
				ip_address: "192.168.1.60",
				port: 4811,
			},
		};
		patchFixtures = [fixture];
		render(<MediaServerSetup />);
		fireEvent.change(screen.getByLabelText("Acme Media One CITP IP address"), {
			target: { value: "" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Disable CITP" }));

		await waitFor(() =>
			expect(mocks.updateFixture).toHaveBeenCalledWith("fixture-media", {
				direct_control: null,
			}),
		);
		expect(mocks.updateFixture).toHaveBeenCalledOnce();
	});

	it("does not show stale online status for a replaced endpoint", () => {
		fixture = {
			...fixture,
			direct_control: {
				protocol: "citp",
				ip_address: "192.168.1.70",
				port: 4811,
			},
		};
		patchFixtures = [fixture];
		server.mediaServers = [
			{
				fixture_id: fixture.fixture_id,
				name: "Media One",
				endpoint: {
					protocol: "citp",
					ip_address: "192.168.1.60",
					port: 4811,
				},
				layers: [],
				status: {
					online: true,
					last_success: "2026-07-21T00:00:00Z",
					last_error: null,
				},
			},
		];

		render(<MediaServerSetup />);

		expect(screen.getByText("● Offline")).toBeInTheDocument();
		expect(screen.queryByText("● Online")).toBeNull();
		expect(
			screen.getByText(/No successful response yet/),
		).toBeInTheDocument();
	});
});

function mediaFixture(): PatchedFixture {
	const profile = blankFixtureProfile();
	profile.id = "profile-media";
	profile.revision = 2;
	profile.manufacturer = "Acme";
	profile.name = "Media One";
	profile.short_name = "Media One";
	profile.direct_control_protocols = ["citp"];
	profile.modes[0].id = "mode-media";
	const definition = fixtureDefinitionsFromProfiles([profile])[0];
	return {
		fixture_id: "fixture-media",
		fixture_number: 1,
		virtual_fixture_number: null,
		name: "Media One",
		definition,
		universe: 1,
		address: 1,
		split_patches: [{ split: 1, universe: 1, address: 1 }],
		layer_id: "default",
		direct_control: null,
		location: { x: 0, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
		logical_heads: [],
		multipatch: [],
		move_in_black_enabled: true,
		move_in_black_delay_millis: 0,
		highlight_overrides: {},
	};
}
