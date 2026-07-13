import { describe, expect, it } from "vitest";
import { editCommandWithSoftwareKey, softwareKeyFromKeyboard, softwareKeypadRows } from "./softwareKeypad";

describe("software keypad", () => {
  it("keeps the documented five by five layout", () => {
    expect(softwareKeypadRows).toEqual([
      ["SET", "GRP", "CUE", "UND", "CLR"],
      ["DEL", "7", "8", "9", "+"],
      ["MOV", "4", "5", "6", "TRU"],
      ["CPY", "1", "2", "3", "DIV"],
      ["BACKSPACE", "0", ".", "AT", "ENT"],
    ]);
  });

  it("maps German physical key positions without consuming letter keys", () => {
    const key = (code: string, shiftKey = false, value = "") => softwareKeyFromKeyboard({ code, shiftKey, key: value }, true);
    expect(key("Minus")).toBe("TRU");
    expect(key("Minus", true, "?")).toBe("CUE");
    expect(key("Backquote")).toBe("PRE");
    expect(key("Backquote", true)).toBe("GRP");
    expect(key("Equal")).toBe("DIV");
    expect(key("Equal", true)).toBe("DEL");
    expect(key("Backslash")).toBe("AT");
    expect(key("Backslash", true)).toBe("MOV");
    expect(key("KeyA", false, "a")).toBeNull();
  });

  it("allows regular digits to be disabled while keeping numpad digits", () => {
    expect(softwareKeyFromKeyboard({ code: "Digit7", shiftKey: false, key: "7" }, false)).toBeNull();
    expect(softwareKeyFromKeyboard({ code: "Numpad7", shiftKey: false, key: "7" }, false)).toBe("7");
  });

  it("expands the double AT and double dot shortcuts and requests execution", () => {
    expect(editCommandWithSoftwareKey("1 AT ", "AT")).toEqual({ command: "1 AT FULL", execute: true });
    expect(editCommandWithSoftwareKey("1.", ".")).toEqual({ command: "1 AT 0", execute: true });
  });
});
