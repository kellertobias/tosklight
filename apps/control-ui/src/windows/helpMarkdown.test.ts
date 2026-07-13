import { describe, expect, it } from "vitest";
import { prepareHelpMarkdown, safeHelpUrl } from "./helpMarkdown";

describe("help Markdown extensions", () => {
  it("marks keys, held and optional keys, and placeholders", () => {
    const output = prepareHelpMarkdown("[AT][ENTER] [+] [CLR+] [GRP*] [0-9] <selection> <target+> `[1]` `[.]`");
    expect(output).toContain("`help-key:AT``help-key:ENTER`");
    expect(output).toContain("`help-key:CLR+`");
    expect(output).toContain("`help-key:GRP*`");
    expect(output).toContain("`help-key:+`");
    expect(output).toContain("`help-key:0-9`");
    expect(output).toContain("`help-placeholder:selection`");
    expect(output).toContain("`help-placeholder:target+`");
    expect(output).toContain("`help-key:1`");
    expect(output).toContain("`help-key:.`");
  });
  it("allows safe links and relative images only", () => {
    expect(safeHelpUrl("images/desk.png", "image")).toBe("/api/v1/help/assets/images/desk.png");
    expect(safeHelpUrl("../secret.png", "image")).toBeUndefined();
    expect(safeHelpUrl("javascript:alert(1)", "link")).toBeUndefined();
    expect(safeHelpUrl("https://example.com/help", "link")).toBe("https://example.com/help");
  });
});
