import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { initialState } from "../../state/appReducer";
import { LayoutPersistence } from "./LayoutPersistence";

const mocks = vi.hoisted(() => ({
  app: { state: null as unknown as typeof initialState, dispatch: vi.fn() },
  server: {
    bootstrap: { active_show: { id: "show" } },
    session: { user: { id: "user" } },
    deskLayout: null as null | { revision: number; body: { desks: typeof initialState.desks; activeDeskId: string } },
    deskLayoutScope: null as string | null,
    saveDeskLayout: vi.fn(),
  },
}));

vi.mock("../../api/ServerContext", async (importOriginal) => {
  const original = await importOriginal<typeof import("../../api/ServerContext")>();
  return { ...original, useServer: () => mocks.server };
});

vi.mock("../../state/AppContext", () => ({
  useApp: () => mocks.app,
}));

describe("LayoutPersistence", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    mocks.app.state = { ...initialState, desks: initialState.desks.map((desk) => ({ ...desk, panes: [...desk.panes] })) };
    mocks.app.dispatch.mockReset();
    mocks.server.deskLayout = null;
    mocks.server.deskLayoutScope = null;
    mocks.server.saveDeskLayout.mockReset().mockResolvedValue(undefined);
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("never saves the fallback layout before the server has resolved the current show and user layout", async () => {
    const view = render(<LayoutPersistence />);
    await act(async () => vi.advanceTimersByTimeAsync(700));
    expect(mocks.server.saveDeskLayout).not.toHaveBeenCalled();

    mocks.server.deskLayoutScope = "show:user";
    view.rerender(<LayoutPersistence />);
    await act(async () => vi.advanceTimersByTimeAsync(700));
    expect(mocks.server.saveDeskLayout).not.toHaveBeenCalled();

    mocks.app.state = { ...mocks.app.state, activeDeskId: "operator-change" };
    view.rerender(<LayoutPersistence />);
    await act(async () => vi.advanceTimersByTimeAsync(700));
    expect(mocks.server.saveDeskLayout).toHaveBeenCalledTimes(1);
    expect(mocks.server.saveDeskLayout).toHaveBeenCalledWith(expect.objectContaining({ activeDeskId: "operator-change" }));
  });

  it("hydrates a stored layout before allowing the hydrated state to be persisted", async () => {
    const storedDesks = [{ id: "tour", name: "Tour", panes: [] }];
    mocks.server.deskLayoutScope = "show:user";
    mocks.server.deskLayout = { revision: 7, body: { desks: storedDesks, activeDeskId: "tour" } };
    const view = render(<LayoutPersistence />);

    expect(mocks.app.dispatch).toHaveBeenCalledWith({
      type: "HYDRATE_LAYOUT",
      desks: storedDesks,
      activeDeskId: "tour",
      windowSettings: undefined,
    });
    await act(async () => vi.advanceTimersByTimeAsync(700));
    expect(mocks.server.saveDeskLayout).not.toHaveBeenCalled();

    mocks.app.state = { ...mocks.app.state, desks: storedDesks, activeDeskId: "tour" };
    view.rerender(<LayoutPersistence />);
    await act(async () => vi.advanceTimersByTimeAsync(700));
    expect(mocks.server.saveDeskLayout).toHaveBeenCalledTimes(1);
    expect(mocks.server.saveDeskLayout).toHaveBeenCalledWith(expect.objectContaining({ desks: storedDesks, activeDeskId: "tour" }));
  });
});
