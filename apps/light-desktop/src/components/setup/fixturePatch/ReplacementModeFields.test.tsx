import {
	cleanup,
	fireEvent,
	render,
	screen,
	within,
} from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { FixtureDefinition } from "../../../api/types";
import { fixtureDefinitionKey } from "../fixtureProfileModel";
import type { PatchController } from "./controller";
import {
	type ReplacementDecisionRow,
	ReplacementDecisions,
	replacementDecisionRows,
} from "./ReplacementDecisions";
import {
	ReplacementEditDialog,
	ReplacementModeFields,
} from "./ReplacementModeFields";

let controller: PatchController;
vi.mock("./controller", () => ({ usePatchController: () => controller }));
vi.mock("./editSave", () => ({ saveEdit: vi.fn() }));
afterEach(cleanup);

function definition(
	shared: boolean,
	names: string[],
	attribute = "intensity",
): FixtureDefinition {
	const heads = names.map((name, index) => ({
		id: `head-${shared ? "old" : "new"}-${index}`,
		name,
		master_shared: shared,
	}));
	return {
		id: shared ? "old-definition" : "new-definition",
		revision: 7,
		profile_id: shared ? "old-profile" : "new-profile",
		manufacturer: "Acme",
		name: shared ? "Old" : "New",
		mode: "Standard",
		mode_id: "mode",
		footprint: 12,
		heads: heads.map(() => ({ parameters: [] })),
		profile_snapshot: {
			id: shared ? "old-profile" : "new-profile",
			revision: 7,
			modes: [
				{
					id: "mode",
					heads,
					channels: heads.map((head) => ({
						head_id: head.id,
						behavior: "controlled",
						attribute,
						fixture_attribute: attribute,
						functions: [],
					})),
				},
			],
		},
	} as unknown as FixtureDefinition;
}
function setup(choices: Record<string, string> = {}, attribute = "intensity") {
	const source = definition(true, ["Main"]);
	const target = definition(false, ["Left", "Right"], attribute);
	controller = {
		data: {
			selected: {
				name: "Fixture 1",
				fixture_id: "root",
				definition: source,
				logical_heads: [{ fixture_id: "old-child", head_index: 0 }],
			},
			definition: target,
			selectedModeFamily: { modes: [source, target] },
			availableDefinitions: [source, target],
		},
		ui: {
			edit: "mode",
			definitionKey: fixtureDefinitionKey(target),
			replacingFixture: true,
			replacementQuery: "",
			replacementRevision: { show: 3, patch: 2 },
			replacementHeads: choices,
			setReplacementHeads: vi.fn(),
			setDefinitionKey: vi.fn(),
			setReplacementQuery: vi.fn(),
			setReplacingFixture: vi.fn(),
			editError: "",
		},
		patch: { pendingFixtureIds: new Set() },
	} as unknown as PatchController;
	return replacementDecisionRows(controller);
}

function DecisionsHarness({ initial }: { initial: ReplacementDecisionRow[] }) {
	const [rows, setRows] = useState(initial);
	return (
		<ReplacementDecisions
			rows={rows}
			disabled={false}
			onChoice={(key, choice) =>
				setRows((current) =>
					current.map((row) =>
						row.key === key
							? {
									...row,
									choice,
									state:
										choice === "__unmapped"
											? "dormant"
											: choice
												? "mapped"
												: "required",
								}
							: row,
					),
				)
			}
		/>
	);
}

describe("replacement decision presentation and navigation", () => {
	it("counts logical heads and shared families separately; blank is not explicit dormancy", () => {
		const rows = setup();
		render(<DecisionsHarness initial={rows} />);
		expect(screen.getByRole("status")).toHaveTextContent(
			"2 of 2 decisions remaining · 1 shared families · 1 logical heads",
		);
		fireEvent.click(
			screen.getByRole("checkbox", {
				name: "Leave Shared head Main · intensity unmatched",
			}),
		);
		expect(screen.getByRole("status")).toHaveTextContent(
			"1 of 2 decisions remaining",
		);
		expect(
			screen.getByText("Resolved · programming stays stored and dormant"),
		).toBeVisible();
	});
	it("keeps multiple shared destination owners instead of a single target select", () => {
		render(<DecisionsHarness initial={setup()} />);
		fireEvent.click(
			screen.getByRole("checkbox", {
				name: "Route Shared head Main · intensity to Left · 1",
			}),
		);
		fireEvent.click(
			screen.getByRole("checkbox", {
				name: "Route Shared head Main · intensity to Right · 2",
			}),
		);
		expect(screen.getByRole("checkbox", { name: /to Left/ })).toBeChecked();
		expect(screen.getByRole("checkbox", { name: /to Right/ })).toBeChecked();
		expect(
			screen.getByRole("checkbox", { name: /unmatched/ }),
		).not.toBeChecked();
	});
	it("Next unresolved scrolls to and focuses the visible ordinary select trigger below completed rows", () => {
		const rows = setup({ "root:head-old-0:intensity": "__unmapped" });
		const scroll = vi.fn();
		const previous = HTMLElement.prototype.scrollIntoView;
		HTMLElement.prototype.scrollIntoView = scroll;
		try {
			render(<DecisionsHarness initial={rows} />);
			fireEvent.click(screen.getByRole("button", { name: "Next unresolved" }));
			expect(
				within(
					screen.getByRole("group", { name: "Existing head 1" }),
				).getByRole("button", { name: "Choose correspondence" }),
			).toHaveFocus();
			expect(scroll).toHaveBeenCalledWith({ block: "nearest" });
		} finally {
			HTMLElement.prototype.scrollIntoView = previous;
		}
	});
	it("Next focuses the actual shared checkbox; no compatible family still requires explicit dormant consent", () => {
		const scroll = vi.fn();
		const previous = HTMLElement.prototype.scrollIntoView;
		HTMLElement.prototype.scrollIntoView = scroll;
		try {
			render(<DecisionsHarness initial={setup({}, "color.red")} />);
			expect(screen.getByText(/No compatible destination/)).toBeVisible();
			fireEvent.click(screen.getByRole("button", { name: "Next unresolved" }));
			expect(
				screen.getByRole("checkbox", { name: /Leave Shared head/ }),
			).toHaveFocus();
			expect(screen.getByRole("status")).toHaveTextContent("2 of 2");
		} finally {
			HTMLElement.prototype.scrollIntoView = previous;
		}
	});
	it("product/mode changes clear previous correspondences through the existing draft setters", () => {
		setup({ "old-child": "head-new-0" });
		render(<ReplacementModeFields />);
		fireEvent.click(
			screen.getByRole("button", { name: "Acme · New · Standard · 12ch" }),
		);
		fireEvent.click(screen.getByRole("option", { name: /Acme · Old/ }));
		expect(controller.ui.setDefinitionKey).toHaveBeenCalledWith(
			fixtureDefinitionKey(definition(true, ["Main"])),
		);
		expect(controller.ui.setReplacementHeads).toHaveBeenCalledWith({});
	});
	it("keeps fixed actions disabled with a reason, readable error and obtainable full consequence details", () => {
		setup();
		controller.ui.editError =
			"Choose a compatible destination.\nDiagnostic detail remains available.";
		render(
			<ModalProvider>
				<ReplacementEditDialog pending={false} close={vi.fn()} />
			</ModalProvider>,
		);
		for (const button of screen.getAllByRole("button", {
			name: "Set",
		}))
			expect(button).toBeDisabled();
		expect(
			screen.getByText("Resolve 2 correspondences before Set."),
		).toBeVisible();
		expect(screen.getByRole("alert")).toHaveTextContent(
			"Choose a compatible destination.",
		);
		fireEvent.click(screen.getByText("Full replacement error"));
		expect(screen.getByText(/Diagnostic detail/)).toBeVisible();
		fireEvent.click(
			screen.getByText("Programming, addresses and calibration details"),
		);
		expect(
			screen.getByText(/Incompatible installed calibration remains stored/),
		).toBeVisible();
		expect(screen.getByText(/New splits start unpatched/)).toBeVisible();
	});
});
