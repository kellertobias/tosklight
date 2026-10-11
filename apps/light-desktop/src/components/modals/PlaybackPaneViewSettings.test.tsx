import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PlaybackPaneViewSettings } from "./PlaybackPaneViewSettings";
import type { PaneModel } from "../../types";
const model = vi.hoisted(() => ({ update: vi.fn(), dispatch: vi.fn(), source: { type: "follow", surface_id: "main" } as unknown }));
vi.mock("../../features/screens/ScreensContext", () => ({ useScreens: () => ({
 surfaces: { playback_surfaces: { main: { name: "Main" }, "screen:second": { name: "Screen 1" }, "pane:playbacks": { page_source: model.source } }, resolved_pages: { "pane:playbacks": 4 } },
 screens: { screens: [{ name: "Screen 1", surface_id: "screen:second" }] }, updatePlaybackSurface: model.update,
}) }));
vi.mock("../../state/AppContext", () => ({ useApp: () => ({ dispatch: model.dispatch }) }));
const pane = { id: "playbacks", title: "Playbacks", surface_id: "pane:playbacks", playbackViewMode: "screen", playbackScreenSurfaceId: "main" } as PaneModel;
afterEach(cleanup);
beforeEach(() => { model.update.mockReset().mockResolvedValue(undefined); model.dispatch.mockReset(); model.source = { type: "follow", surface_id: "main" }; });
describe("Playback pane live screen and independent page", () => {
 it("switches the live screen reference without copying or changing either screen layout", async () => {
  render(<PlaybackPaneViewSettings pane={pane} />);
  fireEvent.click(screen.getByRole("button", { name: "Screen" }));
  fireEvent.click(screen.getByRole("option", { name: "Screen 1" }));
  await waitFor(() => expect(model.dispatch).toHaveBeenCalledWith({ type: "SET_PANE_PLAYBACK_VIEW", id: "playbacks", mode: "screen", screenSurfaceId: "screen:second" }));
  expect(model.update).toHaveBeenCalledExactlyOnceWith("pane:playbacks", { page_source: { type: "follow", surface_id: "screen:second" } });
 });
 it("retains a custom pane page when its displayed screen changes", async () => {
  model.source = { type: "own", page: 7 };
  render(<PlaybackPaneViewSettings pane={pane} />);
  fireEvent.click(screen.getByRole("button", { name: "Screen" }));
  fireEvent.click(screen.getByRole("option", { name: "Screen 1" }));
  await waitFor(() => expect(model.update).toHaveBeenCalledExactlyOnceWith("pane:playbacks", { page_source: { type: "own", page: 7 } }));
 });
 it("chooses a custom page using the currently resolved pane page", async () => {
  render(<PlaybackPaneViewSettings pane={pane} />);
  fireEvent.click(screen.getByRole("button", { name: "Page source" }));
  fireEvent.click(screen.getByRole("option", { name: "Custom page" }));
  await waitFor(() => expect(model.update).toHaveBeenCalledExactlyOnceWith("pane:playbacks", { page_source: { type: "own", page: 4 } }));
 });
 it("leaves saved pane references unchanged after authoritative rejection", async () => {
  model.update.mockRejectedValue(new Error("Page follow cycle"));
  render(<PlaybackPaneViewSettings pane={pane} />);
  fireEvent.click(screen.getByRole("button", { name: "Page source" }));
  fireEvent.click(screen.getByRole("option", { name: "Custom page" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Page follow cycle");
  expect(model.dispatch).not.toHaveBeenCalled();
 });
});
