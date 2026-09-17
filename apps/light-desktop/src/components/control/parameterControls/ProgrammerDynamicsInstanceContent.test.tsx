import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { useDynamicEditorSession } from "../../../features/dynamics/DynamicEditorSessionContext";
import {
	type DynamicControllerChoice,
	ProgrammerDynamicsInstanceContent,
} from "./ProgrammerDynamicsInstanceContent";
import type { ParameterController } from "./useParameterController";
import { VisibleEncoderCountProvider } from "./VisibleEncoderCount";

describe("ProgrammerDynamicsInstanceContent", () => {
	afterEach(cleanup);

	it("paginates six semantic instance controls across four software encoders", () => {
		const selectEncoderGroup = vi.fn();
		const { container, rerender } = renderContent({
			visibleEncoderCount: 4,
			encoderPage: 1,
			selectEncoderGroup,
		});

		expect(container.querySelectorAll(".touch-encoder")).toHaveLength(4);
		expect(
			screen.getByRole("group", { name: "Enc 4 · Instance Speed" }),
		).toBeInTheDocument();
		expect(screen.queryByText("Dynamic Off")).not.toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", { name: "Next instance encoder page" }),
		);
		expect(selectEncoderGroup).toHaveBeenCalledWith("Intensity", 2);

		rerenderContent(rerender, {
			visibleEncoderCount: 4,
			encoderPage: 2,
			selectEncoderGroup,
		});
		expect(container.querySelectorAll(".touch-encoder")).toHaveLength(1);
		expect(
			screen.getByRole("group", { name: "Enc 1 · Instance Phase" }),
		).toBeInTheDocument();
		expect(screen.getByText("Dynamic Off")).toBeInTheDocument();
		expect(screen.getByText("Page 2 of 2")).toBeInTheDocument();
	});

	it("keeps the full six-control instance deck on six-encoder surfaces", () => {
		const { container } = renderContent({
			visibleEncoderCount: 6,
			encoderPage: 2,
			selectEncoderGroup: vi.fn(),
		});

		expect(container.querySelectorAll(".touch-encoder")).toHaveLength(5);
		expect(
			screen.getByRole("group", { name: "Enc 5 · Instance Phase" }),
		).toBeInTheDocument();
		expect(screen.getByText("Dynamic Off")).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "Next instance encoder page" }),
		).not.toBeInTheDocument();
	});

	it("keeps hardware instance controls at six slots when software uses four", () => {
		const { container } = renderContent({
			visibleEncoderCount: 4,
			encoderPage: 2,
			hardwareConnected: true,
			selectEncoderGroup: vi.fn(),
		});

		expect(
			container.querySelectorAll(".hardware-encoder-display"),
		).toHaveLength(6);
		expect(
			screen.getByLabelText("Encoder 6: Dynamic Off, Press"),
		).toBeInTheDocument();
		expect(screen.queryByText("Page 2 of 2")).not.toBeInTheDocument();
	});

	it("drops the lane label on Speed, where no lane choice applies", () => {
		const options = {
			visibleEncoderCount: 6 as const,
			encoderPage: 1,
			selectEncoderGroup: vi.fn(),
			lanes: [{ id: "lane-1", attribute: "Intensity" }],
		};
		const { rerender } = renderContent(options);
		expect(
			screen.getByRole("button", { name: "Dynamic lane" }),
		).toBeInTheDocument();

		rerenderContent(rerender, { ...options, view: "speed" });
		expect(
			screen.queryByRole("button", { name: "Dynamic lane" }),
		).not.toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Dynamic instance" }),
		).toBeInTheDocument();
		for (const name of ["Instance", "Lanes", "Phase", "Speed"])
			expect(screen.getByRole("button", { name })).toBeInTheDocument();
	});
});

interface RenderOptions {
	visibleEncoderCount: 4 | 6;
	encoderPage: number;
	view?: "instance" | "speed";
	lanes?: { id: string; attribute: string }[];
	hardwareConnected?: boolean;
	selectEncoderGroup: ReturnType<typeof vi.fn>;
}

function renderContent(options: RenderOptions) {
	return render(content(options));
}

function rerenderContent(
	rerender: ReturnType<typeof render>["rerender"],
	options: RenderOptions,
) {
	rerender(content(options));
}

function content({
	visibleEncoderCount,
	encoderPage,
	view = "instance",
	lanes = [],
	hardwareConnected = false,
	selectEncoderGroup,
}: RenderOptions) {
	const selected = {
		instance: {
			pool_number: 1,
			name: "Pulse",
			paused: false,
			pending: false,
		},
		controller: {
			controller_id: "controller-1",
			source: "Programmer",
			winning: true,
			paused: false,
			size: 1,
			speed_multiplier: 1,
			phase_offset_degrees: 0,
		},
		definition: null,
	} as unknown as DynamicControllerChoice;
	const controller = {
		encoderPage,
		family: "Intensity",
		hardwareConnected,
		selectEncoderGroup,
	} as unknown as ParameterController;
	return (
		<VisibleEncoderCountProvider count={visibleEncoderCount}>
			<ProgrammerDynamicsInstanceContent
				controller={controller}
				editor={null as unknown as ReturnType<typeof useDynamicEditorSession>}
				choices={[selected]}
				selected={selected}
				selectedLane={(lanes[0] ?? null) as never}
				lanes={lanes as never}
				selectedObject={undefined}
				presets={[]}
				view={view}
				error={null}
				onView={vi.fn()}
				onController={vi.fn()}
				onLane={vi.fn()}
				onCycleChoice={vi.fn()}
				onCycleLane={vi.fn()}
				onUpdate={vi.fn().mockResolvedValue(undefined)}
				onOff={vi.fn().mockResolvedValue(undefined)}
				onLaneChange={vi.fn().mockResolvedValue(undefined)}
				onMutate={vi.fn().mockResolvedValue(undefined)}
			/>
		</VisibleEncoderCountProvider>
	);
}
