import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DynamicEditorSessionProvider } from "../../../features/dynamics/DynamicEditorSessionContext";
import { createDefaultDynamicDefinition } from "../../../windows/DynamicsWindow";
import { DynamicEditorTaskTabs } from "./ParameterFamilyTabs";

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

		expect(screen.getByRole("button", { name: "Curves" })).toBeInTheDocument();
		expect(screen.queryByText(/1\/1/)).toBeNull();

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
			screen.getByRole("button", { name: "Curves (1/2)" }),
		).toBeInTheDocument();
	});

	it("cycles the active Curves encoder page and resets another task to page one", () => {
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

		fireEvent.click(screen.getByRole("button", { name: "Curves (1/2)" }));
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
		fireEvent.click(screen.getByRole("button", { name: "Phase Spread" }));
		expect(onTask).toHaveBeenLastCalledWith("phase");
		expect(onPage).toHaveBeenLastCalledWith(1);
	});
});
