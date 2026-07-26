import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	type PoolSlotViewModel,
	PoolWindow,
	type PoolWindowProps,
} from "./PoolWindow";

afterEach(() => {
	cleanup();
	vi.useRealTimers();
});

function emptySlot(index: number): PoolSlotViewModel<string> {
	return {
		id: `empty-${index}`,
		position: index,
		card: {
			number: index + 1,
			primary: "Empty",
			states: ["empty"],
		},
	};
}

function renderPool(overrides: Partial<PoolWindowProps<string>> = {}) {
	return render(
		<div style={{ width: 900, height: 600 }}>
			<PoolWindow
				title="Test Pool"
				slots={[]}
				emptySlot={emptySlot}
				{...overrides}
			/>
		</div>,
	);
}

describe("PoolWindow", () => {
	it.each([
		[undefined, 200],
		[20, 200],
		[230, 230],
	])("normalizes slotCount %s to %s boxes", (slotCount, expected) => {
		renderPool({ slotCount });
		expect(document.querySelectorAll(".pool-card")).toHaveLength(expected);
	});

	it("replaces sparse positions without appending stored objects", () => {
		renderPool({
			slots: [
				{
					id: "group-3",
					position: 2,
					card: { number: 3, primary: "Front Wash", kind: "group" },
				},
				{
					id: "group-42",
					position: 41,
					card: { number: 42, primary: "Movers", kind: "group" },
				},
			],
		});

		expect(document.querySelectorAll(".pool-card")).toHaveLength(200);
		expect(
			document.querySelector('[data-pool-position="2"]'),
		).toHaveTextContent("Front Wash");
		expect(
			document.querySelector('[data-pool-position="41"]'),
		).toHaveTextContent("Movers");
		expect(screen.getAllByText("Empty")).toHaveLength(198);
	});

	it("returns stable identities and positions for filled and empty click and hold", () => {
		vi.useFakeTimers();
		const click = vi.fn();
		const hold = vi.fn();
		renderPool({
			slots: [
				{
					id: "preset-2.1",
					position: 1,
					card: { number: "2.1", primary: "Red", kind: "preset" },
				},
			],
			onSlotClick: click,
			onSlotPressHold: hold,
		});
		const filled = document.querySelector(
			'[data-pool-slot-id="preset-2.1"]',
		) as HTMLElement;
		const empty = document.querySelector(
			'[data-pool-slot-id="empty-4"]',
		) as HTMLElement;

		fireEvent.click(filled);
		fireEvent.pointerDown(empty);
		vi.advanceTimersByTime(650);
		fireEvent.pointerUp(empty);
		fireEvent.click(empty);

		expect(click).toHaveBeenCalledWith("preset-2.1", 1);
		expect(hold).toHaveBeenCalledWith("empty-4", 4);
		expect(click).toHaveBeenCalledTimes(1);
	});

	it("passes header, actions, settings, and card width to package primitives", () => {
		const action = vi.fn();
		renderPool({
			info: { primary: "4 stored", secondary: "Page 1" },
			actions: [[{ id: "record", label: "Record", onClick: action }]],
			settingsTabs: [
				{
					id: "pool",
					label: "Pool",
					content: "Pool settings",
				},
			],
			minimumCardWidth: 132,
		});

		expect(screen.getByText("4 stored")).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Record" }));
		expect(action).toHaveBeenCalledOnce();
		expect(document.querySelector(".pool-window-grid")).toHaveStyle({
			"--grid-cell-min": "132px",
		});
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		expect(screen.getByText("Pool settings")).toBeInTheDocument();
	});

	it("lets application adapters retain card composition while applying stable slot identity", () => {
		renderPool({
			slots: [
				{
					id: "group-17",
					position: 16,
					card: { number: 17, primary: "Stored group" },
				},
			],
			renderSlot: (slot) => (
				<button type="button" className="application-pool-card">
					{slot.card.primary}
				</button>
			),
		});

		const stored = screen.getByRole("button", { name: "Stored group" });
		expect(stored).toHaveAttribute("data-pool-slot-id", "group-17");
		expect(stored).toHaveAttribute("data-pool-position", "16");
		expect(document.querySelectorAll(".application-pool-card")).toHaveLength(
			200,
		);
	});
});
