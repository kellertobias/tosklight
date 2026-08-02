import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { initialState } from "../../state/initialState";
import { WorkspaceView } from "./WorkspaceView";

const mocks = vi.hoisted(() => ({
	dispatch: vi.fn(),
	state: null as unknown as typeof initialState,
}));

vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: mocks.state, dispatch: mocks.dispatch }),
}));
vi.mock("../../windows/WindowRegistry", () => ({
	isRegisteredWindow: (kind: string) => kind !== "layout",
	windowRegistry: {},
}));
vi.mock("./DeskGrid", () => ({ DeskGrid: () => <div>Desktop content</div> }));

beforeEach(() => {
	mocks.dispatch.mockReset();
	mocks.state = initialState;
});

describe("WorkspaceView retired Layout notice", () => {
	it("points the operator to Group settings and Dynamics Projection and can be dismissed", () => {
		mocks.state = { ...initialState, layoutMigrationNotice: true };
		render(<WorkspaceView />);

		expect(
			screen.getByText(
				"Layout was removed. Spatial ordering now lives in Group settings and Dynamics Projection.",
			),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "DISMISS_LAYOUT_MIGRATION_NOTICE",
		});
	});
});
