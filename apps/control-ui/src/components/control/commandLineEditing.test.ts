import { describe, expect, it } from "vitest";
import { removeCommandToken } from "./commandLineEditing";

describe("removeCommandToken", () => {
  it("removes numeric characters individually and command words as whole tokens", () => {
    let value = "GROUP 1 THRU 6 AT 88";
    const expected = ["GROUP 1 THRU 6 AT 8", "GROUP 1 THRU 6 AT", "GROUP 1 THRU 6", "GROUP 1 THRU", "GROUP 1", "GROUP", ""];
    for (const next of expected) {
      value = removeCommandToken(value);
      expect(value).toBe(next);
    }
  });

  it("treats decimal and minus punctuation like numeric characters", () => {
    expect(removeCommandToken("FIXTURE 1 AT -8.5")).toBe("FIXTURE 1 AT -8.");
    expect(removeCommandToken("FIXTURE 1 AT -8.")).toBe("FIXTURE 1 AT -8");
  });
});
