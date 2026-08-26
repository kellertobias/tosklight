import {
	cleanup,
	fireEvent,
	render as rtlRender,
	screen,
} from "@testing-library/react";
import { StrictMode } from "react";
import type { ReactElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ModalProvider } from "../modals/ModalStack";
import {
	ButtonGrid,
	DataTable,
	GridButton,
	WindowDropdown,
	WindowFrame,
	WindowHeader,
	WindowScrollArea,
	WindowSettings,
} from ".";

const render = (ui: ReactElement) => rtlRender(ui, { wrapper: ModalProvider });

describe("window kit", () => {
	afterEach(cleanup);
	it("renders two-line information, grouped actions, and Settings last", () => {
		const onSettings = vi.fn();
		const { container } = render(
			<WindowHeader
				title="Stage"
				info={{
					primary: "1 selected",
					secondary: <span className="test-legend">Shift for range</span>,
				}}
				search={{ value: "", onSearch: vi.fn() }}
				groups={[
					{
						id: "one",
						actions: [{ id: "one", label: "First", onPress: vi.fn() }],
					},
					{
						id: "two",
						actions: [{ id: "two", label: "Second", onPress: vi.fn() }],
					},
				]}
				settings
				onSettings={onSettings}
			/>,
		);
		expect(screen.getByText("Stage")).toBeInTheDocument();
		expect(screen.getByText("Shift for range")).toHaveClass("test-legend");
		expect(screen.getByText("Shift for range").parentElement?.tagName).toBe(
			"SMALL",
		);
		expect(
			[
				...container.querySelectorAll(
					".ui-title-chrome-group > .ui-title-chrome-action > button, .ui-title-chrome-terminals > .ui-title-chrome-action > button",
				),
			].map((button) => button.textContent),
		).toEqual(["First", "Second", "⚙"]);
		const header = container.querySelector(".ui-window-header");
		const search = screen
			.getByRole("textbox", { name: "Search Stage" })
			.closest(".ui-window-header-search");
		const chrome = container.querySelector(".ui-window-action-groups");
		expect(header).not.toBeNull();
		expect(search).not.toBeNull();
		expect(chrome).not.toBeNull();
		if (!header || !search || !chrome)
			throw new Error("Missing window header controls");
		expect([...chrome.children].indexOf(search)).toBeLessThan(
			[...chrome.children].indexOf(
				chrome.querySelector(".ui-title-chrome-groups")!,
			),
		);
		const settingsButton = screen.getByRole("button", { name: "Settings" });
		document.body.focus();
		expect(document.activeElement).toBe(document.body);
		fireEvent.click(settingsButton);
		expect(onSettings).toHaveBeenCalledWith(settingsButton);
	});
	it("renders standard controlled search only when a callback is supplied", () => {
		const onSearch = vi.fn();
		const { rerender } = render(<WindowHeader title="Groups" />);
		expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "Settings" }),
		).not.toBeInTheDocument();
		rerender(<WindowHeader title="Groups" search={{ value: "", onSearch }} />);
		const input = screen.getByRole("textbox", { name: "Search Groups" });
		fireEvent.change(input, { target: { value: "front" } });
		expect(onSearch).toHaveBeenCalledWith("front");
	});
	it("can keep Settings visible but disabled when the current tab has none", () => {
		render(<WindowHeader title="Running" settings />);

		expect(screen.getByRole("button", { name: "Settings" })).toBeDisabled();
	});
	it("hides Settings in a full window with no configured settings tabs", () => {
		render(<WindowFrame title="Running">Running body</WindowFrame>);

		expect(
			screen.queryByRole("button", { name: "Settings" }),
		).not.toBeInTheDocument();
	});
	it("opens and closes the standard title dropdown around one selected action", () => {
		const selected = vi.fn();
		render(
			<WindowDropdown
				ariaLabel="Add"
				label="Add"
				items={[{ id: "marker", label: "Add Marker", onSelect: selected }]}
			/>,
		);
		const trigger = screen.getByRole("button", { name: "Add" });
		expect(trigger).toHaveAttribute("aria-expanded", "false");
		fireEvent.click(trigger);
		expect(trigger).toHaveAttribute("aria-expanded", "true");
		fireEvent.click(screen.getByRole("menuitem", { name: "Add Marker" }));
		expect(selected).toHaveBeenCalledTimes(1);
		expect(screen.queryByRole("menu")).toBeNull();
	});
	it("makes an armed window-title action pointer and keyboard operable", () => {
		const remove = vi.fn();
		render(
			<WindowHeader
				title="Fixture Sheet"
				onTitleClick={remove}
				titleActionLabel="Remove Fixture Sheet pane"
			/>,
		);
		const title = screen.getByRole("button", {
			name: "Remove Fixture Sheet pane",
		});
		fireEvent.click(title);
		fireEvent.keyDown(title, { key: "Enter" });
		fireEvent.keyDown(title, { key: " " });
		expect(remove).toHaveBeenCalledTimes(3);
	});
	it("preserves window chrome while adding a pane drag-handle class", () => {
		const { container } = render(
			<WindowHeader
				title="Stage"
				dragHandleProps={{ className: "pane-drag-handle" }}
			/>,
		);
		expect(container.querySelector("header")).toHaveClass(
			"ui-window-header",
			"pane-drag-handle",
		);
	});
	it("switches settings tabs and closes", () => {
		const close = vi.fn();
		render(
			<WindowSettings
				title="Pane Settings"
				tabs={[
					{ id: "pane", label: "Pane Settings", content: "Size" },
					{ id: "pool", label: "Pool", content: "Family" },
				]}
				onClose={close}
			/>,
		);
		fireEvent.click(screen.getByRole("tab", { name: "Pool" }));
		expect(screen.getByText("Family")).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Close settings" }));
		expect(close).toHaveBeenCalledOnce();
	});
	it("renders built-in settings as an anchored popover without a backdrop", () => {
		render(
			<WindowSettings
				modal={false}
				anchor={new DOMRect(900, 10, 90, 38)}
				title="Stage Settings"
				tabs={[{ id: "stage", label: "Stage", content: "Display" }]}
				onClose={() => undefined}
			/>,
		);
		const dialog = screen.getByRole("dialog", { name: "Stage Settings" });
		expect(dialog).toHaveClass("popover");
		expect(dialog.closest(".ui-window-settings-backdrop")).toBeNull();
	});
	it("keeps selected and active rows independent and navigates empty rows", () => {
		const active = vi.fn();
		render(
			<DataTable
				rows={[{ id: "one" }, { id: "two" }]}
				columns={[{ id: "name", header: "Name", render: (row) => row.id }]}
				rowKey={(row) => row.id}
				selected={(row) => row.id === "two"}
				activeIndex={0}
				onActiveIndexChange={active}
				emptyRows={1}
			/>,
		);
		const rows = screen.getAllByRole("row");
		expect(rows[2]).toHaveClass("selected");
		expect(rows[1]).toHaveClass("active");
		fireEvent.keyDown(rows[2], { key: "ArrowDown" });
		expect(active).toHaveBeenCalledWith(2);
	});
	it("bounds virtualized table DOM while preserving the authoritative row count", () => {
		const visibleRows = vi.fn();
		render(
			<div className="ui-window-scroller">
				<DataTable
					rows={Array.from({ length: 200 }, (_, index) => ({
						id: `row-${index}`,
					}))}
					columns={[{ id: "name", header: "Name", render: (row) => row.id }]}
					rowKey={(row) => row.id}
					onVisibleRowsChange={visibleRows}
					virtualize
					rowHeight={32}
				/>
			</div>,
		);
		const table = screen.getByRole("table");
		expect(table).toHaveAttribute("aria-rowcount", "201");
		expect(table).toHaveClass("virtualized");
		expect(table).toHaveStyle({ "--table-row-height": "32px" });
		const spacerHeight = Number.parseFloat(
			(document.querySelector(".ui-data-table-spacer") as HTMLElement).style
				.height,
		);
		expect(spacerHeight / 32).toBe(Math.trunc(spacerHeight / 32));
		expect(screen.getAllByRole("row").length).toBeLessThan(40);
		expect(screen.queryByText("row-199")).not.toBeInTheDocument();
		expect(visibleRows).toHaveBeenLastCalledWith(
			expect.arrayContaining([{ id: "row-0" }]),
		);
		expect(visibleRows.mock.lastCall?.[0].length).toBeLessThan(40);
	});
	it("leaves a scrolled virtualized list where the operator put it", () => {
		// jsdom implements no layout, so it ships no scrollIntoView to spy on.
		const scrollIntoView = vi.fn();
		(Element.prototype as { scrollIntoView?: unknown }).scrollIntoView =
			scrollIntoView;
		try {
			render(
				<div className="ui-window-scroller">
					<DataTable
						rows={Array.from({ length: 200 }, (_, index) => ({
							id: `row-${index}`,
						}))}
						columns={[{ id: "name", header: "Name", render: (row) => row.id }]}
						rowKey={(row) => row.id}
						activeIndex={5}
						virtualize
						rowHeight={32}
					/>
				</div>,
			);
			const scroller = document.querySelector(
				".ui-window-scroller",
			) as HTMLElement;
			// jsdom lays nothing out, so the scroller is given the metrics of a ten-row window
			// over two hundred rows. Without them every row measures as unmounted and the
			// re-assert this test is about could never fire.
			Object.defineProperty(scroller, "clientHeight", { value: 320 });
			Object.defineProperty(scroller, "scrollHeight", { value: 200 * 32 });
			// The table only re-asserts the active row once it holds focus, which is how an
			// operator leaves it after selecting a fixture.
			fireEvent.focus(screen.getAllByRole("row")[1], { bubbles: true });
			scrollIntoView.mockClear();

			// A short wheel gesture: far enough to republish the mounted row window, near
			// enough that the active row is still mounted and can be dragged back into view.
			scroller.scrollTop = 200;
			fireEvent.scroll(scroller);

			// The viewport changed, but nothing asked to be scrolled back into view.
			expect(scrollIntoView).not.toHaveBeenCalled();
			expect(scroller.scrollTop).toBe(200);
		} finally {
			(Element.prototype as { scrollIntoView?: unknown }).scrollIntoView =
				undefined;
		}
	});
	it("queues no mount scroll when StrictMode double-invokes the effects", () => {
		// jsdom implements no layout, so it ships no scrollIntoView to spy on.
		const scrollIntoView = vi.fn();
		(Element.prototype as { scrollIntoView?: unknown }).scrollIntoView =
			scrollIntoView;
		try {
			render(
				<StrictMode>
					<div className="ui-window-scroller">
						<DataTable
							rows={Array.from({ length: 200 }, (_, index) => ({
								id: `row-${index}`,
							}))}
							columns={[{ id: "name", header: "Name", render: (row) => row.id }]}
							rowKey={(row) => row.id}
							activeIndex={5}
							virtualize
							rowHeight={32}
						/>
					</div>
				</StrictMode>,
			);
			const scroller = document.querySelector(
				".ui-window-scroller",
			) as HTMLElement;
			Object.defineProperty(scroller, "clientHeight", { value: 320 });
			Object.defineProperty(scroller, "scrollHeight", { value: 200 * 32 });
			fireEvent.focus(screen.getAllByRole("row")[1], { bubbles: true });
			scrollIntoView.mockClear();

			scroller.scrollTop = 200;
			fireEvent.scroll(scroller);

			// A remount must not look like a move: the row the table opened on is where the
			// operator already is.
			expect(scrollIntoView).not.toHaveBeenCalled();
			expect(scroller.scrollTop).toBe(200);
		} finally {
			(Element.prototype as { scrollIntoView?: unknown }).scrollIntoView =
				undefined;
		}
	});
	it("still scrolls to a keyboard-selected row that the viewport has to mount", () => {
		// jsdom implements no layout, so it ships no scrollIntoView to spy on.
		const scrollIntoView = vi.fn();
		(Element.prototype as { scrollIntoView?: unknown }).scrollIntoView =
			scrollIntoView;
		try {
			const rows = Array.from({ length: 200 }, (_, index) => ({
				id: `row-${index}`,
			}));
			const table = (activeIndex: number) => (
				<div className="ui-window-scroller">
					<DataTable
						rows={rows}
						columns={[{ id: "name", header: "Name", render: (row) => row.id }]}
						rowKey={(row) => row.id}
						activeIndex={activeIndex}
						virtualize
						rowHeight={32}
					/>
				</div>
			);
			const view = render(table(0));
			fireEvent.focus(screen.getAllByRole("row")[1], { bubbles: true });
			scrollIntoView.mockClear();

			// Moving the active row to one that is already mounted scrolls to it directly.
			view.rerender(table(3));
			expect(scrollIntoView).toHaveBeenCalled();
			scrollIntoView.mockClear();

			const scroller = document.querySelector(
				".ui-window-scroller",
			) as HTMLElement;
			Object.defineProperty(scroller, "clientHeight", { value: 320 });
			Object.defineProperty(scroller, "scrollHeight", { value: 200 * 32 });

			// Moving far down the list picks a row the viewport has not mounted, so the move
			// cannot be honoured in the render that requested it.
			view.rerender(table(150));
			expect(scrollIntoView).not.toHaveBeenCalled();

			// The request stands. The viewport change that mounts the row honours it.
			scroller.scrollTop = 150 * 32;
			fireEvent.scroll(scroller);
			expect(scrollIntoView).toHaveBeenCalled();
		} finally {
			(Element.prototype as { scrollIntoView?: unknown }).scrollIntoView =
				undefined;
		}
	});
	it("stabilizes table width from declared pixel column minima", () => {
		render(
			<DataTable
				rows={[{ id: "one" }]}
				columns={[
					{ id: "exact", header: "Exact", width: "48px", render: () => "A" },
					{
						id: "flexible",
						header: "Flexible",
						width: "minmax(72px, 1fr)",
						render: () => "B",
					},
					{
						id: "unresolved",
						header: "Unresolved",
						width: "1fr",
						render: () => "C",
					},
				]}
				rowKey={(row) => row.id}
			/>,
		);

		expect(screen.getByRole("table")).toHaveStyle({ minWidth: "120px" });
	});
	it("exposes button grid states", () => {
		render(
			<ButtonGrid>
				<GridButton number="1" primary="Open" state="active" />
				<GridButton number="2" primary="Empty" state="empty" />
				<GridButton number="3" primary="Disabled" state="disabled" />
				<GridButton number="4" primary="Store" state="store-target" />
			</ButtonGrid>,
		);
		expect(screen.getByRole("button", { name: /Open/ })).toHaveClass("active");
		expect(screen.getByRole("button", { name: /Empty/ })).toHaveClass("empty");
		expect(screen.getByRole("button", { name: /Disabled/ })).toBeDisabled();
		expect(screen.getByRole("button", { name: /Store/ })).toHaveClass(
			"store-target",
		);
	});
	it("uses the untransformed column width for every button-grid row", () => {
		const realGetComputedStyle = window.getComputedStyle.bind(window);
		const computedStyle = vi
			.spyOn(window, "getComputedStyle")
			.mockImplementation((element) => {
				const style = realGetComputedStyle(element);
				if (element.tagName === "BUTTON")
					Object.defineProperty(style, "width", { value: "117.25px" });
				return style;
			});
		const rect = vi
			.spyOn(HTMLElement.prototype, "getBoundingClientRect")
			.mockImplementation(function (this: HTMLElement) {
				const width = this.tagName === "BUTTON" ? 114.319 : 400;
				return {
					x: 0,
					y: 0,
					top: 0,
					right: width,
					bottom: width,
					left: 0,
					width,
					height: width,
					toJSON: () => ({}),
				};
			});
		render(
			<ButtonGrid>
				<GridButton number="1" primary="One" />
				<GridButton number="2" primary="Two" />
			</ButtonGrid>,
		);
		expect(
			screen.getByRole("button", { name: /One/ }).parentElement,
		).toHaveStyle({ "--grid-row-size": "117.25px" });
		computedStyle.mockRestore();
		rect.mockRestore();
	});
	it("recalculates square rows after responsive width changes without reacting to height-only changes", () => {
		let width = 142;
		let notifyResize: () => void = () => undefined;
		const realResizeObserver = globalThis.ResizeObserver;
		const realGetComputedStyle = window.getComputedStyle.bind(window);
		globalThis.ResizeObserver = class {
			constructor(callback: ResizeObserverCallback) {
				notifyResize = () => callback([], this);
			}
			observe() {}
			unobserve() {}
			disconnect() {}
			takeRecords() {
				return [];
			}
		};
		const computedStyle = vi
			.spyOn(window, "getComputedStyle")
			.mockImplementation((element) => {
				const style = realGetComputedStyle(element);
				if (element.tagName === "BUTTON")
					Object.defineProperty(style, "width", { value: `${width}px` });
				return style;
			});
		render(
			<ButtonGrid>
				<GridButton number="1" primary="One" />
				<GridButton number="2" primary="Two" />
			</ButtonGrid>,
		);
		const grid = screen.getByRole("button", { name: /One/ }).parentElement;
		expect(grid).not.toBeNull();
		if (!grid) throw new Error("Button grid was not rendered");
		expect(grid).toHaveStyle({ "--grid-row-size": "142px" });

		width = 96.5;
		notifyResize();
		expect(grid).toHaveStyle({ "--grid-row-size": "96.5px" });

		notifyResize();
		expect(grid).toHaveStyle({ "--grid-row-size": "96.5px" });
		computedStyle.mockRestore();
		globalThis.ResizeObserver = realResizeObserver;
	});
	it("lets compact non-pool surfaces opt out of square row measurement", () => {
		render(
			<ButtonGrid square={false}>
				<GridButton number="1" primary="Shortcut" />
			</ButtonGrid>,
		);
		const grid = screen.getByRole("button", { name: /Shortcut/ }).parentElement;
		expect(grid).not.toBeNull();
		if (!grid) throw new Error("Compact button grid was not rendered");
		expect(grid).toHaveClass("compact-grid");
		expect(grid.style.getPropertyValue("--grid-row-size")).toBe("");
	});
	it("shows the unified empty state instead of window content", () => {
		render(
			<WindowScrollArea
				emptyState={{
					title: "Nothing here",
					description: "Add an item to get started.",
					icon: "◇",
				}}
			>
				<span>Hidden content</span>
			</WindowScrollArea>,
		);
		expect(screen.getByRole("status")).toHaveTextContent("Nothing here");
		expect(screen.getByText("Add an item to get started.")).toBeInTheDocument();
		expect(screen.queryByText("Hidden content")).not.toBeInTheDocument();
	});
});
