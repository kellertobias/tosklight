import { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { useFiles } from "../features/files/FilesContext";
import type { FileEntry, FileRoot, TextDocument } from "../api/types";
import { Button, Select, TextArea } from "../components/common";
import { registerPaneRemovalGuard } from "../components/shell/paneRemovalGuard";
import { usePaneChromeTargets } from "../components/shell/PaneChromeContext";
import { useApp } from "../state/AppContext";
import { openFileManagerPicker } from "./FileManagerPickerHost";
import {
  publishTextFileSaved,
  TEXT_FILE_OPERATION_EVENT,
  TEXT_FILE_SAVED_EVENT,
  textDocumentFromSavedEvent,
  textFileLocationChange,
  textFileOperationFromEvent,
} from "./textFileSync";
import type { WindowProps } from "./windowTypes";

const TEXT_FILE_EXTENSIONS = new Set(["txt", "md", "csv", "log"]);
const EXTERNAL_CHECK_INTERVAL_MILLIS = 1_500;
const FILE_CHOOSER_DIRECTORY_LIMIT = 256;
const FILE_CHOOSER_FILE_LIMIT = 2_000;
const MAX_TEXT_FILE_BYTES = 4 * 1024 * 1024;

type Availability = "none" | "loading" | "ready" | "missing";
type Notice = { kind: "info" | "error" | "conflict"; text: string } | null;
interface LegacyTextEditorViewState {
  selectionStart: number;
  selectionEnd: number;
  scrollTop: number;
}

function extension(path: string) {
  return path.split(".").pop()?.toLowerCase() ?? "";
}

function parentDirectory(path: string) {
  return path.split("/").slice(0, -1).join("/");
}

function isSupportedTextFile(path: string) {
  return TEXT_FILE_EXTENSIONS.has(extension(path));
}

function isSameDocumentVersion(left: TextDocument | null, right: TextDocument) {
  return Boolean(
    left
      && left.root_id === right.root_id
      && left.path === right.path
      && left.revision === right.revision
      && left.text === right.text
      && left.read_only === right.read_only,
  );
}

function friendlyError(error: unknown) {
  const raw = error instanceof Error ? error.message : String(error);
  try {
    const parsed = JSON.parse(raw) as { error?: unknown };
    if (typeof parsed.error === "string") return parsed.error;
  } catch {
    // Non-JSON errors already carry the most useful message available.
  }
  return raw.replace(/^Error:\s*/, "");
}

function isMissingError(error: unknown) {
  const message = friendlyError(error).toLowerCase();
  return message.includes("not found") || message.includes("was removed") || message.includes("unavailable");
}

function viewStateKey(paneId: string | undefined, root: string, path: string) {
  return `light.text-editor-view.${paneId ?? "window"}.${root}.${path}`;
}

/**
 * Resolve the supported files below a configured root. The cap prevents a
 * removable drive from turning a simple chooser into an unbounded crawl.
 */
export async function listTextEditorFiles(
  fileEntries: (root: string, path?: string, hidden?: boolean) => Promise<{ entries: FileEntry[] }>,
  root: string,
) {
  const directories = [""];
  const files: FileEntry[] = [];
  let visitedDirectories = 0;
  while (directories.length && visitedDirectories < FILE_CHOOSER_DIRECTORY_LIMIT && files.length < FILE_CHOOSER_FILE_LIMIT) {
    const path = directories.shift()!;
    const listing = await fileEntries(root, path, false);
    visitedDirectories += 1;
    for (const entry of listing.entries) {
      if (entry.kind === "folder") directories.push(entry.path);
      else if (isSupportedTextFile(entry.path)) files.push(entry);
      if (files.length >= FILE_CHOOSER_FILE_LIMIT) break;
    }
  }
  return {
    files: files.sort((left, right) => left.path.localeCompare(right.path, undefined, { sensitivity: "base" })),
    truncated: directories.length > 0 || files.length >= FILE_CHOOSER_FILE_LIMIT,
  };
}

export function TextEditorWindow({ paneId }: WindowProps) {
  const server = useFiles();
  const serverRef = useRef(server);
  serverRef.current = server;
  const { state, dispatch } = useApp();
  const paneChrome = usePaneChromeTargets();
  const pane = state.desks.flatMap((desk) => desk.panes).find((candidate) => candidate.id === paneId);
  const selectedPath = pane?.textFilePath ?? "";
  const paneReadOnly = Boolean(pane?.textEditorReadOnly);
  const editorMode = pane?.textEditorMode ?? "plain";

  const [roots, setRoots] = useState<FileRoot[]>([]);
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [filesLoading, setFilesLoading] = useState(false);
  const [document, setDocument] = useState<TextDocument | null>(null);
  const [externalDocument, setExternalDocument] = useState<TextDocument | null>(null);
  const [text, setText] = useState("");
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [availability, setAvailability] = useState<Availability>("none");
  const [notice, setNotice] = useState<Notice>(null);
  const messageId = useId();
  const textarea = useRef<HTMLTextAreaElement>(null);
  const documentRef = useRef<TextDocument | null>(null);
  const externalDocumentRef = useRef<TextDocument | null>(null);
  const dirtyRef = useRef(false);
  const textRef = useRef("");
  const availabilityRef = useRef<Availability>("none");
  const fileListRequest = useRef(0);
  const relocatedAssociation = useRef<{ root: string; path: string } | null>(null);

  documentRef.current = document;
  externalDocumentRef.current = externalDocument;
  dirtyRef.current = dirty;
  textRef.current = text;
  availabilityRef.current = availability;

  const selectedRoot = pane?.textFileRoot ?? roots[0]?.id ?? "";

  useEffect(() => {
    let cancelled = false;
    void serverRef.current
      .fileRoots()
      .then((next) => {
        if (!cancelled) setRoots(next);
      })
      .catch((error) => {
        if (!cancelled) setNotice({ kind: "error", text: `Could not load file locations: ${friendlyError(error)}` });
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const reloadFiles = useCallback(async (root = selectedRoot) => {
    const request = ++fileListRequest.current;
    if (!root) {
      setFiles([]);
      return;
    }
    setFilesLoading(true);
    try {
      const result = await listTextEditorFiles(serverRef.current.fileEntries, root);
      if (request !== fileListRequest.current) return;
      setFiles(result.files);
      if (result.truncated) {
        setNotice({
          kind: "info",
          text: "The file chooser reached its safety limit. Use Save As with a root-relative path or narrow the configured location.",
        });
      }
    } catch (error) {
      if (request === fileListRequest.current) {
        setFiles([]);
        setNotice({ kind: "error", text: `Could not list text files: ${friendlyError(error)}` });
      }
    } finally {
      if (request === fileListRequest.current) setFilesLoading(false);
    }
  }, [selectedRoot]);

  useEffect(() => {
    void reloadFiles(selectedRoot);
  }, [reloadFiles, selectedRoot]);

  const acceptDocument = useCallback((next: TextDocument, message?: string) => {
    documentRef.current = next;
    externalDocumentRef.current = null;
    dirtyRef.current = false;
    textRef.current = next.text;
    setDocument(next);
    setExternalDocument(null);
    setText(next.text);
    setDirty(false);
    availabilityRef.current = "ready";
    setAvailability("ready");
    setNotice(
      next.read_only
        ? { kind: "info", text: "This file is read-only. Its contents can be copied with Save As, but the original cannot be changed." }
        : message
          ? { kind: "info", text: message }
          : null,
    );
  }, []);

  const surfaceExternalDocument = useCallback((next: TextDocument, source: string) => {
    const current = documentRef.current;
    if (dirtyRef.current && !isSameDocumentVersion(current, next) && textRef.current !== next.text) {
      externalDocumentRef.current = next;
      setExternalDocument(next);
      availabilityRef.current = "ready";
      setAvailability("ready");
      setNotice({
        kind: "conflict",
        text: `${source} changed this file while you have unsaved edits. Your text is preserved; compare, reload, or save your version as a new file.`,
      });
      return;
    }
    acceptDocument(next, `${source} saved a newer version. The editor has been updated.`);
  }, [acceptDocument]);

  useEffect(() => {
    let cancelled = false;
    const relocated = relocatedAssociation.current;
    if (relocated?.root === selectedRoot && relocated.path === selectedPath) {
      relocatedAssociation.current = null;
      return;
    }
    if (!selectedRoot || !selectedPath) {
      documentRef.current = null;
      externalDocumentRef.current = null;
      dirtyRef.current = false;
      textRef.current = "";
      setDocument(null);
      setExternalDocument(null);
      setText("");
      setDirty(false);
      availabilityRef.current = "none";
      setAvailability("none");
      setNotice(null);
      return;
    }
    availabilityRef.current = "loading";
    setAvailability("loading");
    setNotice({ kind: "info", text: `Opening ${selectedPath}…` });
    void serverRef.current
      .readTextFile(selectedRoot, selectedPath)
      .then((next) => {
        if (!cancelled) acceptDocument(next);
      })
      .catch((error) => {
        if (cancelled) return;
        documentRef.current = null;
        externalDocumentRef.current = null;
        dirtyRef.current = false;
        textRef.current = "";
        setDocument(null);
        setExternalDocument(null);
        setText("");
        setDirty(false);
        const nextAvailability = isMissingError(error) ? "missing" : "none";
        availabilityRef.current = nextAvailability;
        setAvailability(nextAvailability);
        setNotice({
          kind: "error",
          text: isMissingError(error)
            ? `The selected file is missing, moved, deleted, or its location is unavailable: ${selectedPath}`
            : `Could not open ${selectedPath}: ${friendlyError(error)}`,
        });
      });
    return () => {
      cancelled = true;
    };
  }, [acceptDocument, selectedPath, selectedRoot]);

  useEffect(() => {
    if (!selectedRoot || !selectedPath) return;
    let cancelled = false;
    let checking = false;
    const check = async () => {
      if (checking) return;
      checking = true;
      try {
        const next = await serverRef.current.readTextFile(selectedRoot, selectedPath);
        if (cancelled) return;
        const current = documentRef.current;
        const pending = externalDocumentRef.current;
        if (availabilityRef.current === "missing") {
          surfaceExternalDocument(next, "Another editor or external program");
          return;
        }
        if (isSameDocumentVersion(pending, next)) return;
        if (!isSameDocumentVersion(current, next)) surfaceExternalDocument(next, "Another editor or external program");
      } catch (error) {
        if (cancelled) return;
        if (isMissingError(error)) {
          availabilityRef.current = "missing";
          setAvailability("missing");
          setNotice({
            kind: "error",
            text: `The selected file is missing, moved, deleted, or its location is unavailable: ${selectedPath}. ${
              textRef.current ? "The last loaded text is retained in this window." : ""
            }`.trim(),
          });
        }
      } finally {
        checking = false;
      }
    };
    const timer = window.setInterval(() => void check(), EXTERNAL_CHECK_INTERVAL_MILLIS);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [reloadFiles, selectedPath, selectedRoot, surfaceExternalDocument]);

  useEffect(() => {
    const saved = (event: Event) => {
      const detail = textDocumentFromSavedEvent(event);
      if (!detail || detail.sourcePaneId === paneId) return;
      const next = detail.document;
      if (next.root_id !== selectedRoot || next.path !== selectedPath) return;
      if (availabilityRef.current !== "missing" && (isSameDocumentVersion(externalDocumentRef.current, next) || isSameDocumentVersion(documentRef.current, next))) return;
      surfaceExternalDocument(next, "Another Text Editor window");
    };
    window.addEventListener(TEXT_FILE_SAVED_EVENT, saved);
    return () => window.removeEventListener(TEXT_FILE_SAVED_EVENT, saved);
  }, [paneId, selectedPath, selectedRoot, surfaceExternalDocument]);

  useEffect(() => {
    const operated = (event: Event) => {
      const detail = textFileOperationFromEvent(event);
      if (!detail) return;
      const current = documentRef.current;
      const root = current?.root_id ?? selectedRoot;
      const path = current?.path ?? selectedPath;
      const change = textFileLocationChange(root, path, detail);
      if (!change) return;
      if (change.kind === "deleted") {
        availabilityRef.current = "missing";
        setAvailability("missing");
        setNotice({
          kind: "error",
          text: `The selected file was deleted or moved to Trash: ${path}. The last loaded text is retained in this window.`,
        });
        return;
      }

      if (current) {
        const moved = { ...current, root_id: change.rootId, path: change.path };
        documentRef.current = moved;
        setDocument(moved);
      }
      const pending = externalDocumentRef.current;
      if (pending) {
        const movedPending = { ...pending, root_id: change.rootId, path: change.path };
        externalDocumentRef.current = movedPending;
        setExternalDocument(movedPending);
      }
      availabilityRef.current = "ready";
      setAvailability("ready");
      setNotice({
        kind: "info",
        text: `The open file moved from ${path} to ${change.path}. Unsaved text, if any, is still in this editor.`,
      });
      if (current) relocatedAssociation.current = { root: change.rootId, path: change.path };
      if (paneId) dispatch({ type: "SET_TEXT_EDITOR_FILE", id: paneId, root: change.rootId, path: change.path });
      void reloadFiles(change.rootId);
    };
    window.addEventListener(TEXT_FILE_OPERATION_EVENT, operated);
    return () => window.removeEventListener(TEXT_FILE_OPERATION_EVENT, operated);
  }, [dispatch, paneId, reloadFiles, selectedPath, selectedRoot]);

  useEffect(() => {
    const warn = (event: BeforeUnloadEvent) => {
      if (!dirtyRef.current) return;
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, []);

  useEffect(() => {
    if (!paneId) return;
    return registerPaneRemovalGuard(paneId, () => dirtyRef.current ? "Text Editor has unsaved changes." : null);
  }, [paneId]);

  useEffect(() => {
    if (!selectedRoot || !selectedPath || availability !== "ready") return;
    try {
      const persisted = pane?.textEditorView;
      const saved = persisted?.root === selectedRoot && persisted.path === selectedPath
        ? persisted
        : JSON.parse(localStorage.getItem(viewStateKey(paneId, selectedRoot, selectedPath)) ?? "null") as Partial<LegacyTextEditorViewState> | null;
      if (!saved) return;
      const control = textarea.current;
      if (!control) return;
      const start = Math.min(Math.max(0, saved.selectionStart ?? 0), control.value.length);
      const end = Math.min(Math.max(start, saved.selectionEnd ?? start), control.value.length);
      control.setSelectionRange(start, end);
      control.scrollTop = Math.max(0, saved.scrollTop ?? 0);
    } catch {
      // Corrupt view metadata is non-authoritative and safe to ignore.
    }
  }, [availability, document?.revision, editorMode, paneId, selectedPath, selectedRoot]);

  const persistViewState = () => {
    const control = textarea.current;
    if (!control || !selectedRoot || !selectedPath) return;
    const view = {
      root: selectedRoot,
      path: selectedPath,
      selectionStart: control.selectionStart,
      selectionEnd: control.selectionEnd,
      scrollTop: control.scrollTop,
    };
    try {
      localStorage.setItem(viewStateKey(paneId, selectedRoot, selectedPath), JSON.stringify(view));
    } catch {
      // View state must never prevent editing or saving the file itself.
    }
    if (paneId) dispatch({ type: "SET_TEXT_EDITOR_VIEW", id: paneId, ...view });
  };

  const confirmDiscard = () => !dirtyRef.current || window.confirm("Discard unsaved changes?");

  const associateFile = (root: string, path: string) => {
    if (!confirmDiscard()) return;
    if (paneId) dispatch({ type: "SET_TEXT_EDITOR_FILE", id: paneId, root, path });
  };

  const openFile = async () => {
    const result = await openFileManagerPicker({
      purpose: "Open a text file",
      target: "files",
      multiple: false,
      allowedExtensions: [...TEXT_FILE_EXTENSIONS],
      initialRootId: selectedRoot || undefined,
      initialDirectory: parentDirectory(selectedPath),
    });
    if (!result || !confirmDiscard()) return;
    if (Array.isArray(result)) {
      const selected = result[0];
      if (selected && paneId) dispatch({ type: "SET_TEXT_EDITOR_FILE", id: paneId, root: selected.rootId, path: selected.entry.path });
      return;
    }
    if (paneReadOnly) {
      setNotice({ kind: "error", text: "This Text Editor pane is read-only and cannot import a system-picked file." });
      return;
    }
    const file = result.files[0];
    const targetRoot = selectedRoot || roots[0]?.id;
    if (!file || !targetRoot) return;
    if (file.size > MAX_TEXT_FILE_BYTES) {
      setNotice({ kind: "error", text: "Text Editor files may not exceed 4 MiB." });
      return;
    }
    try {
      const importedText = new TextDecoder("utf-8", { fatal: true }).decode(await file.arrayBuffer());
      const next = await serverRef.current.saveTextFile(targetRoot, file.name, importedText, null);
      acceptDocument(next, `Imported ${file.name} from the system picker.`);
      if (paneId) dispatch({ type: "SET_TEXT_EDITOR_FILE", id: paneId, root: targetRoot, path: next.path });
      publishTextFileSaved(next, paneId);
      void reloadFiles(targetRoot);
    } catch (error) {
      setNotice({ kind: "error", text: `Could not import ${file.name}: ${friendlyError(error)}` });
    }
  };

  const publishSaved = (next: TextDocument) => {
    publishTextFileSaved(next, paneId);
  };

  const saveTo = async (path: string, revision: string | null, associationChanges: boolean, successMessage: string) => {
    const targetRoot = documentRef.current?.root_id ?? selectedRoot;
    if (!targetRoot || saving) return;
    setSaving(true);
    try {
      const next = await serverRef.current.saveTextFile(targetRoot, path, textRef.current, revision);
      acceptDocument(next, successMessage);
      if (associationChanges && paneId) dispatch({ type: "SET_TEXT_EDITOR_FILE", id: paneId, root: targetRoot, path: next.path });
      publishSaved(next);
      void reloadFiles(targetRoot);
    } catch (error) {
      if (revision) {
        try {
          const latest = await serverRef.current.readTextFile(selectedRoot, path);
          if (!isSameDocumentVersion(documentRef.current, latest)) {
            surfaceExternalDocument(latest, "Another editor or external program");
            return;
          }
        } catch (readError) {
          if (isMissingError(readError)) {
            availabilityRef.current = "missing";
            setAvailability("missing");
            setNotice({
              kind: "error",
              text: "The file was removed before it could be saved. Your unsaved text is preserved; recreate it or save a copy.",
            });
            return;
          }
        }
      }
      setNotice({ kind: "error", text: `Save failed: ${friendlyError(error)}` });
    } finally {
      setSaving(false);
    }
  };

  const save = () => {
    const current = documentRef.current;
    if (!current || paneReadOnly || current.read_only || externalDocumentRef.current || availability === "missing") return;
    void saveTo(current.path, current.revision, false, "Saved");
  };

  const saveAs = () => {
    if (!selectedRoot || paneReadOnly) return;
    const suggested = selectedPath || "operator-notes.txt";
    const path = window.prompt("Save as path (relative to this file location)", suggested)?.trim();
    if (!path) return;
    if (!isSupportedTextFile(path)) {
      setNotice({ kind: "error", text: "Text Editor supports .txt, .md, .csv, and .log files." });
      return;
    }
    void saveTo(path, null, path !== selectedPath, path === selectedPath ? "File recreated" : `Saved as ${path}`);
  };

  const recreate = () => {
    if (paneReadOnly || !selectedPath || !window.confirm(`Recreate ${selectedPath} from the text retained in this window?`)) return;
    void saveTo(selectedPath, null, false, "File recreated");
  };

  const reloadExternal = () => {
    const latest = externalDocumentRef.current;
    if (!latest) return;
    if (dirtyRef.current && !window.confirm("Discard your unsaved version and load the newer file?")) return;
    acceptDocument(latest, "Reloaded the newer file");
  };

  const label = useMemo(() => document?.path || selectedPath || "No file selected", [document?.path, selectedPath]);
  const status = saving
    ? "Saving…"
    : externalDocument
      ? "Conflict"
      : availability === "loading"
        ? "Opening…"
        : availability === "missing"
          ? "Missing"
          : dirty
            ? "Unsaved"
            : document?.read_only || paneReadOnly
              ? "Read-only"
              : document
                ? "Saved"
                : "No file";
  const currentFileInList = selectedPath && !files.some((file) => file.path === selectedPath)
    ? ({ name: selectedPath, path: selectedPath, kind: "file", size: 0, modified_millis: null, created_millis: null, hidden: false, writable: false } satisfies FileEntry)
    : null;
  const chooserFiles = currentFileInList ? [currentFileInList, ...files] : files;

  return <section className="text-editor" aria-label="Text Editor" data-dirty={dirty || undefined}>
    {paneChrome?.info && createPortal(<span className="text-editor-header-state" title={`${status} · ${label}`}><strong className={`text-save-state ${dirty || externalDocument ? "dirty" : ""}`} role="status" aria-live="polite">{status}</strong> · {label}</span>, paneChrome.info)}
    {paneChrome?.toolbar && createPortal(<div className="text-editor-header-actions">
      <Button onClick={() => void openFile()}>Open File</Button>
      <Button disabled={!selectedRoot || filesLoading} onClick={() => void reloadFiles(selectedRoot)}>Refresh</Button>
      <Button disabled={!document || !dirty || paneReadOnly || document.read_only || saving || Boolean(externalDocument) || availability === "missing"} onClick={save}>Save</Button>
      <Button aria-label="Save As" disabled={!selectedRoot || paneReadOnly || saving} onClick={saveAs}>Save As…</Button>
    </div>, paneChrome.toolbar)}
    <div className="text-editor-toolbar">
      {!paneChrome && <Button onClick={() => void openFile()}>Open File</Button>}
      <Select
        aria-label="File root"
        value={selectedRoot}
        onChange={(event) => associateFile(event.target.value, "")}
      >
        {roots.map((root) => <option key={root.id} value={root.id}>{root.label}{root.writable ? "" : " (read-only)"}</option>)}
      </Select>
      <Select
        aria-label="Choose File"
        value={selectedPath}
        disabled={!selectedRoot || filesLoading}
        onChange={(event) => associateFile(selectedRoot, event.target.value)}
      >
        <option value="">{filesLoading ? "Loading text files…" : "Choose File…"}</option>
        {chooserFiles.map((file) => <option key={file.path} value={file.path}>{file.path}{file.writable ? "" : " (read-only or missing)"}</option>)}
      </Select>
      {!paneChrome && <><strong className={`text-save-state ${dirty || externalDocument ? "dirty" : ""}`} role="status" aria-live="polite">{status}</strong><span title={label}>{label}</span><Button disabled={!document || !dirty || paneReadOnly || document.read_only || saving || Boolean(externalDocument) || availability === "missing"} onClick={save}>Save</Button><Button aria-label="Save As" disabled={!selectedRoot || paneReadOnly || saving} onClick={saveAs}>Save As…</Button><Button disabled={!selectedRoot || filesLoading} onClick={() => void reloadFiles(selectedRoot)}>Refresh</Button></>}
      <Button disabled={!selectedPath} onClick={() => associateFile(selectedRoot, "")}>Close File</Button>
    </div>
    {paneReadOnly && <div className="file-message text-editor-read-only" role="status">This pane is configured read-only. Editing, Save, Save As, import, and recreate actions are disabled.</div>}
    {notice && <div id={messageId} className={`file-message text-editor-${notice.kind}`} role={notice.kind === "info" ? "status" : "alert"}>{notice.text}</div>}
    {availability === "missing" && selectedPath && <div className="file-message text-editor-missing-actions" aria-label="Missing file actions">
      <Button disabled={saving || paneReadOnly} onClick={recreate}>Recreate File</Button>
      <Button disabled={saving || paneReadOnly} onClick={saveAs}>Save Retained Text As…</Button>
      <Button disabled={filesLoading} onClick={() => void reloadFiles(selectedRoot)}>Look for Moved File</Button>
    </div>}
    {externalDocument && <section className="file-message text-editor-conflict" aria-label="File revision conflict">
      <b>A newer file revision is available.</b>
      <Button onClick={reloadExternal}>Reload Newer Version</Button>
      <Button disabled={saving || paneReadOnly} onClick={saveAs}>Save My Version As…</Button>
      <details>
        <summary>Compare versions</summary>
        <label>Your unsaved version<TextArea aria-label="Your unsaved version" value={text} readOnly /></label>
        <label>Newer file version<TextArea aria-label="Newer file version" value={externalDocument.text} readOnly /></label>
      </details>
    </section>}
    <div className={`text-editor-content mode-${editorMode}`}>
    {editorMode !== "markdown" && <TextArea
      ref={textarea}
      aria-label="File text"
      aria-describedby={notice ? messageId : undefined}
      value={text}
      readOnly={!document || paneReadOnly || document.read_only || availability === "missing"}
      onBlur={persistViewState}
      onChange={(event) => {
        textRef.current = event.target.value;
        dirtyRef.current = event.target.value !== documentRef.current?.text;
        setText(event.target.value);
        setDirty(dirtyRef.current);
        if (!dirtyRef.current && externalDocumentRef.current) acceptDocument(externalDocumentRef.current, "Your edits now match the stored revision");
      }}
      onKeyDown={(event) => {
        if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
          event.preventDefault();
          if (event.shiftKey) saveAs();
          else save();
        }
      }}
      placeholder={availability === "missing" ? "The associated file is missing." : "Choose a UTF-8 text file to begin."}
    />}
    {editorMode !== "plain" && <article className="text-editor-markdown" aria-label="Rendered Markdown">
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{text}</ReactMarkdown>
    </article>}
    </div>
  </section>;
}
