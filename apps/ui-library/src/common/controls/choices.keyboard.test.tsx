import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { SelectField } from "./choices";

afterEach(cleanup);
it.each([["End", "last"], ["Home", "first"]] as const)(
	"selects the %s option without leaking its key to desk shortcuts",
	(key, expected) => {
		const change = vi.fn();
		const shortcut = vi.fn();
		document.addEventListener("keydown", shortcut);
		try {
			render(<SelectField ariaLabel="Page source" value="middle"
				options={[{ value: "first", label: "First" }, { value: "middle", label: "Middle" }, { value: "last", label: "Last" }]}
				onChange={change} />);
			const trigger = screen.getByRole("button", { name: "Page source" });
			fireEvent.click(trigger);
			const event = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true });
			fireEvent(trigger, event);
			expect.soft(event.defaultPrevented).toBe(true);
			expect.soft(shortcut).not.toHaveBeenCalled();
			fireEvent.keyDown(trigger, { key: "Enter" });
			expect(change).toHaveBeenCalledExactlyOnceWith(expected);
			expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
		} finally {
			document.removeEventListener("keydown", shortcut);
		}
	},
);
it("leaves Home and End available to the desk when the picker is closed", () => {
	const shortcut = vi.fn();
	document.addEventListener("keydown", shortcut);
	try {
		render(<SelectField ariaLabel="Page source" value="own"
			options={[{ value: "own", label: "Own" }]} onChange={vi.fn()} />);
		const trigger = screen.getByRole("button", { name: "Page source" });
		for (const key of ["Home", "End"]) {
			const event = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true });
			fireEvent(trigger, event);
			expect(event.defaultPrevented).toBe(false);
		}
		expect(shortcut).toHaveBeenCalledTimes(2);
	} finally {
		document.removeEventListener("keydown", shortcut);
	}
});

it.each([["Home", "first"], ["End", "last"]] as const)(
	"skips disabled boundary options on %s",
	(key, expected) => {
		const change = vi.fn();
		render(<SelectField ariaLabel="Page source" value="middle"
			options={[
				{ value: "disabled-first", label: "Unavailable first", disabled: true },
				{ value: "first", label: "First" }, { value: "middle", label: "Middle" },
				{ value: "last", label: "Last" },
				{ value: "disabled-last", label: "Unavailable last", disabled: true },
			]} onChange={change} />);
		const trigger = screen.getByRole("button", { name: "Page source" });
		fireEvent.click(trigger);
		fireEvent.keyDown(trigger, { key });
		fireEvent.keyDown(trigger, { key: "Enter" });
		expect(change).toHaveBeenCalledExactlyOnceWith(expected);
	},
);
