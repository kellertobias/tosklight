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
	AUTHORITY_B,
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
			state.projection?.programmers[0] ?? null,
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

	it("reloads a lower lifecycle cursor after the server generation changes", async () => {
		const store = new ProgrammerLifecycleStore();
		const firstTransport = new FakeProgrammerLifecycleTransport();
		const secondTransport = new FakeProgrammerLifecycleTransport();
		const loadSnapshot = vi
			.fn()
			.mockResolvedValueOnce(lifecycleSnapshot({ cursor: 80, revision: 20 }))
			.mockResolvedValueOnce(
				lifecycleSnapshot({
					cursor: 2,
					revision: 1,
					programmers: [otherLifecycleRow({ normalValueCount: 7 })],
				}),
			);
		const view = render(
			<ProgrammerLifecycleViewProvider
				authorityKey={AUTHORITY_A}
				store={store}
				transport={firstTransport}
				loadSnapshot={loadSnapshot}
			>
				<OperatorRowProbe onRender={vi.fn()} />
			</ProgrammerLifecycleViewProvider>,
		);
		await waitFor(() => expect(store.getSnapshot().eventSequence).toBe(80));

		view.rerender(
			<ProgrammerLifecycleViewProvider
				authorityKey={AUTHORITY_B}
				store={store}
				transport={secondTransport}
				loadSnapshot={loadSnapshot}
			>
				<OperatorRowProbe onRender={vi.fn()} />
			</ProgrammerLifecycleViewProvider>,
		);

		await waitFor(() =>
			expect(store.getSnapshot()).toMatchObject({
				authorityKey: AUTHORITY_B,
				eventSequence: 2,
				status: "ready",
				error: null,
				projection: { revision: 1 },
			}),
		);
		expect(loadSnapshot).toHaveBeenCalledTimes(2);
		expect(firstTransport.subscriptions[0].close).toHaveBeenCalledOnce();
		expect(secondTransport.subscriptions[0]).toMatchObject({ after: 2 });
	});
});
