import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AppProvider } from "../../../state/AppContext";
import { createDefaultDynamicDefinition } from "../../../windows/DynamicsWindow";
import { DynamicDefinitionEncoderSurface } from "./ProgrammerDynamicsSurface";

let hardwareConnected = false;

vi.mock("../../../features/deskSnapshot/DeskSnapshotState", () => ({
	useActiveShowId: () => "show-test",
	useHardwareConnected: () => hardwareConnected,
}));

describe("DynamicDefinitionEncoderSurface", () => {
	afterEach(cleanup);
	beforeEach(() => {
		hardwareConnected = false;
		Object.defineProperty(window, "localStorage", {
			configurable: true,
			value: {
				getItem: vi.fn(() => null),
				setItem: vi.fn(),
				removeItem: vi.fn(),
				clear: vi.fn(),
			},
		});
	});

	it("renders the reviewed six-slot Curves deck on software and hardware surfaces", () => {
		const dynamic = createDefaultDynamicDefinition(201, "intensity", {
			definition: "dynamic-201",
			lane: "lane-intensity",
		});
		const props = {
			dynamic,
			lane: dynamic.lanes[0] ?? null,
			view: "curves" as const,
			page: 1,
			onLaneChange: vi.fn().mockResolvedValue(undefined),
			onMutate: vi.fn().mockResolvedValue(undefined),
		};
		const { container, rerender } = render(
			<AppProvider>
				<DynamicDefinitionEncoderSurface {...props} />
			</AppProvider>,
		);

		expect(screen.getByLabelText("Curves encoders")).toHaveAttribute(
			"data-encoder-surface",
			"touch",
		);
		expect(container.querySelectorAll(".touch-encoder")).toHaveLength(6);
		expect(
			screen.getByRole("group", { name: "Enc 1 · Function" }),
		).toBeInTheDocument();

		hardwareConnected = true;
		rerender(
			<AppProvider>
				<DynamicDefinitionEncoderSurface {...props} />
			</AppProvider>,
		);

		expect(screen.getByLabelText("Curves encoders")).toHaveAttribute(
			"data-encoder-surface",
			"hardware",
		);
		expect(container.querySelectorAll(".hardware-encoder-display")).toHaveLength(6);
		expect(
			screen.getByLabelText("Encoder 1: Function, Sinus"),
		).toBeInTheDocument();
	});
});
