import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ModalTitleBar } from "./ModalTitleBar";

describe("ModalTitleBar", () => {
	afterEach(cleanup);
	it("renders a continuous title and close control", () => {
		const close = vi.fn();
		render(
			<ModalTitleBar
				title="Number input"
				details={
					<>
						<b>Choose a number</b>
						<small>Current value: 1</small>
					</>
				}
				onClose={close}
			/>,
		);
		expect(
			screen.getByRole("heading", { name: "Number input" }),
		).toBeInTheDocument();
		expect(screen.getByText("Choose a number")).toBeVisible();
		expect(screen.getByText("Current value: 1")).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: "Close modal" }));
		expect(close).toHaveBeenCalledOnce();
	});

	it("keeps right-aligned search before every modal action and close", () => {
		const select = vi.fn();
		const { container } = render(
			<ModalTitleBar
				title="Settings"
				groups={[
					{
						id: "settings",
						kind: "tabs",
						activeId: "general",
						onActiveChange: select,
						actions: [
							{ id: "general", label: "General" },
							{ id: "output", label: "Output" },
						],
					},
				]}
				search={{ value: "", onSearch: vi.fn(), ariaLabel: "Search settings" }}
				accept={{ id: "reset", label: "Reset", onPress: vi.fn() }}
				onClose={vi.fn()}
			/>,
		);
		expect(
			screen.getByRole("heading", { name: "Settings" }),
		).toBeInTheDocument();
		expect(screen.getByRole("tab", { name: "General" })).toHaveAttribute(
			"aria-selected",
			"true",
		);
		fireEvent.click(screen.getByRole("tab", { name: "Output" }));
		expect(select).toHaveBeenCalledWith("output");
		const titlebar = container.querySelector(".ui-modal-titlebar")!;
		expect([...titlebar.children].map((child) => child.className)).toEqual([
			"ui-modal-title-copy",
			"ui-modal-title-spacer",
			"ui-title-chrome",
		]);
		const chrome = titlebar.querySelector(".ui-title-chrome")!;
		expect([...chrome.children].map((child) => child.className)).toEqual([
			"ui-title-chrome-search ui-modal-title-search",
			"ui-title-chrome-search-toggle",
			"ui-title-chrome-search-close",
			"ui-title-chrome-groups",
			"ui-title-chrome-terminals ui-modal-title-terminals",
		]);
		expect(screen.getByText("Reset")).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Close modal" }),
		).toBeInTheDocument();
	});

	it("omits search dividers when only passive title content surrounds search", () => {
		const { container } = render(
			<ModalTitleBar
				title="Search only"
				search={{ value: "", onSearch: vi.fn() }}
			/>,
		);
		expect(
			container.querySelectorAll(".ui-titlebar-search-divider"),
		).toHaveLength(0);
	});
});
