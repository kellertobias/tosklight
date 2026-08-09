import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PaneModel } from "../../types";
import { Pane } from "./Pane";

const selectionView = vi.hoisted(() =>
	vi.fn((enabled: boolean) =>
		enabled ? { selected: ["fixture-a", "fixture-b"] } : null,
	),
);
const commandLineView = vi.hoisted(() =>
	vi.fn<() => { text: string } | null>(() => null),
);
const commandLineReset = vi.hoisted(() => vi.fn(() => Promise.resolve(true)));
const dispatch = vi.hoisted(() => vi.fn());

vi.mock(
	"../../features/programmingInteraction/ProgrammingInteractionView",
	() => ({
		useProgrammingSelectionView: selectionView,
		useProgrammingCommandLineView: commandLineView,
		useProgrammingCommandLineActions: () => ({ reset: commandLineReset }),
	}),
);
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({
		state: {
			stageMode: "select",
			stageView: "2d",
			presetFamily: "Intensity",
		},
		dispatch,
	}),
}));
vi.mock("../../windows/WindowRegistry", () => ({
	isRegisteredWindow: (kind: string) => kind !== "layout",
	windowRegistry: {
		stage: () => <div>Stage body</div>,
		fixtures: () => <div>Fixture body</div>,
		groups: () => <div>Group body</div>,
		file_manager: () => <div>File Manager body</div>,
	},
}));
vi.mock("@tosklight/ui", async (importOriginal) => {
	const actual = await importOriginal<typeof import("@tosklight/ui")>();
	return { ...actual, Button: () => null };
});
vi.mock("../shared/SourceLegend", () => ({ SourceLegend: () => null }));
vi.mock("@tosklight/ui/window-kit", async (importOriginal) => {
	const actual =
		await importOriginal<typeof import("@tosklight/ui/window-kit")>();
	return {
		...actual,
		WindowHeader: ({
			info,
			onTitleClick,
			titleActionLabel,
			settings,
			onSettings,
		}: {
			info?: { primary: React.ReactNode };
			onTitleClick?: () => void;
			titleActionLabel?: string;
			settings?: boolean;
			onSettings?: () => void;
		}) => (
			<header>
				{info?.primary}
				{onTitleClick && (
					<button
						type="button"
						className="ui-window-title ui-window-title-action"
						aria-label={titleActionLabel}
						onClick={onTitleClick}
					>
						Title action
					</button>
				)}
				{settings && (
					<button type="button" onClick={onSettings}>
						Settings
					</button>
				)}
			</header>
		),
	};
});
vi.mock("./PaneChromeContext", () => ({
	PaneChromeProvider: ({ children }: React.PropsWithChildren) => children,
}));

function pane(kind: PaneModel["kind"]): PaneModel {
	return {
		id: kind,
		kind,
		title: kind,
		x: 1,
		y: 1,
		width: 12,
		height: 18,
	};
}

afterEach(() => {
	cleanup();
	selectionView.mockClear();
	commandLineView.mockReset();
	commandLineView.mockReturnValue(null);
	commandLineReset.mockClear();
	dispatch.mockClear();
});

describe("Pane selection scope", () => {
	it("observes selection only for an active Stage or Fixture pane", () => {
		const rendered = render(
			<Pane pane={pane("stage")} active maximized={false} editing={false} />,
		);
		expect(
			rendered.container.querySelector('[data-ui-component="pane-view"]'),
		).toBeInTheDocument();
		expect(selectionView).toHaveBeenLastCalledWith(true);
		expect(screen.getByText("2 selected")).toBeInTheDocument();

		rendered.rerender(
			<Pane
				pane={pane("fixtures")}
				active={false}
				maximized={false}
				editing={false}
			/>,
		);
		expect(selectionView).toHaveBeenLastCalledWith(false);
		expect(screen.getByText("0 selected")).toBeInTheDocument();

		rendered.rerender(
			<Pane pane={pane("groups")} active maximized={false} editing={false} />,
		);
		expect(selectionView).toHaveBeenLastCalledWith(false);
	});

	it("removes the pane from its title and clears the shared command when DELETE is active", () => {
		commandLineView.mockReturnValue({ text: "DELETE" });
		render(
			<Pane
				pane={pane("file_manager")}
				active
				maximized={false}
				editing={false}
			/>,
		);

		const title = screen.getByRole("button", {
			name: "Remove File Manager pane",
		});
		expect(title).toHaveClass("ui-window-title-action");
		fireEvent.click(title);

		expect(dispatch).toHaveBeenCalledWith({
			type: "REMOVE_PANE",
			id: "file_manager",
		});
		expect(commandLineReset).toHaveBeenCalledOnce();
	});

	it.each([
		"COPY",
		"MOVE",
		"RECORD",
		"SET",
		"DELETE PANE 1",
	])("does not outline or activate a Pane title for unsupported %s targeting", (command) => {
		commandLineView.mockReturnValue({ text: command });
		const rendered = render(
			<Pane pane={pane("groups")} active maximized={false} editing={false} />,
		);

		expect(
			rendered.container.querySelector(".ui-window-title-action"),
		).not.toBeInTheDocument();
		expect(screen.queryByText("DELETE")).not.toBeInTheDocument();
	});

	it("uses the shared DEL alias as the supported Pane Delete target", () => {
		commandLineView.mockReturnValue({ text: "DEL" });
		render(
			<Pane pane={pane("groups")} active maximized={false} editing={false} />,
		);

		expect(
			screen.getByRole("button", { name: "Remove groups pane" }),
		).toBeInTheDocument();
	});

	it("keeps Pane body and Settings navigation ordinary while Delete is armed", () => {
		commandLineView.mockReturnValue({ text: "DELETE" });
		render(
			<Pane pane={pane("groups")} active maximized={false} editing={false} />,
		);

		fireEvent.click(screen.getByText("Group body"));
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));

		expect(dispatch).toHaveBeenCalledTimes(1);
		expect(dispatch).toHaveBeenCalledWith({
			type: "SET_PANE_SETTINGS",
			id: "groups",
		});
		expect(commandLineReset).not.toHaveBeenCalled();
	});
});
