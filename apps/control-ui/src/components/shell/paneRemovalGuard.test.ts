import { describe, expect, it, vi } from "vitest";
import { registerPaneRemovalGuard, requestPaneRemoval } from "./paneRemovalGuard";

describe("pane removal guards", () => {
  it("allows an unguarded or clean pane to be removed without prompting", () => {
    const confirm = vi.fn(() => false);
    const unregister = registerPaneRemovalGuard("editor", () => null);

    expect(requestPaneRemoval("another-pane", confirm)).toBe(true);
    expect(requestPaneRemoval("editor", confirm)).toBe(true);
    expect(confirm).not.toHaveBeenCalled();
    unregister();
  });

  it("requires explicit confirmation before transient pane state is discarded", () => {
    const unregister = registerPaneRemovalGuard("editor", () => "Text Editor has unsaved changes.");
    const reject = vi.fn(() => false);
    const accept = vi.fn(() => true);

    expect(requestPaneRemoval("editor", reject)).toBe(false);
    expect(requestPaneRemoval("editor", accept)).toBe(true);
    expect(reject).toHaveBeenCalledWith(
      "Text Editor has unsaved changes.\n\nRemove this pane and discard those changes?",
    );
    unregister();
    expect(requestPaneRemoval("editor", reject)).toBe(true);
  });
});
