import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { OutputRuntimeProvider } from "../../../features/outputRuntime/OutputRuntimeView";
import { OutputRuntimeStore } from "../../../features/outputRuntime/store";
import {
	DESK_ID,
	FakeOutputRuntimeTransport,
	outputProjection,
	SHOW_ID,
} from "../../../features/outputRuntime/testFixtures";
import { CommandLineStatus } from "./CommandLineStatus";

afterEach(cleanup);

describe("CommandLineStatus Output authority", () => {
	it("preserves status semantics while following only scoped blackout", async () => {
		const store = new OutputRuntimeStore();
		const transport = new FakeOutputRuntimeTransport();
		const onOpen = vi.fn();
		render(
			<OutputRuntimeProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				authorityKey="session-a"
				store={store}
				transport={transport}
			>
				<CommandLineStatus
					status="connected"
					frequency={60}
					timecode={null}
					onOpen={onOpen}
				/>
			</OutputRuntimeProvider>,
		);
		const button = screen.getByRole("button", {
			name: "DMX 60Hz; No Timecode. Open running and output controls",
		});
		expect(button).toHaveClass("command-status", "connected");
		await waitFor(() => expect(transport.subscribe).toHaveBeenCalledOnce());
		expect(screen.queryByText("BLACKOUT")).not.toBeInTheDocument();

		act(() =>
			transport.emit({
				type: "event",
				sequence: 11,
				correlationId: null,
				change: {
					projection: outputProjection({
						revision: 2,
						grandMaster: 0.2,
						blackout: true,
					}),
				},
			}),
		);
		expect(screen.getByText("BLACKOUT")).toBeInTheDocument();
		expect(screen.getByText("BLACKOUT").parentElement).toHaveClass(
			"blackout-status",
		);
		button.click();
		expect(onOpen).toHaveBeenCalledOnce();
	});
});
