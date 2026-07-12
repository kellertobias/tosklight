import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { fallbackKeyboardLayout, ModalNumberInput, ModalTextKeyboard } from "./ModalInputControls";

afterEach(cleanup);

function TextHarness({ enter, escape }: { enter: () => void; escape: () => void }) {
  const [value, setValue] = useState("");
  return <div className="modal-backdrop"><output aria-label="value">{value}</output><ModalTextKeyboard value={value} onChange={setValue} onEnter={enter} onEscape={escape}/></div>;
}

function NumberHarness({ enter, escape }: { enter: () => void; escape: () => void }) {
  const [value, setValue] = useState("");
  return <div className="modal-backdrop"><output aria-label="value">{value}</output><ModalNumberInput value={value} onChange={setValue} onEnter={enter} onEscape={escape}/></div>;
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
});
