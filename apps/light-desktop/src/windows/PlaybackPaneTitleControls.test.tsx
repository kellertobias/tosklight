import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PlaybackPaneTitleControls } from "./PlaybackPaneTitleControls";
const model = vi.hoisted(() => ({ source: { type: "own", page: 4 } as unknown, select: vi.fn(), setPage: vi.fn(), update: vi.fn(), editor: { available: true, ready: true, selecting: false, selected: [{page: 4, position: 1}, {page: 4, position: 2}], name: "Test", saving: false, error: null, begin: vi.fn(), cancel: vi.fn(), save: vi.fn(), setName: vi.fn() } }));
vi.mock("../components/shell/PaneChromeContext", () => ({ usePaneChromeTargets: () => null }));
vi.mock("../features/playbackSolo/PlaybackSoloEditorContext", () => ({ usePlaybackSoloEditor: () => model.editor }));
vi.mock("../features/playbackTopology/PlaybackTopologyView", () => ({ usePlaybackPagesView: () => ({ ready: true, pages: [{ id: "page7", body: {number: 7, name: "Act 2"} }] }) }));
vi.mock("../features/screens/ScreensContext", () => ({ useScreens: () => ({ surfaces: { playback_surfaces: { "pane:test": { page_source: model.source } }, resolved_pages: { "pane:test": 4 } }, selectSurface: model.select, setSurfacePage: model.setPage, updatePlaybackSurface: model.update }) }));
afterEach(cleanup);
beforeEach(() => { model.source = {type: "own", page: 4}; model.editor.selecting = false; model.select.mockReset().mockResolvedValue(undefined); model.setPage.mockReset().mockResolvedValue(undefined); model.update.mockReset().mockResolvedValue(undefined); model.editor.begin.mockReset(); model.editor.cancel.mockReset(); model.editor.save.mockReset(); });
describe("Playback pane title controls", () => {
 it("custom page caret changes only the pane page and selects that command surface", async () => {
  render(<PlaybackPaneTitleControls surfaceId="pane:test" />);
  fireEvent.click(screen.getByRole("button", { name: "Select Playback page 4" }));
  fireEvent.click(screen.getByRole("button", { name: "7 · Act 2" }));
  await waitFor(() => expect(model.setPage).toHaveBeenCalledExactlyOnceWith("pane:test", 7));
  expect(model.select).toHaveBeenCalledExactlyOnceWith("pane:test");
  expect(model.update).not.toHaveBeenCalled();
 });
 it("following a screen exposes page indication without manual page selection", () => {
  model.source = {type: "follow", surface_id: "main"};
  render(<PlaybackPaneTitleControls surfaceId="pane:test" />);
  expect(screen.getByText("Page 4")).toBeInTheDocument();
  expect(screen.queryByRole("button", {name: /Select Playback page/})).not.toBeInTheDocument();
 });
 it("creates locally and replaces its title action with Save and Cancel during a draft", async () => {
  const view = render(<PlaybackPaneTitleControls surfaceId="pane:test" />);
  fireEvent.click(screen.getByRole("button", {name: "Create Solo Region"}));
  await waitFor(() => expect(model.editor.begin).toHaveBeenCalledOnce());
  model.editor.selecting = true; view.rerender(<PlaybackPaneTitleControls surfaceId="pane:test" />);
  expect(screen.queryByRole("button", {name: "Create Solo Region"})).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", {name: "Cancel"}));
  expect(model.editor.cancel).toHaveBeenCalledOnce();
  expect(model.editor.save).not.toHaveBeenCalled();
 });
});
