import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PatchedFixture } from "../../../api/types";
import type {
	SelectionActionOutcome,
	SelectionActionRequest,
} from "../../../features/programmingInteraction/contracts";
import { ProgrammingInteractionViewProvider } from "../../../features/programmingInteraction/ProgrammingInteractionView";
import { ProgrammingInteractionStore } from "../../../features/programmingInteraction/store";
import {
	DESK_ID,
	FakeProgrammingTransport,
	FIXTURE_1,
	FIXTURE_2,
	programmingSnapshot,
	selection,
	selectionChange,
	SHOW_ID,
} from "../../../features/programmingInteraction/testFixtures";
import { toggledFixtureSelection, usePatchSelection } from "./selection";

const secondFixture = {
	fixture_id: FIXTURE_2,
	logical_heads: [],
} as unknown as PatchedFixture;

function Probe({ active }: { active: boolean }) {
	const selection = usePatchSelection(active);
	return (
		<output data-testid="patch-selection">
			{selection.fixtureIds ? [...selection.fixtureIds].join(",") : "loading"}
		</output>
	);
}

function ToggleProbe() {
	const patchSelection = usePatchSelection(true);
	const selected = patchSelection.orderedFixtureIds;
	return (
		<>
			<output data-testid="patch-selection">
				{selected?.join(",") ?? "loading"}
			</output>
			<button
				type="button"
				disabled={!selected || !patchSelection.actions}
				onClick={() =>
					void patchSelection.actions?.replace({
						resolvedFixtures: toggledFixtureSelection(
							selected ?? [],
							secondFixture,
						),
					})
				}
			>
				Toggle second fixture
			</button>
		</>
	);
}

function Harness({
	active,
	store,
	transport,
	loadSnapshot,
}: {
	active: boolean;
	store: ProgrammingInteractionStore;
	transport: FakeProgrammingTransport;
	loadSnapshot: () => Promise<ReturnType<typeof programmingSnapshot>>;
}) {
	return (
		<ProgrammingInteractionViewProvider
			showId={SHOW_ID}
			deskId={DESK_ID}
			store={store}
			transport={transport}
			loadSnapshot={loadSnapshot}
			applySelection={async () => {
				throw new Error("not used");
			}}
		>
			<Probe active={active} />
		</ProgrammingInteractionViewProvider>
	);
}

afterEach(cleanup);

describe("Patch scoped selection", () => {
	it("stays dormant while hidden and streams only selection authority when shown", async () => {
		const store = new ProgrammingInteractionStore();
		const transport = new FakeProgrammingTransport();
		const loadSnapshot = vi.fn(async () => programmingSnapshot());
		const view = (active: boolean) => (
			<Harness
				active={active}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
			/>
		);
		const rendered = render(view(false));

		expect(screen.getByTestId("patch-selection")).toHaveTextContent("loading");
		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);

		rendered.rerender(view(true));
		await waitFor(() =>
			expect(screen.getByTestId("patch-selection")).toHaveTextContent(FIXTURE_1),
		);
		expect(loadSnapshot).toHaveBeenCalledOnce();
		expect(transport.subscriptions[0]?.scope).toEqual({
			commandLine: false,
			selection: true,
		});

		act(() =>
			transport.emit({
				type: "event",
				sequence: 11,
				correlationId: "peer-selection",
				change: selectionChange({ selected: [FIXTURE_2] }),
			}),
		);
		await waitFor(() =>
			expect(screen.getByTestId("patch-selection")).toHaveTextContent(FIXTURE_2),
		);

		rendered.rerender(view(false));
		await waitFor(() =>
			expect(transport.subscriptions[0]?.close).toHaveBeenCalledOnce(),
		);
	});

	it("preserves a closed static selection during an optimistic additive replacement", async () => {
		const store = new ProgrammingInteractionStore();
		const transport = new FakeProgrammingTransport();
		let resolve!: (outcome: SelectionActionOutcome) => void;
		const response = new Promise<SelectionActionOutcome>((settle) => {
			resolve = settle;
		});
		const applySelection = vi.fn(
			(_deskId: string, _request: SelectionActionRequest) => response,
		);
		render(
			<ProgrammingInteractionViewProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				store={store}
				transport={transport}
				loadSnapshot={async () => programmingSnapshot()}
				applySelection={applySelection}
			>
				<ToggleProbe />
			</ProgrammingInteractionViewProvider>,
		);
		const toggle = await screen.findByRole("button", {
			name: "Toggle second fixture",
		});

		fireEvent.click(toggle);
		expect(screen.getByTestId("patch-selection")).toHaveTextContent(
			`${FIXTURE_1},${FIXTURE_2}`,
		);
		await waitFor(() => expect(applySelection).toHaveBeenCalledOnce());
		const request = applySelection.mock.calls[0][1];
		expect(request.action).toEqual({
			type: "replace",
			fixtures: [FIXTURE_1, FIXTURE_2],
			expectedRevision: 1,
		});
		act(() =>
			resolve({
				requestId: request.requestId,
				correlationId: "patch-add",
				action: "replaced",
				applied: 2,
				selection: selection(2, [FIXTURE_1, FIXTURE_2]),
				eventSequence: 11,
				replayed: false,
				warning: null,
			}),
		);
		await waitFor(() =>
			expect(store.getSnapshot().selection?.revision).toBe(2),
		);
	});

	it("removes all logical heads without disturbing earlier ordered fixtures", () => {
		const multiHead = {
			fixture_id: "master",
			logical_heads: [
				{ fixture_id: "head-left" },
				{ fixture_id: "head-right" },
			],
		} as unknown as PatchedFixture;
		expect(
			toggledFixtureSelection(
				[FIXTURE_1, "head-left", "head-right"],
				multiHead,
			),
		).toEqual([FIXTURE_1]);
	});
});
