import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
} from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	fallbackKeyboardLayout,
	ModalCaretValue,
	ModalNumberInput,
	ModalNumberValue,
	ModalTextKeyboard,
} from "./ModalInputControls";

afterEach(cleanup);

function TextHarness({
	enter,
	escape,
	multiline = false,
}: {
	enter: () => void;
	escape: () => void;
	multiline?: boolean;
}) {
	const [value, setValue] = useState("");
	return (
		<div className="modal-backdrop">
			<output aria-label="value">{value}</output>
			<ModalTextKeyboard
				value={value}
				onChange={setValue}
				onEnter={enter}
				onEscape={escape}
				multiline={multiline}
			/>
		</div>
	);
}

function NumberHarness({
	enter,
	escape,
	initial = "",
	replaceOnFirstInput = false,
	allowDecimal = true,
	allowThrough = false,
}: {
	enter: () => void;
	escape: () => void;
	initial?: string;
	replaceOnFirstInput?: boolean;
	allowDecimal?: boolean;
	allowThrough?: boolean;
}) {
	const [value, setValue] = useState(initial);
	const [caret, setCaret] = useState(initial.length);
	return (
		<div className="modal-backdrop">
			<output aria-label="value">{value}</output>
			<ModalNumberValue
				value={value}
				caret={caret}
				onCaretChange={setCaret}
				ariaLabel="Number value"
			/>
			<ModalNumberInput
				value={value}
				caret={caret}
				onChange={setValue}
				onCaretChange={setCaret}
				onEnter={enter}
				onEscape={escape}
				replaceOnFirstInput={replaceOnFirstInput}
				allowDecimal={allowDecimal}
				allowThrough={allowThrough}
			/>
		</div>
	);
}

describe("modal input controls", () => {
	it("provides a German QWERTZ fallback with umlauts", () => {
		const layout = fallbackKeyboardLayout("de-DE");
		expect(layout.KeyY).toBe("Z");
		expect(layout.KeyZ).toBe("Y");
		expect([layout.BracketLeft, layout.Semicolon, layout.Quote]).toEqual([
			"Ü",
			"Ö",
			"Ä",
		]);
		expect(layout.Minus).toBe("ß");
	});
	it("routes the physical keyboard to the text control", () => {
		const enter = vi.fn();
		const escape = vi.fn();
		render(<TextHarness enter={enter} escape={escape} />);
		for (const key of ["T", "e", "s", "t", "@", "1", ".", "_", "-"])
			fireEvent.keyDown(window, { key });
		expect(screen.getByLabelText("value")).toHaveTextContent("Test@1._-");
		fireEvent.keyDown(window, { key: "Backspace" });
		fireEvent.keyDown(window, { key: "Enter" });
		fireEvent.keyDown(window, { key: "Escape" });
		expect(screen.getByLabelText("value")).toHaveTextContent("Test@1._");
		expect(enter).toHaveBeenCalledOnce();
		expect(escape).toHaveBeenCalledOnce();
		expect(
			screen.getByRole("button", { name: "Enter · Confirm" }),
		).toBeVisible();
	});

	it("moves the text caret and inserts or deletes at that position", () => {
		render(<TextHarness enter={vi.fn()} escape={vi.fn()} />);
		for (const key of ["A", "C"]) fireEvent.keyDown(window, { key });
		fireEvent.click(screen.getByRole("button", { name: "Move cursor left" }));
		fireEvent.keyDown(window, { key: "B" });
		expect(screen.getByLabelText("value")).toHaveTextContent("ABC");
		fireEvent.keyDown(window, { key: "ArrowLeft" });
		fireEvent.keyDown(window, { key: "Backspace" });
		expect(screen.getByLabelText("value")).toHaveTextContent("BC");
		fireEvent.click(screen.getByRole("button", { name: "Move cursor right" }));
		fireEvent.click(screen.getByRole("button", { name: "SPACE" }));
		expect(screen.getByLabelText("value")).toHaveTextContent("B C");
	});

	it("places regular Backspace in the action rail above Enter", () => {
		const { container } = render(
			<TextHarness enter={vi.fn()} escape={vi.fn()} />,
		);
		const backspace = screen.getByRole("button", { name: "Backspace" });
		const enter = screen.getByRole("button", { name: "Enter · Confirm" });
		const rail = container.querySelector(".modal-keyboard-actions");
		expect(backspace.parentElement).toBe(rail);
		expect(enter.parentElement).toBe(rail);
		expect([...rail!.children]).toEqual([backspace, enter]);
		expect(
			container.querySelector(".modal-keyboard-bottom .backspace"),
		).not.toBeInTheDocument();
	});

	it("keeps multiline Backspace above the remaining Enter action space", () => {
		const { container } = render(
			<TextHarness enter={vi.fn()} escape={vi.fn()} multiline />,
		);
		const backspace = screen.getByRole("button", { name: "Backspace" });
		const enter = screen.getByRole("button", { name: "Enter · New line" });
		const rail = container.querySelector(".modal-keyboard-actions.multiline");
		expect([...rail!.children]).toEqual([backspace, enter]);
		expect(backspace.querySelector("small")).toHaveTextContent("Backspace");
		expect(enter).toHaveClass("newline");
	});

	it("renders an explicit multiline caret independently of the native textarea", () => {
		const { container } = render(
			<ModalCaretValue
				value={"First line\nSecond line"}
				caret={13}
				multiline
				ariaLabel="Notes value"
				onCaretChange={() => undefined}
			/>,
		);
		const layer = container.querySelector(".modal-multiline-caret-layer");
		expect(layer).toHaveTextContent(/First line\s+Second line/u);
		expect(
			layer?.querySelector(".modal-multiline-caret-content > i"),
		).toBeInTheDocument();
		expect(screen.getByRole("textbox", { name: "Notes value" })).toHaveValue(
			"First line\nSecond line",
		);
	});

	it("supports one-shot Shift, cancellation, locking, and unlocking", () => {
		vi.useFakeTimers();
		try {
			render(<TextHarness enter={vi.fn()} escape={vi.fn()} />);
			const shift = screen.getByRole("button", { name: "Shift" });
			fireEvent.click(shift);
			expect(shift).toHaveAttribute("data-shift-state", "one-shot");
			const letterA = screen.getByRole("button", { name: "A" });
			fireEvent.click(letterA);
			expect(screen.getByLabelText("value")).toHaveTextContent("A");
			expect(shift).toHaveAttribute("data-shift-state", "inactive");
			expect(letterA).toHaveAttribute("data-keyboard-pressed", "true");
			act(() => vi.advanceTimersByTime(140));
			expect(letterA).not.toHaveAttribute("data-keyboard-pressed");

			fireEvent.click(shift);
			fireEvent.click(shift);
			expect(shift).toHaveAttribute("data-shift-state", "inactive");

			fireEvent.pointerDown(shift);
			act(() => vi.advanceTimersByTime(500));
			fireEvent.pointerUp(shift);
			expect(shift).toHaveAttribute("data-shift-state", "locked");
			fireEvent.click(screen.getByRole("button", { name: "B" }));
			fireEvent.click(screen.getByRole("button", { name: "C" }));
			expect(screen.getByLabelText("value")).toHaveTextContent("ABC");
			fireEvent.click(shift);
			expect(shift).toHaveAttribute("data-shift-state", "inactive");
		} finally {
			vi.useRealTimers();
		}
	});

	it("places the wider icon Shift key beside the physical Y/Z row", () => {
		const { container } = render(
			<TextHarness enter={vi.fn()} escape={vi.fn()} />,
		);
		const shift = screen.getByRole("button", { name: "Shift" });
		const physicalZ = container.querySelector('[data-keyboard-code="KeyZ"]');
		expect(shift.parentElement).toBe(physicalZ?.parentElement);
		expect(shift).toHaveClass("shift");
		expect(shift.querySelector(".modal-shift-icon")).toBeInTheDocument();
		expect(shift).not.toHaveTextContent("SHIFT");
	});

	it("shares one-shot Shift state with the physical Shift key", () => {
		render(<TextHarness enter={vi.fn()} escape={vi.fn()} />);
		fireEvent.keyDown(window, { key: "Shift" });
		fireEvent.keyUp(window, { key: "Shift" });
		expect(screen.getByRole("button", { name: "Shift" })).toHaveAttribute(
			"data-shift-state",
			"one-shot",
		);
		fireEvent.keyDown(window, { key: "a" });
		expect(screen.getByLabelText("value")).toHaveTextContent("A");
		expect(screen.getByRole("button", { name: "Shift" })).toHaveAttribute(
			"data-shift-state",
			"inactive",
		);
	});

	it("accepts digits and one decimal point in the number control", () => {
		const enter = vi.fn();
		const escape = vi.fn();
		render(<NumberHarness enter={enter} escape={escape} />);
		for (const key of ["1", "2", ".", "5", ".", "x"])
			fireEvent.keyDown(window, { key });
		expect(screen.getByLabelText("value")).toHaveTextContent("12.5");
		fireEvent.keyDown(window, { key: "Enter" });
		fireEvent.keyDown(window, { key: "Escape" });
		expect(enter).toHaveBeenCalledOnce();
		expect(escape).toHaveBeenCalledOnce();
	});

	it("uses minus as a sign toggle for negative placement values", () => {
		render(
			<NumberHarness enter={vi.fn()} escape={vi.fn()} replaceOnFirstInput />,
		);
		fireEvent.click(screen.getByRole("button", { name: "−" }));
		fireEvent.keyDown(window, { key: "5" });
		expect(screen.getByLabelText("value")).toHaveTextContent("-5");
		fireEvent.click(screen.getByRole("button", { name: "−" }));
		expect(screen.getByLabelText("value")).toHaveTextContent("5");
	});

	it("uses the operator num-block layout and replaces an existing value on first entry", () => {
		render(
			<NumberHarness
				enter={vi.fn()}
				escape={vi.fn()}
				initial="62.8"
				replaceOnFirstInput
				allowThrough
			/>,
		);
		const keypad = screen.getByLabelText("Number input keypad");
		expect([...keypad.children].map((key) => key.textContent)).toEqual([
			"ESC",
			"7",
			"8",
			"9",
			"⌫",
			"+",
			"4",
			"5",
			"6",
			"THRU",
			"DIV",
			"1",
			"2",
			"3",
			"ENTER",
			"−",
			".",
			"0",
			"AT",
		]);
		expect(screen.getByRole("button", { name: "ESC" })).toHaveStyle({
			gridColumn: "1",
			gridRow: "1",
		});
		expect(screen.getByRole("button", { name: "⌫" })).toHaveStyle({
			gridColumn: "5",
			gridRow: "1",
		});
		expect(screen.getByRole("button", { name: "+" })).toHaveStyle({
			gridColumn: "1",
			gridRow: "2",
		});
		expect(screen.getByRole("button", { name: "THRU" })).toHaveStyle({
			gridColumn: "5",
			gridRow: "2",
		});
		expect(screen.getByRole("button", { name: "AT" })).toHaveStyle({
			gridColumn: "4",
			gridRow: "4",
		});
		expect(screen.getByRole("button", { name: "ENTER" })).toHaveStyle({
			gridRow: "3 / span 2",
		});
		fireEvent.keyDown(window, { key: "9" });
		fireEvent.keyDown(window, { key: "5" });
		expect(screen.getByLabelText("value")).toHaveTextContent("95");
	});

	it("moves the number caret without treating cursor-left as backspace", () => {
		const { container } = render(
			<NumberHarness
				enter={vi.fn()}
				escape={vi.fn()}
				initial="13"
				replaceOnFirstInput
			/>,
		);
		const valueRow = container.querySelector(".modal-number-value");
		const keypad = screen.getByLabelText("Number input keypad");
		const left = screen.getByRole("button", { name: "Move cursor left" });
		const right = screen.getByRole("button", { name: "Move cursor right" });
		expect(left.parentElement).toHaveClass("modal-number-cursors");
		expect(right.parentElement).toBe(left.parentElement);
		expect(valueRow).toContainElement(left);
		expect(valueRow).toContainElement(right);
		expect(keypad).not.toContainElement(left);
		expect(keypad).not.toContainElement(right);
		fireEvent.click(screen.getByRole("button", { name: "Move cursor left" }));
		fireEvent.click(screen.getByRole("button", { name: "2" }));
		expect(screen.getByLabelText("value")).toHaveTextContent("123");
		fireEvent.click(screen.getByRole("button", { name: "Move cursor right" }));
		fireEvent.click(screen.getByRole("button", { name: "⌫" }));
		expect(screen.getByLabelText("value")).toHaveTextContent("12");
	});

	it("removes unsupported decimal and THRU buttons without collapsing their grid spaces", () => {
		render(
			<NumberHarness enter={vi.fn()} escape={vi.fn()} allowDecimal={false} />,
		);
		expect(screen.queryByRole("button", { name: "." })).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "THRU" }),
		).not.toBeInTheDocument();
		expect(screen.getByRole("button", { name: "6" })).toHaveStyle({
			gridColumn: "4",
			gridRow: "2",
		});
		expect(screen.getByRole("button", { name: "0" })).toHaveStyle({
			gridColumn: "3",
			gridRow: "4",
		});
		expect(screen.getByRole("button", { name: "AT" })).toHaveStyle({
			gridColumn: "4",
			gridRow: "4",
		});
	});

	it("builds a THRU expression when value spreading is enabled", () => {
		const enter = vi.fn();
		render(
			<NumberHarness
				enter={enter}
				escape={vi.fn()}
				initial="75"
				replaceOnFirstInput
				allowThrough
			/>,
		);
		fireEvent.click(screen.getByRole("button", { name: "0" }));
		fireEvent.click(screen.getByRole("button", { name: "THRU" }));
		fireEvent.click(screen.getByRole("button", { name: "5" }));
		fireEvent.click(screen.getByRole("button", { name: "0" }));
		expect(screen.getByLabelText("value")).toHaveTextContent("0 THRU 50");
		fireEvent.click(screen.getByRole("button", { name: "ENTER" }));
		expect(enter).toHaveBeenCalledOnce();
	});

	it("applies the minus sign independently to both sides of a THRU expression", () => {
		render(<NumberHarness enter={vi.fn()} escape={vi.fn()} allowThrough />);
		for (const key of ["−", "4", "THRU", "−", "3"])
			fireEvent.click(screen.getByRole("button", { name: key }));
		expect(screen.getByLabelText("value")).toHaveTextContent("-4 THRU -3");
	});

	it("closes the number input from its ESC button", () => {
		const escape = vi.fn();
		render(<NumberHarness enter={vi.fn()} escape={escape} />);
		fireEvent.click(screen.getByRole("button", { name: "ESC" }));
		expect(escape).toHaveBeenCalledOnce();
	});
});
