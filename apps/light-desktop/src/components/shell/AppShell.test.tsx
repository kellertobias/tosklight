import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AppShellView } from "./AppShell";

describe("AppShellView", () => {
	it("keeps ready desk controls interactive while show loading is visible", () => {
		const operate = vi.fn();
		render(
			<AppShellView
				dock={<nav>Dock</nav>}
				workspace={<button onClick={operate}>Fixture Sheet action</button>}
				control={<button onClick={operate}>Programmer action</button>}
				loadingOverlay={
					<div className="show-loading-cover" role="status" aria-busy="true">
						Loading show Festival…
					</div>
				}
			/>,
		);

		expect(screen.getByRole("status")).toHaveTextContent(
			"Loading show Festival…",
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Fixture Sheet action" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Programmer action" }));
		expect(operate).toHaveBeenCalledTimes(2);
	});
});
