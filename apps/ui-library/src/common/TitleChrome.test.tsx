import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ModalProvider } from "../modals/ModalStack";
import { ModalTitleBar } from "./ModalTitleBar";
import { TitleChrome, validateTitleAction } from "./TitleChrome";

const renderChrome = (ui: ReactElement) =>
	render(<ModalProvider>{ui}</ModalProvider>);

describe("TitleChrome", () => {
	afterEach(cleanup);

	it("uses one controlled tab-group contract and keeps modal Accept beside Close", () => {
		const select = vi.fn();
		const accept = vi.fn();
		const close = vi.fn();
		const { container } = renderChrome(
			<ModalTitleBar
				title="Cues"
				groups={[
					{
						id: "mode",
						kind: "tabs",
						activeId: "select",
						onActiveChange: select,
						actions: [
							{ id: "select", label: "Select" },
							{ id: "navigate", label: "Navigate" },
						],
					},
				]}
				accept={{ id: "accept", label: "Apply", onPress: accept }}
				onClose={close}
			/>,
		);
		expect(screen.getByRole("tab", { name: "Select" })).toHaveAttribute(
			"aria-selected",
			"true",
		);
		fireEvent.click(screen.getByRole("tab", { name: "Navigate" }));
		expect(select).toHaveBeenCalledWith("navigate");
		const terminals = container.querySelector(".ui-title-chrome-terminals");
		expect(
			[...terminals!.querySelectorAll("button")].map(
				(button) => button.getAttribute("aria-label") ?? button.textContent,
			),
		).toEqual(["Apply", "Close modal"]);
	});

	it("keeps controlled toggles open, closes action rows, and supports custom close", () => {
		const toggle = vi.fn();
		const action = vi.fn();
		const { rerender } = renderChrome(
			<TitleChrome
				groups={[
					{
						id: "menu",
						actions: [
							{
								id: "open",
								kind: "dropdown",
								label: "Menu",
								dropdown: {
									kind: "items",
									items: [
										{
											kind: "toggle",
											id: "follow",
											label: "Follow",
											checked: false,
											onChange: toggle,
										},
										{
											kind: "action",
											id: "add",
											label: "Add Cue",
											onPress: action,
										},
									],
								},
							},
						],
					},
				]}
				terminalActions={[]}
			/>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Menu" }));
		fireEvent.click(screen.getByRole("menuitemcheckbox", { name: "Follow" }));
		expect(toggle).toHaveBeenCalledWith(true);
		expect(screen.getByRole("menu")).toBeInTheDocument();
		fireEvent.click(screen.getByRole("menuitem", { name: "Add Cue" }));
		expect(action).toHaveBeenCalledOnce();
		expect(screen.queryByRole("menu")).not.toBeInTheDocument();

		rerender(
			<ModalProvider>
				<TitleChrome
					groups={[
						{
							id: "custom",
							actions: [
								{
									id: "open-custom",
									kind: "dropdown",
									label: "Custom",
									dropdown: {
										kind: "content",
										ariaLabel: "Custom content",
										render: ({ close }) => (
											<button type="button" onClick={close}>
												Done
											</button>
										),
									},
								},
							],
						},
					]}
					terminalActions={[]}
				/>
			</ModalProvider>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Custom" }));
		fireEvent.click(screen.getByRole("button", { name: "Done" }));
		expect(screen.queryByRole("menu")).not.toBeInTheDocument();
	});

	it("dismisses dropdowns with Escape and an outside press", () => {
		renderChrome(
			<TitleChrome
				groups={[
					{
						id: "menu",
						actions: [
							{
								id: "open",
								kind: "dropdown",
								label: "Menu",
								dropdown: { kind: "items", items: [] },
							},
						],
					},
				]}
				terminalActions={[]}
			/>,
		);
		const trigger = screen.getByRole("button", { name: "Menu" });
		fireEvent.click(trigger);
		fireEvent.keyDown(window, { key: "Escape" });
		expect(screen.queryByRole("menu")).not.toBeInTheDocument();
		fireEvent.click(trigger);
		fireEvent.pointerDown(
			document.querySelector(".ui-title-chrome-dropdown-layer")!,
		);
		expect(screen.queryByRole("menu")).not.toBeInTheDocument();
	});

	it("enforces action content and icon-only accessible names", () => {
		expect(() => validateTitleAction({ label: null })).toThrow(
			"require a label, an icon, or both",
		);
		expect(() =>
			validateTitleAction({ icon: <span>+</span> } as never),
		).toThrow("require ariaLabel");
		const { container } = renderChrome(
			<TitleChrome
				groups={[
					{
						id: "content",
						actions: [
							{ id: "label", label: "Label" },
							{ id: "icon", icon: "+", ariaLabel: "Add" },
							{ id: "both", label: "Both", icon: "+" },
						],
					},
				]}
				terminalActions={[]}
			/>,
		);
		expect(screen.getByRole("button", { name: "Add" })).toHaveClass(
			"is-icon-only",
		);
		expect(screen.getByRole("button", { name: "Both" })).not.toHaveClass(
			"is-icon-only",
		);
		expect(container.querySelectorAll(".is-icon-only")).toHaveLength(1);
	});

	it("shows search settings only when custom settings exist", () => {
		const { rerender } = renderChrome(
			<TitleChrome
				search={{ value: "", onSearch: vi.fn(), ariaLabel: "Search cues" }}
				terminalActions={[]}
			/>,
		);
		expect(
			screen.queryByRole("button", { name: "Search cues settings" }),
		).not.toBeInTheDocument();
		rerender(
			<ModalProvider>
				<TitleChrome
					search={{
						value: "",
						onSearch: vi.fn(),
						ariaLabel: "Search cues",
						settings: <span>Only active cues</span>,
					}}
					terminalActions={[]}
				/>
			</ModalProvider>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Search settings" }));
		expect(screen.getByText("Only active cues")).toBeInTheDocument();
	});

	it("replaces right-side actions with compact search and restores them", () => {
		const close = vi.fn();
		renderChrome(
			<ModalTitleBar
				title="Cues"
				search={{ value: "front", onSearch: vi.fn(), ariaLabel: "Search cues" }}
				groups={[
					{
						id: "actions",
						actions: [{ id: "add", label: "Add Cue" }],
					},
				]}
				onClose={close}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Open Search cues" }));
		expect(screen.getByRole("heading", { name: "Cues" })).toBeInTheDocument();
		expect(screen.getByRole("textbox", { name: "Search cues" })).toHaveFocus();
		expect(screen.queryByRole("button", { name: "Add Cue" })).toBeNull();
		expect(screen.queryByRole("button", { name: "Close modal" })).toBeNull();

		fireEvent.click(screen.getByRole("button", { name: "Close search" }));
		expect(screen.getByRole("button", { name: "Add Cue" })).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Close modal" }),
		).toBeInTheDocument();
		expect(close).not.toHaveBeenCalled();
	});
});
