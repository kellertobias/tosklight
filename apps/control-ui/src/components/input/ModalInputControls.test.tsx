import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { fallbackKeyboardLayout, ModalNumberInput, ModalTextKeyboard } from "./ModalInputControls";

afterEach(cleanup);

function TextHarness({ enter, escape }: { enter: () => void; escape: () => void }) {
  const [value, setValue] = useState("");
  return <div className="modal-backdrop"><output aria-label="value">{value}</output><ModalTextKeyboard value={value} onChange={setValue} onEnter={enter} onEscape={escape}/></div>;
}

function NumberHarness({ enter, escape, initial = "", replaceOnFirstInput = false, allowDecimal = true }: { enter: () => void; escape: () => void; initial?: string; replaceOnFirstInput?: boolean; allowDecimal?: boolean }) {
  const [value, setValue] = useState(initial);
  return <div className="modal-backdrop"><output aria-label="value">{value}</output><ModalNumberInput value={value} onChange={setValue} onEnter={enter} onEscape={escape} replaceOnFirstInput={replaceOnFirstInput} allowDecimal={allowDecimal}/></div>;
}

describe("modal input controls", () => {
  it("provides a German QWERTZ fallback with umlauts", () => {
    const layout = fallbackKeyboardLayout("de-DE");
    expect(layout.KeyY).toBe("Z");
    expect(layout.KeyZ).toBe("Y");
    expect([layout.BracketLeft, layout.Semicolon, layout.Quote]).toEqual(["Ü", "Ö", "Ä"]);
    expect(layout.Minus).toBe("ß");
  });
  it("routes the physical keyboard to the text control", () => {
    const enter = vi.fn(); const escape = vi.fn();
    render(<TextHarness enter={enter} escape={escape}/>);
    for (const key of ["T", "e", "s", "t", "@", "1", ".", "_", "-"]) fireEvent.keyDown(window, { key });
    expect(screen.getByLabelText("value")).toHaveTextContent("Test@1._-");
    fireEvent.keyDown(window, { key: "Backspace" });
    fireEvent.keyDown(window, { key: "Enter" });
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.getByLabelText("value")).toHaveTextContent("Test@1._");
    expect(enter).toHaveBeenCalledOnce(); expect(escape).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: "Enter · Confirm" })).toBeVisible();
  });

  it("accepts digits and one decimal point in the number control", () => {
    const enter = vi.fn(); const escape = vi.fn();
    render(<NumberHarness enter={enter} escape={escape}/>);
    for (const key of ["1", "2", ".", "5", ".", "x"]) fireEvent.keyDown(window, { key });
    expect(screen.getByLabelText("value")).toHaveTextContent("12.5");
    fireEvent.keyDown(window, { key: "Enter" }); fireEvent.keyDown(window, { key: "Escape" });
    expect(enter).toHaveBeenCalledOnce(); expect(escape).toHaveBeenCalledOnce();
  });

  it("uses the operator num-block layout and replaces an existing value on first entry", () => {
    render(<NumberHarness enter={vi.fn()} escape={vi.fn()} initial="62.8" replaceOnFirstInput/>);
    const keypad = screen.getByLabelText("Number input keypad");
    expect([...keypad.children].map((key) => key.textContent)).toEqual([
      "−", "7", "8", "9", "+",
      "ESC", "4", "5", "6", "THRU",
      "DIV", "1", "2", "3", "ENTER",
      "←", "0", ".", "AT",
    ]);
    expect(screen.getByRole("button", { name: "ENTER" })).toHaveStyle({ gridRow: "3 / span 2" });
    fireEvent.keyDown(window, { key: "9" });
    fireEvent.keyDown(window, { key: "5" });
    expect(screen.getByLabelText("value")).toHaveTextContent("95");
  });

  it("keeps the dot in its fixed num-block position when decimals are not accepted", () => {
    render(<NumberHarness enter={vi.fn()} escape={vi.fn()} allowDecimal={false}/>);
    fireEvent.click(screen.getByRole("button", { name: "." }));
    expect(screen.getByLabelText("value")).toBeEmptyDOMElement();
  });

  it("closes the number input from its ESC button", () => {
    const escape = vi.fn();
    render(<NumberHarness enter={vi.fn()} escape={escape}/>);
    fireEvent.click(screen.getByRole("button", { name: "ESC" }));
    expect(escape).toHaveBeenCalledOnce();
  });
});
