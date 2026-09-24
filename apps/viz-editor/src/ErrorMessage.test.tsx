import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CadToolError } from "./cad/CadToolbar";
import type { CadTools } from "./cad/cadTools";
import { ErrorMessage } from "./ErrorMessage";

describe("an error message", () => {
	it("copies its words and stays until it is closed with ×", async () => {
		const writeText = vi.fn().mockResolvedValue(undefined);
		Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
		const onDismiss = vi.fn();
		render(<ErrorMessage className="cad-error" message="The show refused the truss: revision 4" onDismiss={onDismiss} />);
		const words = screen.getByText("The show refused the truss: revision 4");
		// Clicking or selecting the words keeps the message, so it can be copied by hand too.
		fireEvent.click(words);
		expect(onDismiss).not.toHaveBeenCalled();
		fireEvent.click(screen.getByRole("button", { name: "Copy error" }));
		expect(writeText).toHaveBeenCalledWith("The show refused the truss: revision 4");
		await waitFor(() => expect(screen.getByRole("button", { name: "Copy error" })).toHaveTextContent("Copied"));
		fireEvent.click(screen.getByRole("button", { name: "Dismiss error" }));
		expect(onDismiss).toHaveBeenCalledTimes(1);
	});

	it("gives a drawing tool's error the same Copy and ×", () => {
		const clearError = vi.fn();
		render(<CadToolError tools={{ error: "The fixture library could not be read", clearError } as unknown as CadTools} />);
		fireEvent.click(screen.getByText("The fixture library could not be read"));
		expect(clearError).not.toHaveBeenCalled();
		fireEvent.click(screen.getByRole("button", { name: "Dismiss error" }));
		expect(clearError).toHaveBeenCalledTimes(1);
	});
});
