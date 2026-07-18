import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ShowRecoveryModal } from "./ShowRecoveryModal";

const mocks = vi.hoisted(() => ({
  bootstrap: {
    active_show: { id: "damaged", name: "Damaged Show" },
    active_show_error: "invalid playback reference",
  } as any,
  session: { user: { id: "operator", name: "Operator" } } as any,
  shows: [
    { id: "damaged", name: "Damaged Show" },
    { id: "valid", name: "Known Good" },
  ] as any[],
  error: null as string | null,
  initializeEmptyShow: vi.fn(),
  openCleanDefaultShow: vi.fn(),
  openShow: vi.fn(),
}));

vi.mock("../../api/ServerContext", () => ({ useServer: () => mocks }));

describe("ShowRecoveryModal", () => {
  afterEach(cleanup);

  beforeEach(() => {
    mocks.bootstrap.active_show_error = "invalid playback reference";
    mocks.initializeEmptyShow.mockReset().mockResolvedValue(true);
    mocks.openCleanDefaultShow.mockReset().mockResolvedValue(true);
    mocks.openShow.mockReset().mockResolvedValue(undefined);
  });

  it("loads another known-good show through safe blackout without selecting the damaged entry", async () => {
    render(<ShowRecoveryModal />);
    const dialog = screen.getByRole("alertdialog", { name: "Show recovery required" });
    expect(dialog).toHaveTextContent("It has not been changed or deleted");
    expect(screen.queryByRole("button", { name: "Load Latest Autosave for Damaged Show" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Load Latest Autosave for Known Good" }));
    await waitFor(() => expect(mocks.openShow).toHaveBeenCalledWith("valid", "safe_blackout"));
    expect(mocks.initializeEmptyShow).not.toHaveBeenCalled();
  });

  it("can recover from a damaged active show with an untouched built-in default copy", async () => {
    render(<ShowRecoveryModal />);
    fireEvent.click(screen.getByRole("button", { name: "Load Clean Built-in Default" }));
    await waitFor(() => expect(mocks.openCleanDefaultShow).toHaveBeenCalledOnce());
    expect(mocks.openShow).not.toHaveBeenCalled();
    expect(mocks.initializeEmptyShow).not.toHaveBeenCalled();
  });

  it("is absent after recovery clears the active-show error", () => {
    mocks.bootstrap.active_show_error = null;
    render(<ShowRecoveryModal />);
    expect(screen.queryByRole("alertdialog", { name: "Show recovery required" })).not.toBeInTheDocument();
  });
});
