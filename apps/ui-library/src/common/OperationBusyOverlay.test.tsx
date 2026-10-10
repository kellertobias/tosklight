import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { OperationBusyOverlay } from "./OperationBusyOverlay";

afterEach(() => {
	cleanup();
	vi.useRealTimers();
});
describe("operation busy overlay", () => {
	it("shows honest indeterminate elapsed progress, real cancellation and restores focus", () => {
		vi.useFakeTimers();
		vi.setSystemTime(10_000);
		const cancel = vi.fn();
		const prior = document.createElement("button");
		document.body.append(prior);
		prior.focus();
		const view = render(
			<OperationBusyOverlay
				title="Inspect archive"
				message="The active show is unchanged."
				source="Rig.mvr"
				startedAt={10_000}
				onCancel={cancel}
				cancelLabel="Cancel inspection"
			/>,
		);
		expect(screen.getByRole("status")).toHaveFocus();
		expect(screen.getByRole("progressbar")).not.toHaveAttribute("value");
		act(() => vi.advanceTimersByTime(3000));
		expect(screen.getByText("Rig.mvr · Elapsed 3 s")).toBeVisible();
		fireEvent.keyDown(screen.getByRole("status"), { key: "Tab" });
		expect(
			screen.getByRole("button", { name: "Cancel inspection" }),
		).toHaveFocus();
		fireEvent.click(screen.getByRole("button", { name: "Cancel inspection" }));
		expect(cancel).toHaveBeenCalledOnce();
		view.unmount();
		expect(prior).toHaveFocus();
		prior.remove();
	});
	it("blocks underlying modal controls and restores their exact state without inventing cancellation", () => {
		const view = render(
			<section role="dialog">
				<button type="button">Original control</button>
				<OperationBusyOverlay
					title="Import package"
					message="Fixture data may be writing."
				/>
			</section>,
		);
		const original = screen.getByText("Original control");
		expect(original.inert).toBe(true);
		expect(screen.getByRole("dialog")).toHaveAttribute("aria-busy", "true");
		expect(screen.queryByRole("button", { name: "Cancel" })).toBeNull();
		fireEvent.keyDown(screen.getByRole("status"), {
			key: "Tab",
			shiftKey: true,
		});
		expect(screen.getByRole("status")).toHaveFocus();
		view.rerender(
			<section role="dialog">
				<button type="button">Original control</button>
			</section>,
		);
		expect(screen.getByText("Original control").inert).toBe(false);
		expect(screen.getByRole("dialog")).not.toHaveAttribute("aria-busy");
	});
	it("protects nested body siblings and restores existing inert attributes, busy state and focus", () => {
		function Workflow({ busy }: { busy: boolean }) {
			return (
				<section role="dialog" aria-busy="false">
					<header>
						<button type="button">Close workflow</button>
					</header>
					<div>
						<button type="button">Edit source</button>
						<div>
							<button type="button">Edit mapping</button>
							<button type="button" inert>
								Previously unavailable
							</button>
							{busy && (
								<OperationBusyOverlay
									title="Inspect archive"
									message="Reading source data."
									onCancel={() => {}}
								/>
							)}
						</div>
					</div>
					<footer>
						<button type="button">Apply workflow</button>
					</footer>
				</section>
			);
		}
		const view = render(<Workflow busy={false} />);
		const mapping = screen.getByRole("button", { name: "Edit mapping" });
		const unavailable = screen.getByText("Previously unavailable");
		const priorInertAttribute = unavailable.getAttribute("inert");
		mapping.focus();
		view.rerender(<Workflow busy />);
		expect(screen.getByText("Close workflow").parentElement?.inert).toBe(true);
		expect(screen.getByText("Edit source").inert).toBe(true);
		expect(mapping.inert).toBe(true);
		expect(unavailable.inert).toBe(true);
		expect(screen.getByText("Apply workflow").parentElement?.inert).toBe(true);
		expect(screen.getByRole("status")).toHaveFocus();
		fireEvent.keyDown(screen.getByRole("status"), { key: "Tab" });
		expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus();
		view.rerender(<Workflow busy={false} />);
		expect(mapping.inert).toBe(false);
		expect(mapping).not.toHaveAttribute("inert");
		expect(unavailable.inert).toBe(true);
		expect(unavailable.getAttribute("inert")).toBe(priorInertAttribute);
		expect(screen.getByText("Edit source").inert).toBe(false);
		expect(screen.getByText("Close workflow").parentElement?.inert).toBe(false);
		expect(screen.getByText("Apply workflow").parentElement?.inert).toBe(false);
		expect(screen.getByRole("dialog")).toHaveAttribute("aria-busy", "false");
		expect(mapping).toHaveFocus();
	});
});
