import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { RendererSettingsWorkspace } from "./RendererSettingsWorkspace";
import { documentSession, type RendererSettings } from "./document/session";

vi.mock("./document/session", () => ({
	documentSession: {
		rendererSettings: vi.fn(),
		saveRendererSettings: vi.fn(),
	},
}));

let stored: RendererSettings;
beforeEach(() => {
	vi.clearAllMocks();
	stored = {
		source: "planning_software",
		host: "127.0.0.1",
		port: 5158,
		user: "Operator",
		quality: "draft",
		fog: 0.1,
		persistence: 0,
		persistenceFalloff: 3,
		ambient: 0.1,
		exposure: 1,
		laserBrightness: 1,
		lampFogCloudiness: 0.7,
		lampFogTurbulence: 1,
		laserFogCloudiness: 0,
		laserFogTurbulence: 0,
		crowdAmount: 1,
		detailedPeople: true,
		theme: "light_on_dark",
		background: null,
		showLabels: true,
		showSelection: true,
		floorGrid: false,
		blender: "",
		inputOverrides: [],
		artNetInterface: null,
		sacnInterface: null,
	};
	vi.mocked(documentSession.rendererSettings).mockImplementation(async () =>
		structuredClone(stored),
	);
	vi.mocked(documentSession.saveRendererSettings).mockImplementation(async (next) => {
		stored = structuredClone(next);
		return structuredClone(stored);
	});
});

it("keeps raw detailed people while saving another preference in Draft, and toggles it without changing quality", async () => {
	const onError = vi.fn();
	render(<RendererSettingsWorkspace onError={onError} />);
	const detail = await screen.findByRole("switch", { name: "Detailed people" });
	expect(detail).toBeEnabled();
	expect(detail).toBeChecked();
	fireEvent.click(screen.getByRole("switch", { name: "Show selection" }));
	await waitFor(() =>
		expect(documentSession.saveRendererSettings).toHaveBeenLastCalledWith(
			expect.objectContaining({
				quality: "draft",
				detailedPeople: true,
				showSelection: false,
			}),
		),
	);
	fireEvent.click(detail);
	await waitFor(() => expect(stored.detailedPeople).toBe(false));
	expect(stored.quality).toBe("draft");
	fireEvent.click(detail);
	await waitFor(() => expect(stored.detailedPeople).toBe(true));
	expect(stored.quality).toBe("draft");
	expect(onError).not.toHaveBeenCalled();
});

it("points to the actual native Open Viz menu and shortcut instead of creating an export action", async () => {
	render(<RendererSettingsWorkspace onError={vi.fn()} />);
	await screen.findByRole("switch", { name: "Detailed people" });
	expect(screen.getByText("Open Viz")).toBeInTheDocument();
	expect(screen.getByText("File > Render 4K Image")).toBeInTheDocument();
	expect(
		screen.getByText(/Cmd\+Shift\+R on macOS \/ Ctrl\+Shift\+R/),
	).toBeInTheDocument();
	expect(screen.queryByRole("button", { name: /Render 4K/i })).not.toBeInTheDocument();
});
