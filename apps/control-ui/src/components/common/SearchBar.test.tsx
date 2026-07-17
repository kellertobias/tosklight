import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Button } from "./controls";
import { SearchBar } from "./SearchBar";

describe("SearchBar", () => {
	it("keeps magnifier, text, conditional clear, keyboard, and options in order", () => {
		const change = vi.fn();
		const { container, rerender, unmount } = render(
			<SearchBar value="" onChange={change} />,
		);

		const bar = container.querySelector(".console-search")!;
		expect(bar.querySelector(".console-search-icon")).toBeInTheDocument();
		expect(screen.getByRole("textbox", { name: "Search" })).toBeInTheDocument();
		expect(screen.queryByRole("button", { name: "Clear search" })).not.toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Open keyboard" })).toBeInTheDocument();
		expect(screen.queryByRole("button", { name: "Search options" })).not.toBeInTheDocument();

		rerender(<SearchBar value="orbit" onChange={change} options={<p>Option content</p>} />);
		const controls = [...bar.querySelectorAll("input, button")];
		expect(controls.map((control) => control.getAttribute("aria-label"))).toEqual([
			"Search",
			"Clear search",
			"Open keyboard",
			"Search options",
		]);
		fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
		expect(change).toHaveBeenCalledWith("");
		unmount();
	});

	it("opens custom options as a body-level modal and restores trigger focus", async () => {
		const { container } = render(
			<header>
				<SearchBar
					value=""
					onChange={vi.fn()}
					options={<Button>Custom option</Button>}
				/>
			</header>,
		);

		const trigger = screen.getByRole("button", { name: "Search options" });
		fireEvent.click(trigger);

		const dialog = screen.getByRole("dialog", { name: "Search options" });
		expect(document.body).toContainElement(dialog);
		expect(container.querySelector("header")).not.toContainElement(dialog);
		expect(screen.getByRole("button", { name: "Custom option" })).toBeInTheDocument();
		fireEvent.keyDown(document, { key: "Escape" });
		expect(screen.queryByRole("dialog", { name: "Search options" })).not.toBeInTheDocument();
		await waitFor(() => expect(trigger).toHaveFocus());
	});
});
