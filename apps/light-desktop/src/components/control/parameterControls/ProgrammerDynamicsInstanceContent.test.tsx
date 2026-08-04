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
});

interface RenderOptions {
	visibleEncoderCount: 4 | 6;
	encoderPage: number;
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
				selectedLane={null}
				lanes={[]}
				selectedObject={undefined}
				presets={[]}
				view="instance"
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
