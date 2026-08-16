import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PatchedFixture } from "../../../api/types";
import { DynamicEditorSessionProvider } from "../../../features/dynamics/DynamicEditorSessionContext";
import { createDefaultDynamicDefinition } from "../../../windows/DynamicsWindow";
import {
	DynamicEditorTaskTabs,
	ParameterFamilyTabs,
} from "./ParameterFamilyTabs";
import type { ParameterController } from "./useParameterController";

describe("DynamicEditorTaskTabs", () => {
	afterEach(cleanup);

	it("shows encoder page position only when the active task has multiple pages", () => {
		const dynamic = createDefaultDynamicDefinition(201, "intensity", {
			definition: "dynamic-201",
			lane: "lane-intensity",
		});
		const { rerender } = render(
			<DynamicEditorSessionProvider>
				<DynamicEditorTaskTabs
					task="curves"
					onTask={vi.fn()}
					dynamic={dynamic}
					laneId="lane-intensity"
					onLane={vi.fn()}
					page={1}
				/>
			</DynamicEditorSessionProvider>,
		);

		expect(screen.getByRole("button", { name: "Lanes" })).toBeInTheDocument();
		expect(screen.queryByText(/1\/1/)).toBeNull();
		expect(
			screen.queryByRole("combobox", { name: "Dynamic lane" }),
		).not.toBeInTheDocument();

		rerender(
			<DynamicEditorSessionProvider>
				<DynamicEditorTaskTabs
					task="curves"
					onTask={vi.fn()}
					dynamic={dynamic}
					laneId="lane-intensity"
					onLane={vi.fn()}
					page={1}
					pageCount={2}
				/>
			</DynamicEditorSessionProvider>,
		);

		expect(
			screen.getByRole("button", { name: "Lanes (1/2)" }),
		).toBeInTheDocument();
	});

	it("cycles the active Lanes encoder page and resets another task to page one", () => {
		const dynamic = createDefaultDynamicDefinition(201, "intensity", {
			definition: "dynamic-201",
			lane: "lane-intensity",
		});
		const onTask = vi.fn();
		const onPage = vi.fn();
		const { rerender } = render(
			<DynamicEditorSessionProvider>
				<DynamicEditorTaskTabs
					task="curves"
					onTask={onTask}
					dynamic={dynamic}
					laneId="lane-intensity"
					onLane={vi.fn()}
					page={1}
					onPage={onPage}
					pageCount={2}
				/>
			</DynamicEditorSessionProvider>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Lanes (1/2)" }));
		expect(onTask).toHaveBeenLastCalledWith("curves");
		expect(onPage).toHaveBeenLastCalledWith(2);

		rerender(
			<DynamicEditorSessionProvider>
				<DynamicEditorTaskTabs
					task="curves"
					onTask={onTask}
					dynamic={dynamic}
					laneId="lane-intensity"
					onLane={vi.fn()}
					page={2}
					onPage={onPage}
					pageCount={2}
				/>
			</DynamicEditorSessionProvider>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Phase (1/2)" }));
		expect(onTask).toHaveBeenLastCalledWith("phase");
		expect(onPage).toHaveBeenLastCalledWith(1);
	});
});

function specialDialogController(
	family: "Beam" | "Media",
	attributes: readonly string[],
): ParameterController {
	const fixture = {
		fixture_id: "fixture-1",
		logical_heads: [],
		definition: {
			heads: [
				{
					shared: true,
					parameters: attributes.map((attribute) => ({ attribute })),
				},
			],
		},
	} as unknown as PatchedFixture;
	return {
		family,
		selectedFixtures: [fixture],
		selectedFixtureIds: [fixture.fixture_id],
		encoderGroups: [],
		encoderPage: 1,
		alignMode: null,
		state: { shiftArmed: false },
		dynamicsMode: false,
		programmerActions: null,
		selectEncoderGroup: vi.fn(),
		setAlignMode: vi.fn(),
		setDynamicsMode: vi.fn(),
		dispatch: vi.fn(),
	} as unknown as ParameterController;
}

describe("ParameterFamilyTabs special dialogs", () => {
	it("removes Beam and exposes Media only when Play Mode is advertised", () => {
		const { rerender } = render(
			<DynamicEditorSessionProvider>
				<ParameterFamilyTabs
					controller={specialDialogController("Beam", ["gobo"])}
				/>
			</DynamicEditorSessionProvider>,
		);
		expect(
			screen.queryByRole("button", { name: "Special Dialog" }),
		).not.toBeInTheDocument();

		rerender(
			<DynamicEditorSessionProvider>
				<ParameterFamilyTabs
					controller={specialDialogController("Media", [])}
				/>
			</DynamicEditorSessionProvider>,
		);
		expect(
			screen.queryByRole("button", { name: "Special Dialog" }),
		).not.toBeInTheDocument();

		rerender(
			<DynamicEditorSessionProvider>
				<ParameterFamilyTabs
					controller={specialDialogController("Media", ["media.play_mode"])}
				/>
			</DynamicEditorSessionProvider>,
		);
		expect(
			screen.getByRole("button", { name: "Special Dialog" }),
		).toBeInTheDocument();
	});
});
