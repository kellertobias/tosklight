import { describe, expect, it } from "vitest";
import { commandTargetAfterEnter, defaultCommandLine, editCommandWithSoftwareKey, editTargetedCommandWithSoftwareKey, softwareKeyFromKeyboard, softwareKeypadRows } from "./softwareKeypad";

describe("software keypad", () => {
  it("keeps the documented keypad layout with timing, minus, and shift", () => {
    expect(softwareKeypadRows).toEqual([
      ["SET", "GRP", "CUE", "UND", "CLR"],
      ["DEL", "7", "8", "9", "+"],
      ["MOV", "4", "5", "6", "TRU"],
      ["CPY", "1", "2", "3", "DIV"],
      ["BACKSPACE", "0", ".", "AT", "ENT"],
      ["SHIFT", "TIME", "-"],
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
    expect(key("KeyY", true, "Z")).toBe("SELECT");
    expect(key("KeyA", false, "a")).toBeNull();
    expect(key("NumpadSubtract", false, "-")).toBe("-");
  });

  it("allows regular digits to be disabled while keeping numpad digits", () => {
    expect(softwareKeyFromKeyboard({ code: "Digit7", shiftKey: false, key: "7" }, false)).toBeNull();
    expect(softwareKeyFromKeyboard({ code: "Numpad7", shiftKey: false, key: "7" }, false)).toBe("7");
  });

  it("expands the double AT and double dot shortcuts and requests execution", () => {
    expect(editCommandWithSoftwareKey("1 AT ", "AT")).toEqual({ command: "1 AT FULL", execute: true });
    expect(editCommandWithSoftwareKey("1.", ".")).toEqual({ command: "1 AT 0", execute: true });
  });

  it("shows a double TIME press as DELAY", () => {
    expect(editCommandWithSoftwareKey("1 AT 100 TIME ", "TIME")).toEqual({ command: "1 AT 100 DELAY ", execute: false });
    expect(editCommandWithSoftwareKey("1 AT 100 ", "-")).toEqual({ command: "1 AT 100 - ", execute: false });
  });

  it("enters SELECT from its software shortcut without selecting a playback", () => {
    expect(editTargetedCommandWithSoftwareKey("FIXTURE", "SELECT", "FIXTURE", true)).toEqual({
      command: "SELECT", execute: false, pristine: false,
    });
  });

  it("starts with the persistent fixture prefix and continues the current Group scope", () => {
    expect(editTargetedCommandWithSoftwareKey("FIXTURE", "7", "FIXTURE", true)).toEqual({
      command: "F7", execute: false, pristine: false,
    });
    expect(editTargetedCommandWithSoftwareKey("G7 + ", "8", "FIXTURE", false)).toEqual({
      command: "G7 + G8", execute: false, pristine: false,
    });
  });

  it("uses short Group terms and lets GRP override the current scope after plus", () => {
    const group = editTargetedCommandWithSoftwareKey("G7 + ", "GRP", "FIXTURE", false);
    expect(group).toEqual({ command: "G7 + F", execute: false, pristine: false });
    expect(editTargetedCommandWithSoftwareKey(group.command, "8", "FIXTURE", group.pristine)).toEqual({
      command: "G7 + F8", execute: false, pristine: false,
    });
  });

  it("defaults to Groups in Group mode and makes plus-GRP a Fixture override", () => {
    expect(editTargetedCommandWithSoftwareKey("GROUP", "7", "GROUP", true)).toEqual({ command: "G7", execute: false, pristine: false });
    expect(editTargetedCommandWithSoftwareKey("G7 + ", "8", "GROUP", false)).toEqual({ command: "G7 + G8", execute: false, pristine: false });
    const fixture = editTargetedCommandWithSoftwareKey("G7 + ", "GRP", "GROUP", false);
    expect(fixture).toEqual({ command: "G7 + F", execute: false, pristine: false });
    expect(editTargetedCommandWithSoftwareKey(fixture.command, "8", "GROUP", fixture.pristine)).toEqual({ command: "G7 + F8", execute: false, pristine: false });
  });

  it("keeps GROUP as the storage target after RECORD plus", () => {
    expect(editTargetedCommandWithSoftwareKey("RECORD + ", "GRP", "GROUP", false)).toEqual({ command: "RECORD + GROUP ", execute: false, pristine: false });
  });

  it("marks the visible Group prefix as entered and preserves double-GRP dereference", () => {
    const entered = editTargetedCommandWithSoftwareKey("GROUP", "GRP", "GROUP", true);
    expect(entered).toEqual({ command: "GROUP", execute: false, pristine: false });
    expect(editTargetedCommandWithSoftwareKey(entered.command, "GRP", "GROUP", entered.pristine)).toEqual({
      command: "DEGRP", execute: false, pristine: false,
    });
  });

  it("switches to persistent Group mode on bare Group Enter and restores it after Clear or Escape", () => {
    const target = commandTargetAfterEnter("GROUP ", "FIXTURE", false);
    expect(target).toBe("GROUP");
    expect(defaultCommandLine(target!)).toBe("GROUP");
    expect(defaultCommandLine(target!)).toBe("GROUP");
  });
});
