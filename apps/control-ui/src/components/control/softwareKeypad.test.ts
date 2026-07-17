import { describe, expect, it } from "vitest";
import { numericPadLayout, oscProgrammerActionForKey } from "../../../../shared/programmerKeypad";
import { commandTargetAfterEnter, defaultCommandLine, editCommandWithSoftwareKey, editTargetedCommandWithSoftwareKey, softwareKeyFromKeyboard, softwareKeypadRows } from "./softwareKeypad";

describe("software keypad", () => {
  it("shares the current physical number-block layout and OSC actions with hardware surfaces", () => {
    const matrix = Array.from({ length: 5 }, () => Array<string>(6).fill(""));
    for (const item of numericPadLayout) {
      const column = item.section === "commands" ? item.column : item.column - 1;
      matrix[item.row - 1][column - 1] = item.key;
    }
    expect(matrix).toEqual([
      ["", "", "GRP", "CUE", "TIME", "DIV"],
      ["DEL", "CLR", "7", "8", "9", "-"],
      ["MOV", "BACKSPACE", "4", "5", "6", "+"],
      ["CPY", "UND", "1", "2", "3", "TRU"],
      ["SET", "SHIFT", ".", "0", "AT", "ENT"],
    ]);
    expect(oscProgrammerActionForKey("7")).toBe("digit-7");
    expect(oscProgrammerActionForKey("TRU")).toBe("thru");
    expect(oscProgrammerActionForKey("ENT")).toBe("enter");
    expect(oscProgrammerActionForKey("BACKSPACE")).toBe("backspace");
  });

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

  it("displays Speed Group decimal BPM values with the documented comma", () => {
    const decimal = editTargetedCommandWithSoftwareKey("SPD GRP 2 AT 127", ".", "FIXTURE", false);
    expect(decimal).toEqual({ command: "SPD GRP 2 AT 127,", execute: false, pristine: false });
    expect(editTargetedCommandWithSoftwareKey(decimal.command, "5", "FIXTURE", decimal.pristine)).toEqual({
      command: "SPD GRP 2 AT 127,5", execute: false, pristine: false,
    });
  });

  it("builds selected-playback Go To and Load Cue commands", () => {
    let goTo = editTargetedCommandWithSoftwareKey("FIXTURE", "CUE", "FIXTURE", true);
    goTo = editTargetedCommandWithSoftwareKey(goTo.command, "8", "FIXTURE", goTo.pristine);
    expect(goTo.command).toBe("CUE 8");

    let load = editTargetedCommandWithSoftwareKey("FIXTURE", "CUE", "FIXTURE", true);
    load = editTargetedCommandWithSoftwareKey(load.command, "CUE", "FIXTURE", load.pristine);
    load = editTargetedCommandWithSoftwareKey(load.command, "8", "FIXTURE", load.pristine);
    expect(load.command).toBe("CUE CUE 8");
  });

  it("enters SELECT from its software shortcut without selecting a playback", () => {
    expect(editTargetedCommandWithSoftwareKey("FIXTURE", "SELECT", "FIXTURE", true)).toEqual({
      command: "SELECT", execute: false, pristine: false,
    });
  });

  it("starts with the persistent fixture prefix and prefixes a fixture after a Group term", () => {
    expect(editTargetedCommandWithSoftwareKey("FIXTURE", "7", "FIXTURE", true)).toEqual({
      command: "F7", execute: false, pristine: false,
    });
    expect(editTargetedCommandWithSoftwareKey("G7 + ", "8", "FIXTURE", false)).toEqual({
      command: "G7 + F8", execute: false, pristine: false,
    });
  });

  it("prefixes a leading Plus continuation with the persistent target", () => {
    expect(editTargetedCommandWithSoftwareKey("+", "4", "FIXTURE", false)).toEqual({
      command: "+F4", execute: false, pristine: false,
    });
    expect(editTargetedCommandWithSoftwareKey("+", "4", "GROUP", false)).toEqual({
      command: "+G4", execute: false, pristine: false,
    });
  });

  it("uses short Group terms and lets GRP override Fixture mode after plus", () => {
    const group = editTargetedCommandWithSoftwareKey("G7 + ", "GRP", "FIXTURE", false);
    expect(group).toEqual({ command: "G7 + G", execute: false, pristine: false });
    expect(editTargetedCommandWithSoftwareKey(group.command, "8", "FIXTURE", group.pristine)).toEqual({
      command: "G7 + G8", execute: false, pristine: false,
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

  it("uses GRP as the opposite one-term prefix in Group mode and preserves Fixture-mode dereference", () => {
    const fixture = editTargetedCommandWithSoftwareKey("GROUP", "GRP", "GROUP", true);
    expect(fixture).toEqual({ command: "FIXTURE", execute: false, pristine: false });
    expect(editTargetedCommandWithSoftwareKey(fixture.command, "1", "GROUP", fixture.pristine)).toEqual({
      command: "F1", execute: false, pristine: false,
    });
    const entered = editTargetedCommandWithSoftwareKey("FIXTURE", "GRP", "FIXTURE", true);
    expect(entered).toEqual({ command: "GROUP", execute: false, pristine: false });
    expect(editTargetedCommandWithSoftwareKey(entered.command, "GRP", "FIXTURE", entered.pristine)).toEqual({
      command: "DEGRP", execute: false, pristine: false,
    });
  });

  it("toggles the persistent target in both directions on bare opposite-prefix Enter", () => {
    const group = commandTargetAfterEnter("GROUP ", "FIXTURE", false);
    expect(group).toBe("GROUP");
    expect(defaultCommandLine(group!)).toBe("GROUP");
    const fixture = commandTargetAfterEnter("FIXTURE", "GROUP", false);
    expect(fixture).toBe("FIXTURE");
    expect(defaultCommandLine(fixture!)).toBe("FIXTURE");
  });
});
