import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AppProvider } from "../state/AppContext";
import {
	createDefaultDynamicDefinition,
	DynamicsWindow,
} from "./DynamicsWindow";

let dynamics: Array<Record<string, unknown>> = [];
let deleteArmed = false;
const deleteDynamic = vi.fn();
const resetCommand = vi.fn();

vi.mock("../features/showObjects/ShowObjectsState", () => ({
	useDynamics: () => dynamics,
	usePresets: () => [],
}));
vi.mock("../features/showObjects/ShowObjectsView", () => ({
	useShowObjectView: () => undefined,
}));
vi.mock("../features/deskSnapshot/DeskSnapshotState", () => ({
	useActiveShowId: () => "show-test",
	useAttributeRegistry: () => [
		{
			id: "intensity",
			label: "Intensity",
			family: "Intensity",
			recordable: true,
			value_type: "continuous",
			normalized_min: 0,
			normalized_max: 1,
		},
	],
	useHardwareConnected: () => false,
}));
vi.mock("../features/dynamics/DynamicsActionsContext", () => ({
	useDynamicsActions: () => ({
		dynamics: {
			runtime: vi.fn().mockResolvedValue({
				global_paused: false,
				instances: [],
				definitions: [],
			}),
			toggle: vi.fn(),
		},
		showObjects: {
			deleteDynamic,
		},
	}),
}));
vi.mock("../components/control/commandLine/useCommandLineSurface", () => ({
	useCommandLineSurface: () => ({
		selected: [],
		selectedGroupId: null,
		read: () => ({ text: "" }),
		reset: resetCommand,
		replace: vi.fn(),
	}),
}));
vi.mock(
	"../features/programmingInteraction/ProgrammingInteractionView",
	() => ({
		useProgrammingCommandLineActions: () => ({ reset: resetCommand }),
		useProgrammingDeleteCommandActive: () => deleteArmed,
	}),
);

function renderWindow() {
	return render(
		<AppProvider>
			<DynamicsWindow compact={false} />
		</AppProvider>,
	);
}

function dynamicObject() {
	const body = createDefaultDynamicDefinition(1, "intensity", {
		definition: "dynamic-1",
		lane: "lane-1",
	});
	return {
		kind: "dynamic",
		id: "dynamic-1",
		revision: 3,
		updated_at: "2026-07-27T00:00:00.000Z",
		body: { ...body, name: "Pulse" },
	};
}

describe("DynamicsWindow", () => {
	afterEach(cleanup);
	beforeEach(() => {
		Object.defineProperty(window, "localStorage", {
			configurable: true,
			value: {
				getItem: vi.fn(() => null),
				setItem: vi.fn(),
				removeItem: vi.fn(),
				clear: vi.fn(),
			},
		});
		dynamics = [];
		deleteArmed = false;
		deleteDynamic.mockReset().mockResolvedValue(undefined);
		resetCommand.mockReset().mockResolvedValue(true);
	});

	it("uses the shared pool without pagination or implementation legend", () => {
		renderWindow();

		expect(screen.getByText("Dynamics")).toBeInTheDocument();
		expect(screen.queryByText(/Immediate, server-authoritative/i)).toBeNull();
		expect(screen.queryByLabelText(/Dynamic pool page/i)).toBeNull();
		expect(screen.getAllByText("Empty").length).toBeGreaterThan(0);
	});

	it("opens a populated Dynamic for editing from right click", () => {
		dynamics = [dynamicObject()];
		renderWindow();

		fireEvent.contextMenu(screen.getByRole("button", { name: /Pulse/i }));

		expect(
			screen.getByRole("button", { name: /Back to Pool/ }),
		).toBeInTheDocument();
		expect(
			screen.getAllByText("Intensity", { selector: "strong" }),
		).toHaveLength(1);
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		expect(
			screen.getByRole("dialog", { name: "Dynamic Settings" }),
		).toBeInTheDocument();
		expect(screen.getByLabelText("Name")).toHaveValue("Pulse");
		expect(screen.queryByRole("button", { name: "Take Selection" })).toBeNull();
		fireEvent.click(screen.getByRole("tab", { name: "Targets" }));
		expect(
			screen.getByRole("button", { name: "Take Selection" }),
		).toBeInTheDocument();
	});

	it("opens the standard creation modal before choosing the first lane", () => {
		renderWindow();

		fireEvent.click(screen.getAllByRole("button", { name: /Empty/i })[0]);

		expect(
			screen.getByRole("dialog", { name: "Create Dynamic 1" }),
		).toBeInTheDocument();
		expect(screen.getByText("Choose the first lane")).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Create and edit" }),
		).toBeInTheDocument();
	});

	it("deletes an occupied Dynamic when Delete is armed", async () => {
		dynamics = [dynamicObject()];
		deleteArmed = true;
		renderWindow();

		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }));

		expect(deleteDynamic).toHaveBeenCalledWith("show-test", "dynamic-1", 3);
		await vi.waitFor(() => expect(resetCommand).toHaveBeenCalled());
	});
});
