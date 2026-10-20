import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	type PoolSlotViewModel,
	PoolWindow,
	PoolGrid,
	type PoolWindowProps,
	resolvePoolGridColumns,
} from "./PoolWindow";
import { PoolTileLayoutProvider } from "./PoolTileLayout";

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

	it("compacts filtered results while preserving their actual pool numbers", () => {
		renderPool({
			fillEmptySlots: false,
			slots: [
				{
					id: "group-1",
					position: 0,
					card: { number: 1, primary: "Front" },
				},
				{
					id: "group-4",
					position: 3,
					card: { number: 4, primary: "Back" },
				},
			],
		});

		const cards = document.querySelectorAll(".pool-card");
		expect(cards).toHaveLength(2);
		expect(cards[0]).toHaveAttribute("data-pool-position", "0");
		expect(cards[0]?.querySelector(".number")).toHaveTextContent("1");
		expect(cards[1]).toHaveAttribute("data-pool-position", "1");
		expect(cards[1]?.querySelector(".number")).toHaveTextContent("4");
	});

	it("applies grid-level filled treatment and semantic workflow colors", () => {
		renderPool({
			appearance: {
				filledStyle: "outline",
				recordColor: "#aa0000",
				updateColor: "#bbbb00",
				setColor: "#00cccc",
			},
		});
		const grid = document.querySelector(".pool-window-grid");
		expect(grid).toHaveClass("pool-filled-outline");
		expect(grid).toHaveStyle({
			"--pool-record-color": "#aa0000",
			"--pool-update-color": "#bbbb00",
			"--pool-set-color": "#00cccc",
		});
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
			groups: [
				{
					id: "record",
					actions: [{ id: "record", label: "Record", onPress: action }],
				},
			],
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

	it("uses the shared 100px minimum for pool cards", () => {
		renderPool();

		expect(document.querySelector(".pool-window-grid")).toHaveStyle({
			"--grid-cell-min": "100px",
		});
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

describe("resolvePoolGridColumns", () => {
	const sizing = { defaultWidth: 100, minimumWidth: 70 };

	it("fits default-width tiles and lets them stretch up to 1.5 times", () => {
		expect(resolvePoolGridColumns(300, 0, sizing)).toBe(3);
		expect(resolvePoolGridColumns(450, 0, sizing)).toBe(4);
		expect(resolvePoolGridColumns(140, 0, sizing)).toBe(1);
	});

	it("adds a narrower column only once tiles would grow past 1.5 times", () => {
		expect(resolvePoolGridColumns(160, 0, sizing)).toBe(2);
		expect(
			resolvePoolGridColumns(160, 0, { ...sizing, minimumWidth: 90 }),
		).toBe(1);
	});

	it("accounts for the gap and never goes below one column", () => {
		expect(resolvePoolGridColumns(304, 2, sizing)).toBe(3);
		expect(resolvePoolGridColumns(40, 2, sizing)).toBe(1);
	});

	it("never lets the minimum exceed the default width", () => {
		expect(
			resolvePoolGridColumns(300, 0, { defaultWidth: 100, minimumWidth: 200 }),
		).toBe(3);
	});
});

describe("PoolTileLayoutProvider", () => {
	it("fills large fixed layouts without losing stored addresses or a larger configured pool", () => {
		const stored = {
			id: "group-576",
			position: 575,
			card: { number: 576, primary: "Last visible group" },
		};
		const view = render(
			<PoolTileLayoutProvider layout={{ mode: "fixed", columns: 24, rows: 24 }}>
				<PoolGrid slots={[stored]} emptySlot={emptySlot} />
			</PoolTileLayoutProvider>,
		);
		expect(document.querySelectorAll("[data-pool-position]")).toHaveLength(576);
		expect(
			document.querySelector('[data-pool-position="575"]'),
		).toHaveTextContent("Last visible group");
		view.rerender(
			<PoolTileLayoutProvider layout={{ mode: "fixed", columns: 24, rows: 24 }}>
				<PoolGrid slots={[stored]} slotCount={1000} emptySlot={emptySlot} />
			</PoolTileLayoutProvider>,
		);
		expect(document.querySelectorAll("[data-pool-position]")).toHaveLength(
			1000,
		);
		expect(
			document.querySelector('[data-pool-position="575"]'),
		).toHaveAttribute("data-pool-slot-id", "group-576");
	});
	it("does not invent navigation entries when a fixed grid disables empty slots", () => {
		const stored = {
			id: "media-4",
			position: 3,
			card: { number: 4, primary: "Actual media file" },
		};
		render(
			<PoolTileLayoutProvider layout={{ mode: "fixed", columns: 24, rows: 24 }}>
				<PoolGrid
					slots={[stored]}
					fillEmptySlots={false}
					emptySlot={emptySlot}
				/>
			</PoolTileLayoutProvider>,
		);
		expect(document.querySelectorAll("[data-pool-position]")).toHaveLength(1);
		expect(screen.getByText("Actual media file")).toBeInTheDocument();
	});
	it("preserves legacy fixed columns when no layout is provided", () => {
		render(<PoolGrid slots={[]} columns={4} emptySlot={emptySlot} />);
		const grid = document.querySelector(".pool-window-grid");
		expect(grid).toHaveClass("square-grid");
		expect(grid).toHaveStyle({
			gridTemplateColumns: "repeat(4, minmax(0, 1fr))",
		});
	});
	it("explicit automatic mode restores squares and ignores legacy fixed columns", () => {
		render(
			<PoolTileLayoutProvider layout={{ mode: "automatic" }}>
				<PoolGrid slots={[]} columns={4} emptySlot={emptySlot} />
			</PoolTileLayoutProvider>,
		);
		const grid = document.querySelector<HTMLElement>(".pool-window-grid")!;
		expect(grid).toHaveClass("square-grid");
		expect(grid.style.gridTemplateColumns).toBe("");
	});
	it("switches to viewport rows while retaining every numbered address and back to squares", () => {
		const view = render(
			<PoolTileLayoutProvider layout={{ mode: "fixed", columns: 6, rows: 3 }}>
				<PoolGrid slots={[]} emptySlot={emptySlot} />
			</PoolTileLayoutProvider>,
		);
		const grid = document.querySelector(".pool-window-grid");
		expect(grid).toHaveClass("pool-fixed-grid", "compact-grid");
		expect(grid).toHaveStyle({
			gridTemplateColumns: "repeat(6, minmax(0, 1fr))",
		});
		expect(grid?.querySelectorAll("[data-pool-position]")).toHaveLength(200);
		view.rerender(
			<PoolTileLayoutProvider layout={{ mode: "automatic" }}>
				<PoolGrid slots={[]} emptySlot={emptySlot} />
			</PoolTileLayoutProvider>,
		);
		expect(grid).toHaveClass("square-grid");
		expect(grid).not.toHaveClass("pool-fixed-grid");
		expect(grid?.querySelectorAll("[data-pool-position]")).toHaveLength(200);
	});
});
