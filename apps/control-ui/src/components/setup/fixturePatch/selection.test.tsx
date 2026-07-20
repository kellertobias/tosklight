import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProgrammingInteractionViewProvider } from "../../../features/programmingInteraction/ProgrammingInteractionView";
import { ProgrammingInteractionStore } from "../../../features/programmingInteraction/store";
import {
	DESK_ID,
	FakeProgrammingTransport,
	FIXTURE_1,
	FIXTURE_2,
	programmingSnapshot,
	selectionChange,
	SHOW_ID,
} from "../../../features/programmingInteraction/testFixtures";
import { usePatchSelection } from "./selection";

function Probe({ active }: { active: boolean }) {
	const selection = usePatchSelection(active);
	return (
		<output data-testid="patch-selection">
			{selection.fixtureIds ? [...selection.fixtureIds].join(",") : "loading"}
		</output>
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
});
