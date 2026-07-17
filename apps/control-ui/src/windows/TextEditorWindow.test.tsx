import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FileEntry, TextDocument } from "../api/types";
import { requestPaneRemoval } from "../components/shell/paneRemovalGuard";
import { listTextEditorFiles, TextEditorWindow } from "./TextEditorWindow";
import { textFileLocationChange } from "./textFileSync";

const mocks = vi.hoisted(() => ({
  dispatch: vi.fn(),
  openPicker: vi.fn(),
  state: {
    desks: [
      {
        id: "desk",
        panes: [
          {
            id: "editor",
            textFileRoot: "shows",
            textFilePath: "notes.txt",
            textEditorReadOnly: false,
            textEditorMode: "plain" as "plain" | "markdown" | "split",
          },
        ],
      },
    ],
  },
  server: {
    fileRoots: vi.fn(),
    fileEntries: vi.fn(),
    readTextFile: vi.fn(),
    saveTextFile: vi.fn(),
  },
}));

vi.mock("../api/ServerContext", () => ({
  useServer: () => mocks.server,
}));

vi.mock("../state/AppContext", () => ({
  useApp: () => ({ state: mocks.state, dispatch: mocks.dispatch }),
}));

vi.mock("./FileManagerPickerHost", () => ({ openFileManagerPicker: mocks.openPicker }));

const entry = (path: string, kind: "file" | "folder" = "file", writable = true): FileEntry => ({
  name: path.split("/").pop()!,
  path,
  kind,
  size: kind === "file" ? 12 : 0,
  modified_millis: 1,
  created_millis: 1,
  hidden: false,
  writable,
});

const document = (text = "House open", revision = "rev-1", readOnly = false, path = "notes.txt"): TextDocument => ({
  root_id: "shows",
  path,
  text,
  revision,
  read_only: readOnly,
});

describe("TextEditorWindow", () => {
  beforeEach(() => {
    mocks.dispatch.mockReset();
    mocks.state.desks[0].panes[0].textFileRoot = "shows";
    mocks.state.desks[0].panes[0].textFilePath = "notes.txt";
    mocks.state.desks[0].panes[0].textEditorReadOnly = false;
    mocks.state.desks[0].panes[0].textEditorMode = "plain";
    mocks.openPicker.mockReset().mockResolvedValue(null);
    mocks.server.fileRoots.mockReset().mockResolvedValue([
      { id: "shows", label: "Shows", icon: "shows", removable: false, writable: true },
    ]);
    mocks.server.fileEntries.mockReset().mockResolvedValue({ root_id: "shows", path: "", entries: [entry("notes.txt")] });
    mocks.server.readTextFile.mockReset().mockResolvedValue(document());
    mocks.server.saveTextFile.mockReset().mockImplementation(async (_root, path, text) => document(text, "rev-2", false, path));
    vi.spyOn(window, "confirm").mockReturnValue(true);
    vi.spyOn(window, "prompt").mockReturnValue(null);
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it("discovers supported text files in nested folders without offering arbitrary binary files", async () => {
    const list = vi.fn(async (_root: string, path = "") => path === ""
      ? { entries: [entry("run", "folder"), entry("root.md"), entry("image.png")] }
      : { entries: [entry("run/cues.txt"), entry("run/audio.wav"), entry("run/archive", "folder")] });
    list.mockImplementationOnce(async () => ({ entries: [entry("run", "folder"), entry("root.md"), entry("image.png")] }));
    list.mockImplementationOnce(async () => ({ entries: [entry("run/cues.txt"), entry("run/audio.wav")] }));

    const result = await listTextEditorFiles(list, "shows");

    expect(result.truncated).toBe(false);
    expect(result.files.map((file) => file.path)).toEqual(["root.md", "run/cues.txt"]);
  });

  it("maps a parent-folder move to the open document's descendant path", () => {
    expect(textFileLocationChange("shows", "run/act-1/cues.md", {
      operation: "move",
      items: [{
        source_root_id: "shows",
        source: "run",
        destination_root_id: "archive",
        destination: "past-shows/run",
        status: "completed",
        error: null,
      }],
    })).toEqual({ kind: "moved", rootId: "archive", path: "past-shows/run/act-1/cues.md" });
  });

  it("loads an associated read-only file and makes the state explicit", async () => {
    mocks.server.readTextFile.mockResolvedValue(document("Do not alter", "read-only", true));

    render(<TextEditorWindow paneId="editor" />);

    const editor = await screen.findByLabelText("File text");
    await waitFor(() => expect(editor).toHaveValue("Do not alter"));
    expect(editor).toHaveAttribute("readonly");
    expect(screen.getByText("Read-only", { selector: ".text-save-state" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Save$/ })).toBeDisabled();
    expect(screen.getByText(/contents can be copied with Save As/i)).toBeInTheDocument();
  });

  it("opens files through the root-confined ToskLight picker", async () => {
    mocks.openPicker.mockResolvedValue([{ rootId: "shows", entry: entry("run/cues.md") }]);
    render(<TextEditorWindow paneId="editor" />);
    await waitFor(() => expect(screen.getByLabelText("File text")).toHaveValue("House open"));

    fireEvent.click(screen.getByRole("button", { name: "Open File" }));

    await waitFor(() => expect(mocks.openPicker).toHaveBeenCalledWith({
      target: "files",
      multiple: false,
      allowedExtensions: ["txt", "md", "csv", "log"],
      initialRootId: "shows",
      initialDirectory: "",
    }));
    expect(mocks.dispatch).toHaveBeenCalledWith({
      type: "SET_TEXT_EDITOR_FILE",
      id: "editor",
      root: "shows",
      path: "run/cues.md",
    });
  });

  it("enforces pane-level read-only operation even for a writable file", async () => {
    mocks.state.desks[0].panes[0].textEditorReadOnly = true;
    render(<TextEditorWindow paneId="editor" />);

    const editor = await screen.findByLabelText("File text");
    await waitFor(() => expect(editor).toHaveValue("House open"));
    expect(editor).toHaveAttribute("readonly");
    expect(screen.getByText("Read-only", { selector: ".text-save-state" })).toBeVisible();
    expect(screen.getByText(/pane is configured read-only/i)).toBeVisible();
    expect(screen.getByRole("button", { name: /^Save$/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Save As" })).toBeDisabled();
  });

  it("renders Markdown-only and two-column Edit + Markdown views from the same authoritative text", async () => {
    mocks.state.desks[0].panes[0].textEditorMode = "markdown";
    mocks.server.readTextFile.mockResolvedValue(document("# Running Order\n\n- House open"));
    const view = render(<TextEditorWindow paneId="editor" />);

    const rendered = await screen.findByRole("article", { name: "Rendered Markdown" });
    expect(within(rendered).getByRole("heading", { name: "Running Order" })).toBeVisible();
    expect(screen.queryByLabelText("File text")).not.toBeInTheDocument();

    mocks.state.desks[0].panes[0].textEditorMode = "split";
    view.rerender(<TextEditorWindow paneId="editor" />);
    expect(screen.getByLabelText("File text")).toHaveValue("# Running Order\n\n- House open");
    expect(screen.getByRole("article", { name: "Rendered Markdown" })).toBeVisible();
  });

  it("reflects a clean save from another Text Editor window", async () => {
    render(<TextEditorWindow paneId="editor" />);
    const editor = screen.getByLabelText("File text");
    await waitFor(() => expect(editor).toHaveValue("House open"));

    act(() => {
      window.dispatchEvent(new CustomEvent("light:text-file-saved", {
        detail: { document: document("Beginners", "rev-2"), sourcePaneId: "other-editor" },
      }));
    });

    await waitFor(() => expect(editor).toHaveValue("Beginners"));
    expect(screen.getByText("Saved", { selector: ".text-save-state" })).toBeInTheDocument();
    expect(screen.getByText(/Another Text Editor window saved a newer version/i)).toBeInTheDocument();
  });

  it("preserves dirty text and provides compare/reload actions for a concurrent save", async () => {
    render(<TextEditorWindow paneId="editor" />);
    const editor = screen.getByLabelText("File text");
    await waitFor(() => expect(editor).toHaveValue("House open"));
    fireEvent.change(editor, { target: { value: "My local edit" } });

    act(() => {
      window.dispatchEvent(new CustomEvent("light:text-file-saved", {
        detail: { document: document("External edit", "rev-2"), sourcePaneId: "other-editor" },
      }));
    });

    expect(screen.getByText("Conflict", { selector: ".text-save-state" })).toBeInTheDocument();
    expect(editor).toHaveValue("My local edit");
    expect(screen.getByLabelText("Your unsaved version")).toHaveValue("My local edit");
    expect(screen.getByLabelText("Newer file version")).toHaveValue("External edit");
    expect(screen.getByRole("button", { name: /^Save$/ })).toBeDisabled();

    fireEvent.click(screen.getByRole("button", { name: "Reload Newer Version" }));
    expect(window.confirm).toHaveBeenCalledWith("Discard your unsaved version and load the newer file?");
    await waitFor(() => expect(editor).toHaveValue("External edit"));
    expect(screen.getByText("Saved", { selector: ".text-save-state" })).toBeInTheDocument();
  });

  it("turns a stale write into a visible revision conflict instead of overwriting", async () => {
    mocks.server.readTextFile
      .mockResolvedValueOnce(document("Original", "rev-1"))
      .mockResolvedValue(document("External", "rev-2"));
    mocks.server.saveTextFile.mockRejectedValue(new Error('{"error":"file changed since it was opened"}'));
    render(<TextEditorWindow paneId="editor" />);
    const editor = screen.getByLabelText("File text");
    await waitFor(() => expect(editor).toHaveValue("Original"));
    fireEvent.change(editor, { target: { value: "Operator draft" } });

    fireEvent.click(screen.getByRole("button", { name: /^Save$/ }));

    await waitFor(() => expect(screen.getByText("Conflict", { selector: ".text-save-state" })).toBeInTheDocument());
    expect(editor).toHaveValue("Operator draft");
    expect(screen.getByLabelText("Newer file version")).toHaveValue("External");
    expect(mocks.server.saveTextFile).toHaveBeenCalledWith("shows", "notes.txt", "Operator draft", "rev-1");
  });

  it("saves a copy without overwriting an existing revision and persists the new association", async () => {
    vi.mocked(window.prompt).mockReturnValue("run/new-notes.md");
    render(<TextEditorWindow paneId="editor" />);
    const editor = screen.getByLabelText("File text");
    await waitFor(() => expect(editor).toHaveValue("House open"));
    fireEvent.change(editor, { target: { value: "House open\nBeginners" } });

    fireEvent.click(screen.getByRole("button", { name: "Save As" }));

    await waitFor(() => expect(mocks.server.saveTextFile).toHaveBeenCalledWith(
      "shows",
      "run/new-notes.md",
      "House open\nBeginners",
      null,
    ));
    expect(mocks.dispatch).toHaveBeenCalledWith({
      type: "SET_TEXT_EDITOR_FILE",
      id: "editor",
      root: "shows",
      path: "run/new-notes.md",
    });
  });

  it("detects deletion, retains the last loaded text, and can recreate the file", async () => {
    vi.useFakeTimers();
    mocks.server.readTextFile
      .mockResolvedValueOnce(document("Retain me", "rev-1"))
      .mockRejectedValue(new Error('{"error":"file not found"}'));
    render(<TextEditorWindow paneId="editor" />);
    await act(async () => Promise.resolve());
    expect(screen.getByLabelText("File text")).toHaveValue("Retain me");

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1_600);
    });

    expect(screen.getByText("Missing", { selector: ".text-save-state" })).toBeInTheDocument();
    expect(screen.getByLabelText("File text")).toHaveValue("Retain me");
    expect(screen.getByLabelText("File text")).toHaveAttribute("readonly");
    expect(screen.getByRole("button", { name: "Recreate File" })).toBeInTheDocument();

    mocks.server.saveTextFile.mockResolvedValue(document("Retain me", "rev-3"));
    fireEvent.click(screen.getByRole("button", { name: "Recreate File" }));
    await act(async () => Promise.resolve());
    expect(mocks.server.saveTextFile).toHaveBeenCalledWith("shows", "notes.txt", "Retain me", null);
  });

  it("follows an externally moved open file without losing an unsaved draft", async () => {
    render(<TextEditorWindow paneId="editor" />);
    const editor = screen.getByLabelText("File text");
    await waitFor(() => expect(editor).toHaveValue("House open"));
    fireEvent.change(editor, { target: { value: "Draft retained across move" } });

    act(() => {
      window.dispatchEvent(new CustomEvent("light:file-operation", {
        detail: {
          operation: "move",
          items: [{
            source_root_id: "shows",
            source: "notes.txt",
            destination_root_id: "archive",
            destination: "run/renamed-notes.txt",
            status: "completed",
            error: null,
          }],
        },
      }));
    });

    expect(editor).toHaveValue("Draft retained across move");
    expect(screen.getByText("Unsaved", { selector: ".text-save-state" })).toBeInTheDocument();
    expect(screen.getByTitle("run/renamed-notes.txt")).toBeInTheDocument();
    expect(screen.getByText(/moved from notes.txt to run\/renamed-notes.txt/i)).toBeInTheDocument();
    expect(mocks.dispatch).toHaveBeenCalledWith({
      type: "SET_TEXT_EDITOR_FILE",
      id: "editor",
      root: "archive",
      path: "run/renamed-notes.txt",
    });

    fireEvent.click(screen.getByRole("button", { name: /^Save$/ }));
    await waitFor(() => expect(mocks.server.saveTextFile).toHaveBeenCalledWith(
      "archive",
      "run/renamed-notes.txt",
      "Draft retained across move",
      "rev-1",
    ));
  });

  it("surfaces an externally deleted file immediately and retains its last text", async () => {
    render(<TextEditorWindow paneId="editor" />);
    const editor = screen.getByLabelText("File text");
    await waitFor(() => expect(editor).toHaveValue("House open"));

    act(() => {
      window.dispatchEvent(new CustomEvent("light:file-operation", {
        detail: {
          operation: "delete",
          items: [{
            source_root_id: "shows",
            source: "notes.txt",
            destination_root_id: null,
            destination: null,
            status: "completed",
            error: null,
          }],
        },
      }));
    });

    expect(screen.getByText("Missing", { selector: ".text-save-state" })).toBeInTheDocument();
    expect(editor).toHaveValue("House open");
    expect(editor).toHaveAttribute("readonly");
    expect(screen.getByText(/deleted or moved to Trash/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Recreate File" })).toBeInTheDocument();
  });

  it("recovers from missing when another editor recreates identical content", async () => {
    vi.useFakeTimers();
    mocks.server.readTextFile
      .mockResolvedValueOnce(document("Same content", "same-content-revision"))
      .mockRejectedValue(new Error('{"error":"file not found"}'));
    render(<TextEditorWindow paneId="editor" />);
    await act(async () => Promise.resolve());
    await act(async () => vi.advanceTimersByTimeAsync(1_600));
    expect(screen.getByText("Missing", { selector: ".text-save-state" })).toBeInTheDocument();

    act(() => {
      window.dispatchEvent(new CustomEvent("light:text-file-saved", {
        detail: { document: document("Same content", "same-content-revision"), sourcePaneId: "other-editor" },
      }));
    });

    expect(screen.getByText("Saved", { selector: ".text-save-state" })).toBeInTheDocument();
    expect(screen.getByLabelText("File text")).toHaveValue("Same content");
  });

  it("asks before closing an associated file with unsaved changes", async () => {
    render(<TextEditorWindow paneId="editor" />);
    const editor = screen.getByLabelText("File text");
    await waitFor(() => expect(editor).toHaveValue("House open"));
    fireEvent.change(editor, { target: { value: "Unstored" } });
    vi.mocked(window.confirm).mockReturnValueOnce(false);

    fireEvent.click(screen.getByRole("button", { name: "Close File" }));
    expect(mocks.dispatch).not.toHaveBeenCalled();

    vi.mocked(window.confirm).mockReturnValueOnce(true);
    fireEvent.click(screen.getByRole("button", { name: "Close File" }));
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_TEXT_EDITOR_FILE", id: "editor", root: "shows", path: "" });
  });

  it("protects dirty text when the configurable pane is removed", async () => {
    render(<TextEditorWindow paneId="editor" />);
    const editor = screen.getByLabelText("File text");
    await waitFor(() => expect(editor).toHaveValue("House open"));
    fireEvent.change(editor, { target: { value: "Unstored pane text" } });
    vi.mocked(window.confirm).mockReturnValueOnce(false);

    expect(requestPaneRemoval("editor")).toBe(false);
    expect(window.confirm).toHaveBeenCalledWith(
      "Text Editor has unsaved changes.\n\nRemove this pane and discard those changes?",
    );
  });

  it("persists cursor and scroll state in the pane layout without persisting the draft", async () => {
    render(<TextEditorWindow paneId="editor" />);
    const editor = screen.getByLabelText("File text") as HTMLTextAreaElement;
    await waitFor(() => expect(editor).toHaveValue("House open"));
    editor.setSelectionRange(2, 7);
    editor.scrollTop = 120;
    fireEvent.blur(editor);

    expect(mocks.dispatch).toHaveBeenCalledWith({
      type: "SET_TEXT_EDITOR_VIEW",
      id: "editor",
      root: "shows",
      path: "notes.txt",
      selectionStart: 2,
      selectionEnd: 7,
      scrollTop: 120,
    });
    expect(mocks.dispatch.mock.calls.flat()).not.toContainEqual(expect.objectContaining({ text: expect.anything() }));
  });
});
