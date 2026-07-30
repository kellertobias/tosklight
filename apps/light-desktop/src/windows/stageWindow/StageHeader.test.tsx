import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { StageLayoutApiClient } from "../../api/client/stageLayout";
import { StageLayoutActionsProvider } from "../../features/stageLayout/StageLayoutActions";
import { StageHeader } from "./StageHeader";
import type { StageLayoutModel, StageOptionsModel } from "./types";

const app = vi.hoisted(() => ({
	state: {
		stageShowSelection: true,
		stageShowFloorGrid: true,
		stageShowBeamGuides: true,
		stageEnvironmentBrightness: 1,
	},
	dispatch: vi.fn(),
}));

vi.mock("../../state/AppContext", () => ({ useApp: () => app }));

const options: StageOptionsModel = {
	mode: "select",
	setMode: vi.fn(),
	view: "2d",
	setView: vi.fn(),
	followPreload: false,
	toggleFollowPreload: vi.fn(),
	groupsVisible: false,
	showSelection: true,
	showFloorGrid: true,
	showBeamGuides: true,
	renderQuality: "lines_and_beams",
	environmentBrightness: 1,
};

const layout: StageLayoutModel = {
	positions: {},
	positions3d: {},
	positions2dConfig: {
		provenance: "automatic",
		projection: "front_to_back",
	},
};

function renderHeader({
	regenerate2d = vi.fn(async () => ({
		request_id: "request",
		revision: 2,
		moved_fixture_ids: [],
		replayed: false,
		changed: true,
	})),
	writable = true,
	canWrite = true,
	stageOptions = options,
}: {
	regenerate2d?: StageLayoutApiClient["regenerate2d"];
	writable?: boolean;
	canWrite?: boolean;
	stageOptions?: StageOptionsModel;
} = {}) {
	const client = { regenerate2d } as unknown as StageLayoutApiClient;
	return {
		regenerate2d,
		...render(
			<StageLayoutActionsProvider
				client={client}
				showId="show-26"
				canWrite={canWrite}
			>
				<StageHeader
					layout={layout}
					options={stageOptions}
					selectedCount={0}
					writable={writable}
				/>
			</StageLayoutActionsProvider>,
		),
	};
}

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

describe("Stage automatic 2D settings", () => {
	it("puts shortcuts and selection above View and shows render controls only in 3D", () => {
		renderHeader();
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		const dialog = screen.getByRole("dialog", { name: "Stage Settings" });
		const groups = within(dialog).getByText("Group shortcuts", {
			selector: "label",
		});
		const selection = within(dialog).getByText("Show selection", {
			selector: "label",
		});
		const view = within(dialog).getByText("View", { selector: "label" });
		expect(
			groups.compareDocumentPosition(view) & Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
		expect(
			selection.compareDocumentPosition(view) &
				Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
		for (const label of [
			"Floor grid",
			"Beam direction guidelines",
			"Render quality",
			"Environment brightness",
		])
			expect(within(dialog).queryByText(label)).not.toBeInTheDocument();

		cleanup();
		renderHeader({ stageOptions: { ...options, view: "3d" } });
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		const settings = screen.getByRole("dialog", { name: "Stage Settings" });
		for (const label of [
			"Floor grid",
			"Beam direction guidelines",
			"Render quality",
			"Environment brightness",
		])
			expect(within(settings).getByText(label)).toBeVisible();
	});

	it("shows provenance and intentionally regenerates with the selected projection", async () => {
		const { regenerate2d } = renderHeader();
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		const dialog = screen.getByRole("dialog", { name: "Stage Settings" });
		expect(within(dialog).getByText(/Automatic · Front to Back/)).toBeVisible();

		fireEvent.click(within(dialog).getByRole("button", { name: "Front to Back" }));
		fireEvent.click(screen.getByRole("option", { name: "Left to Right" }));
		fireEvent.click(
			within(dialog).getByRole("button", { name: "Regenerate 2D layout" }),
		);

		await waitFor(() =>
			expect(regenerate2d).toHaveBeenCalledWith("show-26", "left_to_right"),
		);
	});

	it("does not expose regeneration on a view-only or secondary surface", () => {
		renderHeader({ writable: false });
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		expect(
			screen.queryByRole("button", { name: "Regenerate 2D layout" }),
		).not.toBeInTheDocument();
		cleanup();

		renderHeader({ canWrite: false });
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		expect(
			screen.queryByRole("button", { name: "Regenerate 2D layout" }),
		).not.toBeInTheDocument();
	});

	it("surfaces regeneration errors and restores the action", async () => {
		renderHeader({
			regenerate2d: vi.fn(async () => {
				throw new Error("Stage layout revision changed");
			}),
		});
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		fireEvent.click(screen.getByRole("button", { name: "Regenerate 2D layout" }));

		expect(await screen.findByRole("alert")).toHaveTextContent(
			"Stage layout revision changed",
		);
		expect(
			screen.getByRole("button", { name: "Regenerate 2D layout" }),
		).toBeEnabled();
	});
});
