import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { DeskLockOverlay } from "./DeskLockOverlay";

const mocks = vi.hoisted(() => ({ unlockDesk: vi.fn(), deskLock: { locked: true, message: "Call the operator", wallpaper: null, unlock_mode: "pin" as const } }));
vi.mock("../../api/ServerContext", () => ({ useServer: () => mocks }));

describe("DeskLockOverlay", () => {
  it("covers the desk and retains the lock after an incorrect PIN", async () => {
    mocks.unlockDesk.mockResolvedValue(false);
    render(<DeskLockOverlay />);
    expect(screen.getByRole("dialog", { name: "Desk locked" })).toHaveTextContent("Call the operator");
    fireEvent.change(screen.getByLabelText("PIN"), { target: { value: "12ab34" } });
    fireEvent.click(screen.getByRole("button", { name: "Unlock Desk" }));
    await waitFor(() => expect(mocks.unlockDesk).toHaveBeenCalledWith("1234"));
    expect(screen.getByText("Incorrect PIN")).toBeInTheDocument();
  });
});
