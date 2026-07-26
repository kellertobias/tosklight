import { fireEvent, render as rtlRender, screen, waitFor, type RenderOptions } from "@testing-library/react";
import type { ReactElement } from "react";
import { describe, expect, it, vi } from "vitest";
import { ModalProvider } from "../modals/ModalStack";
import { SearchBar } from "./SearchBar";

function render(ui: ReactElement, options?: Omit<RenderOptions, "wrapper">) {
	return rtlRender(ui, { wrapper: ModalProvider, ...options });
}

describe("SearchBar", () => {
	it("uses a wider leading magnifier as the optional options trigger", () => {
		const change = vi.fn();
		const { container, rerender, unmount } = render(
			<SearchBar value="" onChange={change} />,
		);

		const bar = container.querySelector(".console-search")!;
		expect(bar.querySelector(".console-search-icon")).toBeInTheDocument();
		expect(screen.getByRole("textbox", { name: "Search" })).toBeInTheDocument();
		expect(screen.queryByRole("button", { name: "Clear search" })).not.toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Open keyboard" })).toBeInTheDocument();
		expect(screen.queryByRole("button", { name: "Search settings" })).not.toBeInTheDocument();

		rerender(<SearchBar value="orbit" onChange={change} settings={[{
			kind: "toggle",
			id: "favorites",
			label: "Favorites only",
			value: false,
		}]} />);
		const controls = [...bar.querySelectorAll("input, button")];
		expect(controls.map((control) => control.getAttribute("aria-label"))).toEqual([
			"Search settings",
			"Search",
			"Clear search",
			"Open keyboard",
		]);
		expect(bar.querySelector(".console-search-chevron")).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
		expect(change).toHaveBeenCalledWith("");
		unmount();
	});

	it("renders typed settings in a body-level modal and restores trigger focus", async () => {
		const settingChange = vi.fn();
		const clearSettings = vi.fn();
		const { container } = render(
			<header>
				<SearchBar
					value=""
					onChange={vi.fn()}
					settingsTitle="Fixture search settings"
					settings={[
						{
							kind: "select",
							id: "type",
							label: "Fixture type",
							value: "",
							options: [
								{ value: "", label: "All" },
								{ value: "moving", label: "Moving light" },
							],
						},
						{
							kind: "toggle",
							id: "favorites",
							label: "Favorites only",
							value: false,
						},
					]}
					onSettingChange={settingChange}
					onClearSettings={clearSettings}
				/>
			</header>,
		);

		const trigger = screen.getByRole("button", { name: "Search settings" });
		trigger.focus();
		fireEvent.click(trigger);

		const dialog = screen.getByRole("dialog", { name: "Fixture search settings" });
		expect(document.body).toContainElement(dialog);
		expect(container.querySelector("header")).not.toContainElement(dialog);
		fireEvent.click(screen.getByRole("switch", { name: "Favorites only" }));
		expect(settingChange).toHaveBeenCalledWith("favorites", true);
		fireEvent.click(screen.getByRole("button", { name: "Clear settings" }));
		expect(clearSettings).toHaveBeenCalledOnce();
		expect(screen.queryByRole("button", { name: /Apply|Save/ })).not.toBeInTheDocument();
		fireEvent.keyDown(document, { key: "Escape" });
		expect(screen.queryByRole("dialog", { name: "Fixture search settings" })).not.toBeInTheDocument();
		await waitFor(() => expect(trigger).toHaveFocus());
	});
});
