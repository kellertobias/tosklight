import { WindowHeader } from "@tosklight/ui/window-kit";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DesktopProvider } from "../../platform/desktop";
import { browserDesktopBridge } from "../../platform/desktop/browserDesktopBridge";
import { initialState } from "../../state/initialState";
import { WorkspaceView } from "./WorkspaceView";

const mocks = vi.hoisted(() => ({
	dispatch: vi.fn(),
	action: vi.fn(),
	state: null as unknown as typeof initialState,
}));

vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: mocks.state, dispatch: mocks.dispatch }),
}));
vi.mock("../../windows/WindowRegistry", () => ({
	isRegisteredWindow: (kind: string) => kind !== "layout",
	windowRegistry: {
		groups: () => <TestWindow />,
	},
}));
vi.mock("./DeskGrid", () => ({ DeskGrid: () => <TestWindow /> }));

function TestWindow() {
	return (
		<div>
			<WindowHeader
				title={<span>Group Pool</span>}
				info={{ primary: "2 fixtures selected" }}
				search={{ value: "", onSearch: () => undefined }}
				groups={[
					{
						id: "actions",
						actions: [{ id: "edit", label: "Edit", onPress: mocks.action }],
					},
				]}
			/>
			<div>Desktop content</div>
			<WindowHeader title="Nested dialog" />
		</div>
	);
}

afterEach(cleanup);

beforeEach(() => {
	mocks.dispatch.mockReset();
	mocks.action.mockReset();
	mocks.state = initialState;
});

describe("built-in native title dragging", () => {
	function nativeWorkspace(fullscreen = false, available = true) {
		const startCurrentWindowDrag = vi.fn().mockResolvedValue(undefined);
		const currentWindowFullscreen = vi.fn().mockResolvedValue(fullscreen);
		const view = render(
			<DesktopProvider
				bridge={{
					...browserDesktopBridge,
					available,
					startCurrentWindowDrag,
					currentWindowFullscreen,
				}}
			>
				<WorkspaceView />
			</DesktopProvider>,
		);
		return { ...view, startCurrentWindowDrag, currentWindowFullscreen };
	}

	beforeEach(() => {
		mocks.state = { ...initialState, builtIn: "groups" };
	});

	it.each(["title", "status", "spacer", "background"])(
		"moves the native window from the %s",
		async (part) => {
			const { container, startCurrentWindowDrag } = nativeWorkspace();
			const targets = {
				title: screen.getByText("Group Pool"),
				status: screen.getByText("2 fixtures selected"),
				spacer: container.querySelector(".ui-window-header-spacer")!,
				background: container.querySelector(".ui-window-header")!,
			};
			fireEvent.pointerDown(targets[part as keyof typeof targets], { button: 0 });
			await waitFor(() => expect(startCurrentWindowDrag).toHaveBeenCalledOnce());
		},
	);

	it("preserves actions, search and content without querying native state", () => {
		const { container, currentWindowFullscreen } = nativeWorkspace();
		const action = screen.getByRole("button", { name: "Edit" });
		for (const target of [
			action,
			screen.getByRole("textbox"),
			screen.getByText("Desktop content"),
			screen.getByText("Nested dialog"),
			container.querySelector(".ui-window-action-groups")!,
		]) {
			fireEvent.pointerDown(target, { button: 0 });
		}
		fireEvent.click(action);
		expect(mocks.action).toHaveBeenCalledOnce();
		expect(currentWindowFullscreen).not.toHaveBeenCalled();
	});

	it("checks fullscreen again after a mode change", async () => {
		const { currentWindowFullscreen, startCurrentWindowDrag } = nativeWorkspace(true);
		fireEvent.pointerDown(screen.getByText("Group Pool"), { button: 0 });
		await waitFor(() => expect(currentWindowFullscreen).toHaveBeenCalledOnce());
		expect(startCurrentWindowDrag).not.toHaveBeenCalled();
		currentWindowFullscreen.mockResolvedValue(false);
		fireEvent.pointerDown(screen.getByText("Group Pool"), { button: 0 });
		await waitFor(() => expect(startCurrentWindowDrag).toHaveBeenCalledOnce());
	});

	it("ignores secondary buttons", () => {
		const { currentWindowFullscreen } = nativeWorkspace();
		fireEvent.pointerDown(screen.getByText("Group Pool"), { button: 2 });
		expect(currentWindowFullscreen).not.toHaveBeenCalled();
	});

	it("does not call native APIs in a browser", () => {
		const { currentWindowFullscreen } = nativeWorkspace(false, false);
		fireEvent.pointerDown(screen.getByText("Group Pool"), { button: 0 });
		expect(currentWindowFullscreen).not.toHaveBeenCalled();
	});

	it("leaves Desktop pane headers outside native dragging", () => {
		mocks.state = { ...initialState, builtIn: null };
		const { currentWindowFullscreen } = nativeWorkspace();
		fireEvent.pointerDown(screen.getByText("Group Pool"), { button: 0 });
		expect(currentWindowFullscreen).not.toHaveBeenCalled();
	});
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
