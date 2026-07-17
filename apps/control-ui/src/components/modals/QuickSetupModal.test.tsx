import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QuickSetupModal } from "./QuickSetupModal";

const mocks = vi.hoisted(() => ({
  dispatch: vi.fn(),
  server: {
    status: "connected" as const,
    bootstrap: {
      active_show: {
        id: "copy",
        name: "Tour-rev-3-2026-07-17",
        revision: 1,
        updated_at: "2026-07-17T12:00:00Z",
        path: "copy.show",
        revision_copy: {
          show_id: "original",
          show_name: "Tour",
          revision: 3,
          revision_name: "Approved focus",
          copied_at: "2026-07-17T11:30:00Z",
        },
      },
      users: [{ id: "operator", name: "Operator", enabled: true }],
    } as any,
    session: { user: { id: "operator", name: "Operator" } },
    shows: [
      { id: "original", name: "Tour", revision: 4, updated_at: "", path: "tour.show" },
      { id: "copy", name: "Tour-rev-3-2026-07-17", revision: 1, updated_at: "", path: "copy.show" },
      { id: "other", name: "Festival", revision: 2, updated_at: "", path: "festival.show" },
    ] as any[],
    error: null as string | null,
    listShowRevisions: vi.fn(),
    openShowRevision: vi.fn(),
    overwriteShow: vi.fn(),
    saveShowRevision: vi.fn(),
    saveShowAs: vi.fn(),
    openShow: vi.fn(),
    uploadShow: vi.fn(),
    initializeEmptyShow: vi.fn(),
    saveScreen: vi.fn(),
    createUser: vi.fn(),
    changeUser: vi.fn(),
    shutdownServer: vi.fn(),
    previewMvr: vi.fn(),
    applyMvr: vi.fn(),
    previewMvrExport: vi.fn(),
    downloadMvr: vi.fn(),
    downloadShow: vi.fn(),
  },
}));

vi.mock("../../api/ServerContext", () => ({ useServer: () => mocks.server }));
vi.mock("../../state/AppContext", () => ({
  useApp: () => ({
    state: { setupOpen: true, desks: [], activeDeskId: "" },
    dispatch: mocks.dispatch,
  }),
}));

describe("QuickSetupModal named revision copies", () => {
  beforeEach(() => {
    mocks.server.bootstrap.active_show.revision_copy = {
      show_id: "original",
      show_name: "Tour",
      revision: 3,
      revision_name: "Approved focus",
      copied_at: "2026-07-17T11:30:00Z",
    };
    mocks.server.shows = [
      { id: "original", name: "Tour", revision: 4, updated_at: "", path: "tour.show" },
      { id: "copy", name: "Tour-rev-3-2026-07-17", revision: 1, updated_at: "", path: "copy.show" },
      { id: "other", name: "Festival", revision: 2, updated_at: "", path: "festival.show" },
    ];
    mocks.server.listShowRevisions.mockReset().mockImplementation(async (id: string) => id === "original" ? [{ show_id: id, revision: 3, name: "Approved focus", created_at: "2026-07-16T10:00:00Z" }] : []);
    mocks.server.openShowRevision.mockReset().mockResolvedValue(true);
    mocks.server.overwriteShow.mockReset().mockResolvedValue(true);
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("identifies the active copy and requires confirmation before overwriting the original", async () => {
    render(<QuickSetupModal />);
    const menu = screen.getByRole("dialog", { name: "Show" });
    expect(menu).toHaveTextContent("Separate revision copy");
    expect(menu).toHaveTextContent("Tour, Revision 3 · Approved focus");
    expect(menu).toHaveTextContent("Current changes are autosaved to this copy, not to Tour");

    fireEvent.click(within(menu).getByRole("button", { name: "Save" }));
    const save = screen.getByRole("dialog", { name: "Save revision copy" });
    expect(save).toHaveTextContent("Autosave already protects this copy");
    fireEvent.click(within(save).getByRole("button", { name: "Overwrite Original Show" }));

    const confirmation = screen.getByRole("alertdialog", { name: "Confirm overwrite Tour" });
    expect(confirmation).toHaveTextContent("identity and named revisions are preserved");
    expect(mocks.server.overwriteShow).not.toHaveBeenCalled();
    fireEvent.click(within(confirmation).getByRole("button", { name: "Replace Tour Latest Autosave" }));
    await waitFor(() => expect(mocks.server.overwriteShow).toHaveBeenCalledWith("original"));
  });

  it("offers every existing Save As destination but keeps cancel as the safe default", () => {
    render(<QuickSetupModal />);
    fireEvent.click(screen.getByRole("button", { name: "Save As" }));
    const saveAs = screen.getByRole("dialog", { name: "Save show" });
    expect(saveAs).toHaveTextContent("Original show");
    const festival = within(saveAs).getByText("Festival").closest("article")!;
    fireEvent.click(within(festival).getByRole("button", { name: "Choose Destination" }));
    const confirmation = screen.getByRole("alertdialog", { name: "Confirm overwrite Festival" });
    fireEvent.click(within(confirmation).getByRole("button", { name: "Cancel" }));
    expect(mocks.server.overwriteShow).not.toHaveBeenCalled();
  });

  it("loads a named revision through the explicit copy action", async () => {
    render(<QuickSetupModal />);
    fireEvent.click(screen.getByRole("button", { name: "Load" }));
    const load = await screen.findByRole("dialog", { name: "Load show" });
    const original = within(load).getByText("Tour").closest("article")!;
    const action = await within(original).findByText("Load Revision as Copy");
    fireEvent.click(action.closest("button")!);
    await waitFor(() => expect(mocks.server.openShowRevision).toHaveBeenCalledWith("original", 3));
  });

  it("keeps an orphaned revision copy usable without an overwrite-original action", () => {
    mocks.server.shows = mocks.server.shows.filter((show) => show.id !== "original");
    render(<QuickSetupModal />);
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    const save = screen.getByRole("dialog", { name: "Save revision copy" });
    expect(save).toHaveTextContent("original show is no longer available");
    expect(within(save).queryByRole("button", { name: "Overwrite Original Show" })).not.toBeInTheDocument();
  });
});
