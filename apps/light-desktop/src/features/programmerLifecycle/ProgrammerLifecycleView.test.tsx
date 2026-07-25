import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { useCallback } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	ProgrammerLifecycleViewProvider,
	useProgrammerLifecycleSelector,
} from "./ProgrammerLifecycleView";
import { ProgrammerLifecycleStore } from "./store";
import {
	AUTHORITY_A,
	FakeProgrammerLifecycleTransport,
	lifecycleSnapshot,
	otherLifecycleRow,
	upsertChange,
} from "./testFixtures";

function StaticProbe() {
	return <span>Dormant child</span>;
}

function OperatorRowProbe({ onRender }: { onRender: () => void }) {
	onRender();
	const selectRow = useCallback(
		(state: ReturnType<ProgrammerLifecycleStore["getSnapshot"]>) =>
			state.projection?.programmers.find(
				(row) => row.userId === "operator-a",
			) ?? null,
		[],
	);
	const row = useProgrammerLifecycleSelector(selectRow);
	return <span>{row?.normalValueCount ?? "Loading"}</span>;
}

afterEach(cleanup);

describe("ProgrammerLifecycleViewProvider", () => {
	it("does not load or subscribe until a lifecycle view mounts", async () => {
		const store = new ProgrammerLifecycleStore();
		const transport = new FakeProgrammerLifecycleTransport();
		const loadSnapshot = vi.fn(async () => lifecycleSnapshot());

		render(
			<ProgrammerLifecycleViewProvider
				authorityKey={AUTHORITY_A}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
			>
				<StaticProbe />
			</ProgrammerLifecycleViewProvider>,
		);
		await act(async () => Promise.resolve());

		expect(screen.getByText("Dormant child")).toBeInTheDocument();
		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);
	});

	it("does not rerender a selector for an unrelated user delta", async () => {
		const store = new ProgrammerLifecycleStore();
		const transport = new FakeProgrammerLifecycleTransport();
		const onRender = vi.fn();
		render(
			<ProgrammerLifecycleViewProvider
				authorityKey={AUTHORITY_A}
				store={store}
				transport={transport}
				loadSnapshot={async () => lifecycleSnapshot()}
			>
				<OperatorRowProbe onRender={onRender} />
			</ProgrammerLifecycleViewProvider>,
		);
		await waitFor(() => expect(screen.getByText("3")).toBeInTheDocument());
		const renderedBeforeUnrelatedChange = onRender.mock.calls.length;

		act(() =>
			transport.emit({
				type: "event",
				sequence: 11,
				correlationId: "other-user-change",
				change: upsertChange(otherLifecycleRow({ normalValueCount: 8 }), 5),
			}),
		);

		expect(screen.getByText("3")).toBeInTheDocument();
		expect(onRender).toHaveBeenCalledTimes(renderedBeforeUnrelatedChange);
	});
});
