import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { StartupRecovery } from "./StartupRecovery";

const mocks = vi.hoisted(() => ({ snapshot: vi.fn(), open: vi.fn(), create: vi.fn() }));
vi.mock("./document/session", () => ({ documentSession: {startupRecovery: mocks.snapshot} }));
vi.mock("./FileBar", () => ({ chooseShowToOpen: mocks.open, chooseNewShow: mocks.create }));
const refused = {path: "/shows/old.show", reason: "unsupported programming contract", fallback: null, fallbackError: "packaged Demo Show is missing"};
beforeEach(() => {
 vi.clearAllMocks();
 mocks.snapshot.mockResolvedValue(refused);
 mocks.open.mockResolvedValue(null);
 mocks.create.mockResolvedValue(null);
});
it("keeps cancellation and rejected Open visible, then accepts a separate New Show", async () => {
 const accepted = vi.fn();
 render(<StartupRecovery onDocument={accepted} onError={vi.fn()} />);
 const notice = await screen.findByRole("alert");
 expect(notice.textContent).toContain("No fallback show is open.");
 expect(notice.textContent).toContain("packaged Demo Show is missing");
 fireEvent.click(screen.getByRole("button", {name: "Open Show"}));
 await waitFor(() => expect(mocks.open).toHaveBeenCalledOnce());
 expect(accepted).not.toHaveBeenCalled();
 mocks.open.mockRejectedValueOnce(new Error("chosen show is incompatible"));
 await waitFor(() => expect(screen.getByRole("button", {name: "Open Show"})).not.toBeDisabled());
 fireEvent.click(screen.getByRole("button", {name: "Open Show"}));
 await screen.findByText(/chosen show is incompatible/);
 expect(screen.getByRole("alert")).toBeTruthy();
 const newShow = {name: "Separate", path: "/shows/separate.show"};
 mocks.create.mockResolvedValueOnce(newShow);
 fireEvent.click(screen.getByRole("button", {name: "New Show"}));
 await waitFor(() => expect(accepted).toHaveBeenCalledWith(newShow));
 expect(screen.queryByRole("alert")).toBeNull();
});
it("uses a persistent snapshot for a late mount without needing an earlier event", async () => {
 const mounted = render(<StartupRecovery onDocument={vi.fn()} onError={vi.fn()} />);
 await screen.findByRole("alert");
 mounted.unmount();
 render(<StartupRecovery onDocument={vi.fn()} onError={vi.fn()} />);
 expect((await screen.findByRole("alert")).textContent).toContain("/shows/old.show");
 expect(mocks.snapshot).toHaveBeenCalledTimes(2);
});
