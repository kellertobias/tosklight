import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ModalTitleBar } from "./ModalTitleBar";

describe("ModalTitleBar", () => {
	it("renders a continuous title and close control", () => {
		const close = vi.fn();
		render(<ModalTitleBar title="Number input" details={<><b>Choose a number</b><small>Current value: 1</small></>} onClose={close} />);
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
				tabs={[
					{ id: "general", label: "General" },
					{ id: "output", label: "Output" },
				]}
				activeTab="general"
				onTabChange={select}
				search={<span data-testid="search">Search</span>}
				actions={<span>Reset</span>}
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
			"ui-modal-title-heading",
			"ui-modal-title-tabs",
			"ui-modal-title-spacer",
			"ui-modal-title-search",
			"ui-modal-title-actions",
			"ui-button ui-secondary ui-default ui-modal-title-close",
		]);
	});
});
