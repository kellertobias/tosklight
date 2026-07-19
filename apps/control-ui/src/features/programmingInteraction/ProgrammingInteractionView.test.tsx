import { act, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
	ProgrammingInteractionViewProvider,
	useProgrammingCommandLineView,
} from "./ProgrammingInteractionView";
import { ProgrammingInteractionStore } from "./store";
import {
	commandChange,
	commandLine,
	DESK_ID,
	FakeProgrammingTransport,
	OTHER_SHOW_ID,
	programmingSnapshot,
	selectionChange,
	SHOW_ID,
} from "./testFixtures";

function CommandProbe({
	visible,
	onRender,
}: {
	visible: boolean;
	onRender: () => void;
}) {
	onRender();
	const command = useProgrammingCommandLineView(visible);
	return <span>{command?.text ?? (visible ? "Loading" : "Hidden")}</span>;
}

describe("ProgrammingInteractionViewProvider", () => {
	it("performs no work for a hidden view and isolates irrelevant events", async () => {
		const store = new ProgrammingInteractionStore();
		const transport = new FakeProgrammingTransport();
		const loadSnapshot = vi.fn(async () => programmingSnapshot());
		const onRender = vi.fn();
		const view = (visible: boolean) => (
			<ProgrammingInteractionViewProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
			>
				<CommandProbe visible={visible} onRender={onRender} />
			</ProgrammingInteractionViewProvider>
		);
		const rendered = render(view(false));

		expect(screen.getByText("Hidden")).toBeInTheDocument();
		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);

		rendered.rerender(view(true));
		await waitFor(() => expect(transport.subscriptions).toHaveLength(1));
		expect(transport.subscriptions[0].scope).toEqual({
			commandLine: true,
			selection: false,
		});
		await waitFor(() => expect(screen.getByText("FIXTURE")).toBeInTheDocument());

		const beforeIrrelevant = onRender.mock.calls.length;
		act(() =>
			transport.emit({
				type: "event",
				sequence: 24,
				change: selectionChange({ revision: 2 }),
			}),
		);
		expect(onRender).toHaveBeenCalledTimes(beforeIrrelevant);

		act(() =>
			transport.emit({
				type: "event",
				sequence: 31,
				change: commandChange({ revision: 2, text: "FIXTURE 31" }),
			}),
		);
		await waitFor(() =>
			expect(screen.getByText("FIXTURE 31")).toBeInTheDocument(),
		);

		rendered.rerender(view(false));
		await waitFor(() =>
			expect(transport.subscriptions[0].close).toHaveBeenCalledOnce(),
		);
		expect(screen.getByText("Hidden")).toBeInTheDocument();
	});

	it("resets and rehydrates when the active show changes", async () => {
		const store = new ProgrammingInteractionStore();
		const transport = new FakeProgrammingTransport();
		const loadSnapshot = vi
			.fn()
			.mockResolvedValueOnce(programmingSnapshot())
			.mockResolvedValueOnce(
				programmingSnapshot({
					sequence: 20,
					command: commandLine(3, "GROUP", "GROUP"),
				}),
			);
		const view = (showId: string) => (
			<ProgrammingInteractionViewProvider
				showId={showId}
				deskId={DESK_ID}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
			>
				<CommandProbe visible onRender={() => undefined} />
			</ProgrammingInteractionViewProvider>
		);
		const rendered = render(view(SHOW_ID));
		await waitFor(() => expect(screen.getByText("FIXTURE")).toBeInTheDocument());

		rendered.rerender(view(OTHER_SHOW_ID));
		await waitFor(() => expect(screen.getByText("GROUP")).toBeInTheDocument());

		expect(loadSnapshot).toHaveBeenCalledTimes(2);
		expect(transport.subscriptions[0].close).toHaveBeenCalledOnce();
		expect(transport.subscriptions[1].after).toBe(20);
		expect(store.getSnapshot()).toMatchObject({
			showId: OTHER_SHOW_ID,
			eventSequence: 20,
		});
	});
});
