import {
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { useFiles } from "../features/files/FilesContext";
import type { FileConflictChoice, FileDirectory, FileEntry, FileNativeNote, FileOperationResult, FileRoot, TextDocument } from "../api/types";
import { Button, CheckboxField, TextArea, TextInput } from "../components/common";
import { registerPaneRemovalGuard } from "../components/shell/paneRemovalGuard";
import { usePaneChromeTargets } from "../components/shell/PaneChromeContext";
import { WindowHeader } from "../components/window-kit";
import { useOptionalApp } from "../state/AppContext";
import "./FileManagerWindow.css";
import {
  publishTextFileOperation,
  publishTextFileSaved,
  TEXT_FILE_OPERATION_EVENT,
  TEXT_FILE_SAVED_EVENT,
  textDocumentFromSavedEvent,
  textFileLocationChange,
  textFileOperationFromEvent,
} from "./textFileSync";
import type { WindowProps } from "./windowTypes";

const textExtensions = new Set(["txt", "md", "csv", "log"]);
const imageExtensions = new Set(["png", "jpg", "jpeg", "gif", "webp"]);
const audioExtensions = new Set(["mp3", "wav"]);

type FileManagerOperationKind = "rename" | "copy" | "move" | "delete";
export type FileManagerTarget = "files" | "folders" | "either";

export interface FileManagerSelection {
  rootId: string;
  entry: FileEntry;
}

export interface FileManagerPickerOptions {
  purpose?: string;
  target?: FileManagerTarget;
  multiple?: boolean;
  allowedExtensions?: string[];
  initialRootId?: string;
  initialDirectory?: string;
  selectLabel?: string;
  cancelLabel?: string;
  hideCancel?: boolean;
  onSelect: (selection: FileManagerSelection[]) => void;
  onCancel: () => void;
}

export interface FileManagerProps {
  picker?: FileManagerPickerOptions;
  instanceId?: string;
  paneId?: string;
  closeable?: boolean;
  purpose?: string;
}

interface Location {
  rootId: string;
  path: string;
}

interface FileOperationState {
  kind: FileManagerOperationKind;
  sources: FileManagerSelection[];
  renameDraft: string;
  confirming: boolean;
}

interface ConflictState {
  operation: FileOperationState;
  applyToAll: boolean;
}

type FileHeaderMenuKind = "location" | "edit" | "create" | "view";
interface FileHeaderMenu {
  kind: FileHeaderMenuKind;
  anchor: DOMRect;
}

type FileMenuIconName = "chevron" | "rename" | "copy" | "move" | "delete" | "file-new" | "folder-new" | "list" | "grid" | "folder";

function FileMenuIcon({ name }: { name: FileMenuIconName }) {
  const paths: Record<Exclude<FileMenuIconName, "grid">, ReactNode> = {
    chevron: <path d="m6 9 6 6 6-6"/>,
    rename: <><path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/></>,
    copy: <><rect width="13" height="13" x="9" y="9" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></>,
    move: <><path d="M5 9h11"/><path d="m13 6 3 3-3 3"/><path d="M19 15H8"/><path d="m11 12-3 3 3 3"/></>,
    delete: <><path d="M3 6h18"/><path d="M8 6V4h8v2"/><path d="m19 6-1 14H6L5 6"/><path d="M10 11v5M14 11v5"/></>,
    "file-new": <><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z"/><path d="M14 2v6h6M12 18v-6M9 15h6"/></>,
    "folder-new": <><path d="M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"/><path d="M12 11v6M9 14h6"/></>,
    list: <><path d="M8 6h13M8 12h13M8 18h13"/><path d="M3 6h.01M3 12h.01M3 18h.01"/></>,
    folder: <path d="M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"/>,
  };
  return <svg className="file-menu-icon" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
    {name === "grid" ? <><rect width="7" height="7" x="3" y="3"/><rect width="7" height="7" x="14" y="3"/><rect width="7" height="7" x="3" y="14"/><rect width="7" height="7" x="14" y="14"/></> : paths[name]}
  </svg>;
}

let pendingDeskAction: FileManagerOperationKind | null = null;
let claimedInputOwner: string | null = null;

export function extension(name: string) {
  const value = name.split(".").pop()?.toLowerCase() ?? "";
  return value === name.toLowerCase() ? "" : value;
}

export function parentPath(path: string) {
  return path.split("/").slice(0, -1).join("/");
}

export function joinPath(parent: string, name: string) {
  return parent ? `${parent}/${name}` : name;
}

export function sortFileEntries(entries: FileEntry[]) {
  return [...entries].sort((left, right) => {
    if (left.kind !== right.kind) return left.kind === "folder" ? -1 : 1;
    return left.name.localeCompare(right.name, undefined, { numeric: true, sensitivity: "base" });
  });
}

export function validItemName(name: string) {
  const value = name.trim();
  const upper = value.replace(/[. ]+$/, "").split(".")[0]?.toUpperCase();
  const reserved = new Set(["CON", "PRN", "AUX", "NUL", ...Array.from({ length: 9 }, (_, index) => `COM${index + 1}`), ...Array.from({ length: 9 }, (_, index) => `LPT${index + 1}`)]);
  return Boolean(value && value !== "." && value !== ".." && new TextEncoder().encode(value).length <= 255 && !/[\\/\0-\x1f\x7f]/.test(value) && !/[. ]$/.test(value) && !reserved.has(upper));
}

export function nextKeepBothName(name: string, existingNames: Iterable<string>) {
  const existing = new Set([...existingNames].map((value) => value.toLocaleLowerCase()));
  const dot = name.lastIndexOf(".");
  const hasExtension = dot > 0;
  const stem = hasExtension ? name.slice(0, dot) : name;
  const suffix = hasExtension ? name.slice(dot) : "";
  let sequence = 1;
  let candidate = `${stem} copy${suffix}`;
  while (existing.has(candidate.toLocaleLowerCase())) {
    sequence += 1;
    candidate = `${stem} copy ${sequence}${suffix}`;
  }
  return candidate;
}

function assertFileOperationComplete(result: FileOperationResult) {
  if (result.complete) return;
  const failures = result.items.filter((item) => item.status === "failed").map((item) => `${item.source}: ${item.error ?? "operation failed"}`);
  throw new Error(failures.join("; ") || "one or more file operations failed");
}

export function operationFromCommandLine(commandLine: string): FileManagerOperationKind | null {
  const command = commandLine.trim().toUpperCase();
  if (command === "SET") return "rename";
  if (command === "CPY" || command === "COPY") return "copy";
  if (command === "MOV" || command === "MOVE") return "move";
  if (command === "DEL" || command === "DELETE") return "delete";
  return null;
}

export function pickerSelectionIsValid(selection: FileManagerSelection[], picker: FileManagerPickerOptions) {
  if (!selection.length || (!picker.multiple && selection.length !== 1)) return false;
  const target = picker.target ?? "files";
  const allowed = new Set((picker.allowedExtensions ?? []).map((value) => value.replace(/^\./, "").toLowerCase()));
  return selection.every(({ entry }) => {
    if (target === "files" && entry.kind !== "file") return false;
    if (target === "folders" && entry.kind !== "folder") return false;
    return entry.kind !== "file" || !allowed.size || allowed.has(extension(entry.name));
  });
}

function emptyOperation(kind: FileManagerOperationKind, sources: FileManagerSelection[] = []): FileOperationState {
  return {
    kind,
    sources,
    renameDraft: kind === "rename" && sources.length === 1 ? sources[0].entry.name : "",
    confirming: kind === "delete" && sources.length > 0,
  };
}

function selectionKey(selection: FileManagerSelection) {
  return `${selection.rootId}:${selection.entry.path}`;
}

function rootIcon(root: FileRoot) {
  if (root.removable || root.icon === "drive") return "⏏";
  if (root.id === "shows" || root.icon === "shows") return "🎭";
  if (root.icon && root.icon !== "folder") return root.icon;
  return "▣";
}

function itemIcon(item: FileEntry) {
  if (item.kind === "folder") return "📁";
  if (imageExtensions.has(extension(item.name))) return "▧";
  if (audioExtensions.has(extension(item.name))) return "♪";
  return "▤";
}

function formatSize(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(value < 10 * 1024 ? 1 : 0)} KB`;
  if (value < 1024 * 1024 * 1024) return `${(value / (1024 * 1024)).toFixed(value < 10 * 1024 * 1024 ? 1 : 0)} MB`;
  return `${(value / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function formatTime(value: number | null) {
  return value == null ? "Unavailable" : new Date(value).toLocaleString();
}

function safeObjectUrl(blob: Blob) {
  return typeof URL.createObjectURL === "function" ? URL.createObjectURL(blob) : "";
}

function releaseObjectUrl(url: string) {
  if (url.startsWith("blob:") && typeof URL.revokeObjectURL === "function") URL.revokeObjectURL(url);
}

function isMissingFileError(error: unknown) {
  const message = String(error).toLowerCase();
  return message.includes("not found") || message.includes("was removed") || message.includes("unavailable");
}

function RasterThumbnail({ rootId, entry, load }: { rootId: string; entry: FileEntry; load: (rootId: string, path: string) => Promise<Blob> }) {
  const [url, setUrl] = useState("");
  useEffect(() => {
    let cancelled = false;
    let allocated = "";
    void load(rootId, entry.path)
      .then((blob) => {
        allocated = safeObjectUrl(blob);
        if (cancelled) releaseObjectUrl(allocated);
        else setUrl(allocated);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
      releaseObjectUrl(allocated);
    };
  }, [entry.path, load, rootId]);
  return url ? <img className="file-thumbnail" src={url} alt="" /> : <span className="file-item-icon" aria-hidden="true">▧</span>;
}

export function FileManagerWindow({ builtIn, paneId }: WindowProps) {
  return <FileManager instanceId={paneId} paneId={paneId} closeable={builtIn} />;
}

export function FileManager({ picker, instanceId: suppliedInstanceId, paneId, closeable = false, purpose = "Browse and manage files" }: FileManagerProps) {
  const server = useFiles();
  const serverRef = useRef(server);
  serverRef.current = server;
  const fileRoots = server.fileRoots;
  const fileEntries = server.fileEntries;
  const app = useOptionalApp();
  const paneChrome = usePaneChromeTargets();
  const pane = app?.state.desks.flatMap((desk) => desk.panes).find((candidate) => candidate.id === paneId);
  const generatedId = useId();
  const instanceId = useRef(suppliedInstanceId ?? `file-manager-${generatedId.replaceAll(":", "")}`).current;
  const [roots, setRoots] = useState<FileRoot[]>([]);
  const [listing, setListing] = useState<FileDirectory | null>(null);
  const [history, setHistory] = useState<Location[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  const [selected, setSelected] = useState<FileManagerSelection[]>([]);
  const [selectionAnchor, setSelectionAnchor] = useState<string | null>(null);
  const [localHidden, setLocalHidden] = useState(false);
  const [view, setView] = useState<"list" | "grid">("list");
  const [sidePanel, setSidePanel] = useState<"none" | "navigation" | "info">("none");
  const [propertiesVisible, setPropertiesVisible] = useState(true);
  const [headerMenu, setHeaderMenu] = useState<FileHeaderMenu | null>(null);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [previewUrl, setPreviewUrl] = useState("");
  const [nativeNote, setNativeNote] = useState<FileNativeNote | null>(null);
  const [noteDraft, setNoteDraft] = useState("");
  const [editor, setEditor] = useState<TextDocument | null>(null);
  const [editorText, setEditorText] = useState("");
  const [editorConflict, setEditorConflict] = useState<TextDocument | null>(null);
  const [editorMissing, setEditorMissing] = useState(false);
  const editorRef = useRef<TextDocument | null>(null);
  const editorTextRef = useRef("");
  editorRef.current = editor;
  editorTextRef.current = editorText;
  const [operation, setOperationState] = useState<FileOperationState | null>(null);
  const operationRef = useRef<FileOperationState | null>(null);
  const [conflict, setConflict] = useState<ConflictState | null>(null);
  const [treeChildren, setTreeChildren] = useState<Record<string, FileEntry[]>>({});
  const [treeExpanded, setTreeExpanded] = useState<Set<string>>(new Set());
  const initialized = useRef(false);
  const current = historyIndex >= 0 ? history[historyIndex] : null;
  const rootId = current?.rootId ?? "";
  const currentPath = current?.path ?? "";
  const currentRoot = roots.find((root) => root.id === rootId) ?? null;
  const hidden = paneId ? Boolean(pane?.fileManagerShowHidden) : localHidden;
  const setHidden = (value: boolean) => {
    if (paneId && app) app.dispatch({ type: "SET_FILE_MANAGER_SHOW_HIDDEN", id: paneId, value });
    else setLocalHidden(value);
  };

  function hasUnsavedEditorText() {
    return Boolean(editorRef.current && editorTextRef.current !== editorRef.current.text);
  }

  function confirmDiscardEditor() {
    return !hasUnsavedEditorText() || window.confirm("Discard unsaved changes?");
  }

  const setOperation = useCallback((next: FileOperationState | null) => {
    operationRef.current = next;
    setOperationState(next);
  }, []);

  const navigate = useCallback((next: Location) => {
    if (!confirmDiscardEditor()) return;
    const retained = history.slice(0, historyIndex + 1);
    const previous = retained.at(-1);
    if (previous?.rootId === next.rootId && previous.path === next.path) return;
    const nextHistory = [...retained, next];
    setHistory(nextHistory);
    setHistoryIndex(nextHistory.length - 1);
    setSelected([]);
    setSelectionAnchor(null);
    setEditor(null);
    setEditorConflict(null);
    setSidePanel("none");
    setMessage("");
  }, [history, historyIndex]);

  const loadRoots = useCallback(async () => {
    try {
      const items = await fileRoots();
      setRoots(items);
      if (!initialized.current && items.length) {
        initialized.current = true;
        const requested = picker?.initialRootId;
        const initialRoot = items.find((root) => root.id === requested) ?? items[0];
        setHistory([{ rootId: initialRoot.id, path: picker?.initialDirectory ?? "" }]);
        setHistoryIndex(0);
      }
    } catch (error) {
      setMessage(`Locations unavailable: ${String(error)}`);
    }
  }, [fileRoots, picker?.initialDirectory, picker?.initialRootId]);

  useEffect(() => {
    void loadRoots();
    const timer = window.setInterval(() => void loadRoots(), 5000);
    return () => window.clearInterval(timer);
  }, [loadRoots]);

  useEffect(() => {
    if (!initialized.current || !rootId || roots.some((root) => root.id === rootId)) return;
    const fallback = roots[0];
    setMessage(`The location “${rootId}” was disconnected.`);
    if (fallback) {
      setHistory((value) => [...value.slice(0, historyIndex + 1), { rootId: fallback.id, path: "" }]);
      setHistoryIndex((value) => value + 1);
    }
  }, [historyIndex, rootId, roots]);

  const refresh = useCallback(async () => {
    if (!rootId) return;
    try {
      const next = await fileEntries(rootId, currentPath, hidden);
      setListing({ ...next, entries: sortFileEntries(next.entries) });
      setMessage((value) => value.startsWith("Could not open this location:") ? "" : value);
    } catch (error) {
      setListing(null);
      setMessage(`Could not open this location: ${String(error)}`);
    }
  }, [currentPath, fileEntries, hidden, rootId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    setTreeChildren({});
    setTreeExpanded(new Set());
  }, [hidden]);

  const previewSelection = selected.length === 1 && selected[0].entry.kind === "file"
    ? { rootId: selected[0].rootId, path: selected[0].entry.path, name: selected[0].entry.name }
    : null;

  useEffect(() => {
    let cancelled = false;
    let allocated = "";
    setPreviewUrl((value) => {
      releaseObjectUrl(value);
      return "";
    });
    if (!previewSelection) return;
    if (!imageExtensions.has(extension(previewSelection.name)) && !audioExtensions.has(extension(previewSelection.name))) return;
    if (audioExtensions.has(extension(previewSelection.name))) {
      void serverRef.current.fileStreamUrl(previewSelection.rootId, previewSelection.path)
        .then((url) => {
          if (!cancelled) setPreviewUrl(url);
        })
        .catch((error) => setMessage(`Preview unavailable: ${String(error)}`));
    } else {
      void serverRef.current.fileThumbnail(previewSelection.rootId, previewSelection.path)
        .then((blob) => {
          allocated = safeObjectUrl(blob);
          if (cancelled) releaseObjectUrl(allocated);
          else setPreviewUrl(allocated);
        })
        .catch((error) => setMessage(`Preview unavailable: ${String(error)}`));
    }
    return () => {
      cancelled = true;
      releaseObjectUrl(allocated);
    };
  }, [previewSelection?.name, previewSelection?.path, previewSelection?.rootId]);

  const noteSelection = selected.length === 1 && selected[0].entry.note_supported
    ? { rootId: selected[0].rootId, path: selected[0].entry.path }
    : null;

  useEffect(() => {
    let cancelled = false;
    setNativeNote(null);
    setNoteDraft("");
    if (!noteSelection) return;
    void serverRef.current.readFileNote(noteSelection.rootId, noteSelection.path).then((note) => {
      if (cancelled) return;
      setNativeNote(note);
      setNoteDraft(note.note ?? "");
    }).catch((error) => {
      if (!cancelled) setMessage(`Notes unavailable: ${String(error)}`);
    });
    return () => { cancelled = true; };
  }, [noteSelection?.path, noteSelection?.rootId]);

  useEffect(() => {
    if (!paneId) return;
    return registerPaneRemovalGuard(paneId, () => hasUnsavedEditorText() ? "File Manager has unsaved text changes." : null);
  }, [paneId]);

  const surfaceTextRevision = useCallback((next: TextDocument, source: string) => {
    const currentEditor = editorRef.current;
    if (!currentEditor || currentEditor.root_id !== next.root_id || currentEditor.path !== next.path) return;
    if (currentEditor.revision === next.revision && currentEditor.text === next.text && currentEditor.read_only === next.read_only) return;
    if (editorTextRef.current !== currentEditor.text && editorTextRef.current !== next.text) {
      setEditorConflict(next);
      setMessage(`${source} changed this file while the File Manager has unsaved edits. The local text has been preserved.`);
      return;
    }
    editorRef.current = next;
    editorTextRef.current = next.text;
    setEditor(next);
    setEditorText(next.text);
    setEditorConflict(null);
    setEditorMissing(false);
    setMessage(`${source} saved a newer version. The File Manager editor has been updated.`);
  }, []);

  useEffect(() => {
    const saved = (event: Event) => {
      const detail = textDocumentFromSavedEvent(event);
      if (!detail || detail.sourcePaneId === instanceId) return;
      surfaceTextRevision(detail.document, "Another editor");
    };
    window.addEventListener(TEXT_FILE_SAVED_EVENT, saved);
    return () => window.removeEventListener(TEXT_FILE_SAVED_EVENT, saved);
  }, [instanceId, surfaceTextRevision]);

  useEffect(() => {
    const operated = (event: Event) => {
      const detail = textFileOperationFromEvent(event);
      const currentEditor = editorRef.current;
      if (!detail || !currentEditor) return;
      const change = textFileLocationChange(currentEditor.root_id, currentEditor.path, detail);
      if (!change) return;
      if (change.kind === "deleted") {
        setEditorMissing(true);
        setMessage(`The open file was deleted or moved to Trash. Its last loaded text is retained.`);
        return;
      }
      const moved = { ...currentEditor, root_id: change.rootId, path: change.path };
      editorRef.current = moved;
      setEditor(moved);
      setEditorConflict((pending) => pending ? { ...pending, root_id: change.rootId, path: change.path } : null);
      setEditorMissing(false);
      setMessage(`The open file moved to ${change.path}. Unsaved text, if any, was retained.`);
    };
    window.addEventListener(TEXT_FILE_OPERATION_EVENT, operated);
    return () => window.removeEventListener(TEXT_FILE_OPERATION_EVENT, operated);
  }, []);

  useEffect(() => {
    if (!editor) return;
    let cancelled = false;
    let checking = false;
    const check = async () => {
      if (checking) return;
      checking = true;
      try {
        const next = await serverRef.current.readTextFile(editor.root_id, editor.path);
        if (!cancelled) surfaceTextRevision(next, "Another editor or external program");
      } catch (error) {
        if (!cancelled && isMissingFileError(error)) {
          setEditorMissing(true);
          setMessage("The open file is missing, moved, deleted, or its location is unavailable. Its last loaded text is retained.");
        }
      } finally {
        checking = false;
      }
    };
    const timer = window.setInterval(() => void check(), 1_500);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [editor?.path, editor?.root_id, surfaceTextRevision]);

  useEffect(() => () => {
    if (claimedInputOwner === instanceId) {
      claimedInputOwner = null;
      void serverRef.current.releaseFileInput(instanceId).catch(() => undefined);
    }
  }, [instanceId]);

  useEffect(() => {
    const routeDeskAction = (event: Event) => {
      const action = String((event as CustomEvent<string>).detail ?? "").toLowerCase();
      const next = action === "set" ? "rename" : action === "copy" || action === "cpy" ? "copy" : action === "move" || action === "mov" ? "move" : action === "delete" || action === "del" ? "delete" : null;
      if (next) pendingDeskAction = next;
      if (claimedInputOwner !== instanceId || !operationRef.current) return;
      if (action === "escape" || action === "esc") {
        event.preventDefault();
        cancelOperation();
      }
      if (action === "enter" || action === "ent") {
        event.preventDefault();
        void completeOperation();
      }
    };
    const routeFileInput = (event: Event) => {
      const detail = (event as CustomEvent<{ action?: string; instance_id?: string }>).detail;
      if (detail?.instance_id !== instanceId || claimedInputOwner !== instanceId || !operationRef.current) return;
      if (detail.action === "escape") {
        event.preventDefault();
        cancelOperation();
      } else if (detail.action === "enter") {
        event.preventDefault();
        void completeOperation();
      }
    };
    const releaseUnclaimed = (event: PointerEvent) => {
      if (pendingDeskAction && (!(event.target instanceof Element) || !event.target.closest(".file-manager"))) pendingDeskAction = null;
    };
    window.addEventListener("light:desk-action", routeDeskAction);
    window.addEventListener("light:file-manager-input", routeFileInput);
    document.addEventListener("pointerdown", releaseUnclaimed, true);
    return () => {
      window.removeEventListener("light:desk-action", routeDeskAction);
      window.removeEventListener("light:file-manager-input", routeFileInput);
      document.removeEventListener("pointerdown", releaseUnclaimed, true);
    };
  });

  useEffect(() => {
    if (!operation || claimedInputOwner !== instanceId) return;
    const timer = window.setInterval(() => {
      void serverRef.current.claimFileInput(instanceId, operation.kind, "toolbar").catch(() => {
        cancelOperation("The server released this File Manager input context.");
      });
    }, 30_000);
    return () => window.clearInterval(timer);
  }, [instanceId, operation?.kind]);

  useEffect(() => {
    if (server.status === "connected" || !operationRef.current) return;
    cancelOperation("The file operation was cancelled because the desk connection was lost.");
  }, [server.status]);

  const pickerValid = Boolean(picker && pickerSelectionIsValid(selected, picker));

  useEffect(() => {
    const interceptKeys = (event: KeyboardEvent) => {
      const target = event.target;
      const editingName = target instanceof Element && Boolean(target.closest(".file-rename-editor"));
      if (event.key === "Escape" && operationRef.current && claimedInputOwner === instanceId) {
        event.preventDefault();
        event.stopImmediatePropagation();
        cancelOperation();
        return;
      }
      if (event.key === "Enter" && operationRef.current && claimedInputOwner === instanceId && !editingName) {
        event.preventDefault();
        event.stopImmediatePropagation();
        void completeOperation();
        return;
      }
      if (!picker || editingName) return;
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopImmediatePropagation();
        picker.onCancel();
      } else if (event.key === "Enter" && pickerValid) {
        event.preventDefault();
        event.stopImmediatePropagation();
        picker.onSelect(selected);
      }
    };
    const interceptTouchKey = (event: MouseEvent) => {
      if (claimedInputOwner !== instanceId || !operationRef.current) return;
      const key = (event.target as Element | null)?.closest<HTMLElement>("[data-keypad-key]")?.dataset.keypadKey;
      if (key !== "ENT" && key !== "ESC") return;
      event.preventDefault();
      event.stopImmediatePropagation();
      if (key === "ENT") void completeOperation();
      else cancelOperation();
    };
    window.addEventListener("keydown", interceptKeys, true);
    document.addEventListener("click", interceptTouchKey, true);
    return () => {
      window.removeEventListener("keydown", interceptKeys, true);
      document.removeEventListener("click", interceptTouchKey, true);
    };
  });

  function cancelOperation(reason = "File operation cancelled.") {
    setConflict(null);
    setOperation(null);
    if (claimedInputOwner === instanceId) {
      claimedInputOwner = null;
      void server.releaseFileInput(instanceId).catch(() => undefined);
    }
    setMessage(reason);
  }

  function beginOperation(kind: FileManagerOperationKind, sources = selected) {
    if ((kind === "rename" || kind === "delete") && !sources.length) {
      setMessage(`${kind === "rename" ? "Rename" : "Delete"} requires a selection.`);
      return;
    }
    if (kind === "rename" && sources.length !== 1) {
      setMessage("Rename requires exactly one selected item.");
      return;
    }
    claimedInputOwner = instanceId;
    setOperation(emptyOperation(kind, sources));
    setConflict(null);
    setMessage(kind === "copy" || kind === "move" ? `${kind === "copy" ? "Copy" : "Move"} is ready. Select sources, choose a destination, then press ENTER.` : "");
    void server.claimFileInput(instanceId, kind, "toolbar").catch((error) => {
      if (claimedInputOwner === instanceId) claimedInputOwner = null;
      setOperation(null);
      setMessage(`Could not claim File Manager input: ${String(error)}`);
    });
  }

  function claimPendingAction(event: ReactPointerEvent<HTMLElement>) {
    if (operationRef.current || picker) return;
    const pending = pendingDeskAction ?? operationFromCommandLine(server.commandLine);
    if (!pending) return;
    event.stopPropagation();
    pendingDeskAction = null;
    claimedInputOwner = instanceId;
    setOperation(emptyOperation(pending));
    setMessage(`${pending === "rename" ? "Rename" : pending[0].toUpperCase() + pending.slice(1)} claimed by this File Manager. Select the source.`);
    void server.claimFileInput(instanceId, pending, "pending").then(() => server.resetCommandLine()).catch((error) => {
      if (claimedInputOwner === instanceId) claimedInputOwner = null;
      pendingDeskAction = pending;
      setOperation(null);
      setMessage(`Could not claim the pending desk action: ${String(error)}`);
    });
  }

  function selectEntry(item: FileEntry, event: ReactMouseEvent<HTMLButtonElement>) {
    const value = { rootId, entry: item };
    const activeOperation = operationRef.current;
    if (activeOperation) {
      let sources: FileManagerSelection[];
      if (activeOperation.kind === "rename") sources = [value];
      else {
        const key = selectionKey(value);
        sources = activeOperation.sources.some((source) => selectionKey(source) === key)
          ? activeOperation.sources.filter((source) => selectionKey(source) !== key)
          : [...activeOperation.sources, value];
      }
      const next = {
        ...activeOperation,
        sources,
        renameDraft: activeOperation.kind === "rename" ? item.name : activeOperation.renameDraft,
        confirming: false,
      };
      setOperation(next);
      setSelected(sources.filter((source) => source.rootId === rootId));
      setSelectionAnchor(item.path);
      return;
    }
    const multiple = picker?.multiple ?? true;
    const toggle = multiple && (event.metaKey || event.ctrlKey);
    if (multiple && event.shiftKey && selectionAnchor && listing) {
      const ordered = listing.entries;
      const first = ordered.findIndex((entry) => entry.path === selectionAnchor);
      const last = ordered.findIndex((entry) => entry.path === item.path);
      if (first >= 0 && last >= 0) {
        const [start, end] = first < last ? [first, last] : [last, first];
        setSelected(ordered.slice(start, end + 1).map((entry) => ({ rootId, entry })));
        return;
      }
    }
    if (toggle) {
      const key = selectionKey(value);
      setSelected((values) => values.some((candidate) => selectionKey(candidate) === key) ? values.filter((candidate) => selectionKey(candidate) !== key) : [...values, value]);
    } else setSelected([value]);
    setSelectionAnchor(item.path);
  }

  async function openText(file: FileManagerSelection) {
    try {
      const document = await server.readTextFile(file.rootId, file.entry.path);
      setEditor(document);
      setEditorText(document.text);
      setEditorConflict(null);
      setEditorMissing(false);
    } catch (error) {
      setMessage(`Text editor unavailable: ${String(error)}`);
    }
  }

  async function saveText() {
    if (!editor || editorConflict || editorMissing) return;
    setBusy(true);
    try {
      const document = await server.saveTextFile(editor.root_id, editor.path, editorText, editor.revision);
      setEditor(document);
      setEditorText(document.text);
      setEditorConflict(null);
      setMessage("Saved.");
      publishTextFileSaved(document, instanceId);
    } catch (error) {
      try {
        const latest = await server.readTextFile(editor.root_id, editor.path);
        if (latest.revision !== editor.revision || latest.text !== editor.text || latest.read_only !== editor.read_only) {
          surfaceTextRevision(latest, "Another editor or external program");
          return;
        }
      } catch (latestError) {
        if (isMissingFileError(latestError)) {
          setEditorMissing(true);
          setMessage("The file was removed before it could be saved. The File Manager retained your text; recreate the file to store it.");
          return;
        }
      }
      setMessage(`Could not save: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  async function recreateText() {
    if (!editor || !editorMissing || !window.confirm(`Recreate ${editor.path} from the retained text?`)) return;
    setBusy(true);
    try {
      const document = await server.saveTextFile(editor.root_id, editor.path, editorText, null);
      editorRef.current = document;
      setEditor(document);
      setEditorText(document.text);
      setEditorConflict(null);
      setEditorMissing(false);
      setMessage("File recreated.");
      publishTextFileSaved(document, instanceId);
      await refreshAfterMutation();
    } catch (error) {
      setMessage(`Could not recreate the file: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  async function saveNativeNote() {
    if (!nativeNote) return;
    setBusy(true);
    try {
      const saved = await server.saveFileNote(nativeNote.root_id, nativeNote.path, noteDraft);
      setNativeNote(saved);
      setNoteDraft(saved.note ?? "");
      setMessage("Native filesystem note saved.");
    } catch (error) {
      setMessage(`Could not save the native note: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  async function create(folder: boolean) {
    const name = prompt(folder ? "Folder name" : "File name")?.trim();
    if (!name) return;
    if (!validItemName(name)) {
      setMessage("Names may not be empty, dot paths, or contain path separators.");
      return;
    }
    setBusy(true);
    try {
      await server.fileOperation(rootId, { operation: folder ? "create_folder" : "create_file", destination: currentPath, name });
      setMessage(`${folder ? "Folder" : "File"} created.`);
      await refreshAfterMutation();
    } catch (error) {
      setMessage(`Could not create ${folder ? "folder" : "file"}: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  async function refreshAfterMutation() {
    setSelected([]);
    setTreeChildren({});
    setTreeExpanded(new Set());
    await refresh();
  }

  async function runOperation(activeOperation: FileOperationState, conflictChoice?: FileConflictChoice, applyToAll = false) {
    if (!activeOperation.sources.length) {
      setMessage("Select at least one source item.");
      return;
    }
    const sourceRoot = activeOperation.sources[0].rootId;
    if (activeOperation.sources.some((source) => source.rootId !== sourceRoot)) {
      setMessage("A single operation cannot mix sources from different roots.");
      return;
    }
    if (activeOperation.kind === "rename") {
      if (!validItemName(activeOperation.renameDraft)) {
        setMessage("Names may not be empty, dot paths, or contain path separators.");
        return;
      }
      const result = await server.fileOperation(sourceRoot, {
        operation: "rename",
        sources: [activeOperation.sources[0].entry.path],
        name: activeOperation.renameDraft.trim(),
        conflict: conflictChoice,
        apply_to_all: applyToAll,
      });
      publishTextFileOperation("rename", result, instanceId);
      assertFileOperationComplete(result);
      return;
    }
    if (activeOperation.kind === "delete") {
      const useTrash = activeOperation.sources.every((source) => source.entry.trash_supported);
      const operation = useTrash ? "trash" : "delete";
      const result = await server.fileOperation(sourceRoot, { operation, sources: activeOperation.sources.map((source) => source.entry.path) });
      publishTextFileOperation(operation, result, instanceId);
      assertFileOperationComplete(result);
      return;
    }
    const result = await server.fileOperation(sourceRoot, {
      operation: activeOperation.kind,
      sources: activeOperation.sources.map((source) => source.entry.path),
      destination: currentPath,
      destination_root_id: rootId,
      conflict: conflictChoice,
      apply_to_all: applyToAll,
    });
    publishTextFileOperation(activeOperation.kind, result, instanceId);
    assertFileOperationComplete(result);
  }

  async function finishSuccessfulOperation(activeOperation: FileOperationState) {
    setConflict(null);
    setOperation(null);
    if (claimedInputOwner === instanceId) {
      claimedInputOwner = null;
      void server.releaseFileInput(instanceId).catch(() => undefined);
    }
    setMessage(`${activeOperation.kind === "delete" ? "Delete" : activeOperation.kind[0].toUpperCase() + activeOperation.kind.slice(1)} completed.`);
    await refreshAfterMutation();
  }

  async function completeOperation() {
    const activeOperation = operationRef.current;
    if (!activeOperation || busy) return;
    if (!activeOperation.sources.length) {
      setMessage("Select at least one source item.");
      return;
    }
    if (activeOperation.kind === "delete" && !activeOperation.confirming) {
      setOperation({ ...activeOperation, confirming: true });
      return;
    }
    setBusy(true);
    try {
      await runOperation(activeOperation);
      await finishSuccessfulOperation(activeOperation);
    } catch (error) {
      const reason = String(error);
      if (/409|already exist|conflict/i.test(reason) && activeOperation.kind !== "delete") setConflict({ operation: activeOperation, applyToAll: false });
      else setMessage(`File operation failed: ${reason}`);
    } finally {
      setBusy(false);
    }
  }

  async function resolveReplace() {
    await resolveConflictChoice("replace");
  }

  async function resolveSkip() {
    await resolveConflictChoice("skip");
  }

  async function resolveKeepBoth() {
    await resolveConflictChoice("keep_both");
  }

  async function resolveConflictChoice(choice: FileConflictChoice) {
    if (!conflict || busy) return;
    const activeOperation = conflict.operation;
    setBusy(true);
    try {
      if (conflict.applyToAll || activeOperation.sources.length === 1 || activeOperation.kind === "rename") {
        await runOperation(activeOperation, choice, conflict.applyToAll);
        await finishSuccessfulOperation(activeOperation);
        if (choice === "skip") setMessage("Conflicting item(s) skipped; existing items were left unchanged.");
        return;
      }
      const destinationNames = new Set((listing?.entries ?? []).map((entry) => entry.name.toLocaleLowerCase()));
      const conflicting = activeOperation.sources.filter((source) => destinationNames.has(source.entry.name.toLocaleLowerCase()));
      const clear = activeOperation.sources.filter((source) => !destinationNames.has(source.entry.name.toLocaleLowerCase()));
      for (const source of clear) await runOperation({ ...activeOperation, sources: [source] });
      const [resolved, ...remaining] = conflicting;
      if (resolved) await runOperation({ ...activeOperation, sources: [resolved] }, choice, false);
      if (remaining.length) {
        const next = { ...activeOperation, sources: remaining };
        setOperation(next);
        setConflict({ operation: next, applyToAll: false });
        setMessage(`Resolved one conflict. ${remaining.length} conflict${remaining.length === 1 ? " remains" : "s remain"}.`);
      } else {
        await finishSuccessfulOperation(activeOperation);
        if (choice === "skip") setMessage("Conflicting item skipped; the existing item was left unchanged.");
      }
    } catch (error) {
      setMessage(`${choice === "keep_both" ? "Keep Both" : choice === "replace" ? "Replace" : "Skip"} failed: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  const loadTreeFolder = async (location: Location) => {
    const key = `${location.rootId}:${location.path}`;
    if (treeExpanded.has(key)) {
      setTreeExpanded((values) => {
        const next = new Set(values);
        next.delete(key);
        return next;
      });
      return;
    }
    setTreeExpanded((values) => new Set(values).add(key));
    if (!treeChildren[key]) {
      try {
        const contents = await server.fileEntries(location.rootId, location.path, hidden);
        setTreeChildren((values) => ({ ...values, [key]: sortFileEntries(contents.entries).filter((entry) => entry.kind === "folder") }));
      } catch (error) {
        setMessage(`Could not expand folder: ${String(error)}`);
      }
    }
  };

  const renderTreeFolders = (treeRootId: string, path: string, depth: number): ReactNode => {
    const key = `${treeRootId}:${path}`;
    if (!treeExpanded.has(key)) return null;
    const children = treeChildren[key];
    if (!children) return <span className="file-tree-loading" role="status">Loading…</span>;
    return children.map((folder) => {
      const childKey = `${treeRootId}:${folder.path}`;
      const expanded = treeExpanded.has(childKey);
      return <div className="file-tree-branch" key={childKey}>
        <Button
          variant="ghost"
          className={treeRootId === rootId && folder.path === currentPath ? "active" : ""}
          style={{ paddingInlineStart: `${0.4 + depth * 0.8}rem` }}
          aria-expanded={expanded}
          onClick={() => {
            navigate({ rootId: treeRootId, path: folder.path });
            void loadTreeFolder({ rootId: treeRootId, path: folder.path });
          }}
        ><span aria-hidden="true">{expanded ? "▾" : "▸"} 📁</span> {folder.name}</Button>
        {renderTreeFolders(treeRootId, folder.path, depth + 1)}
      </div>;
    });
  };

  const breadcrumbs = currentPath ? currentPath.split("/") : [];
  const operationLabel = operation?.kind === "copy" ? "Copy" : operation?.kind === "move" ? "Move" : operation?.kind === "rename" ? "Rename" : operation?.kind === "delete" ? "Delete" : "";
  const trashForOperation = Boolean(operation?.kind === "delete" && operation.sources.length && operation.sources.every((source) => source.entry.trash_supported));
  const sourceKeys = new Set(operation?.sources.map(selectionKey) ?? []);
  const selectedKeys = new Set(selected.map(selectionKey));
  const headerPath = `/${currentPath}`;
  const openHeaderMenu = (kind: FileHeaderMenuKind, event: ReactMouseEvent<HTMLButtonElement>) => {
    const anchor = event.currentTarget.getBoundingClientRect();
    setHeaderMenu((current) => current?.kind === kind ? null : { kind, anchor });
  };
  const closeHeaderMenu = () => setHeaderMenu(null);
  const menuAction = (action: () => void | Promise<void>) => {
    closeHeaderMenu();
    void action();
  };
  const locationChoices = [{ label: currentRoot?.label ?? "Location", path: "" }, ...breadcrumbs.map((_, index) => ({ label: `/${breadcrumbs.slice(0, index + 1).join("/")}`, path: breadcrumbs.slice(0, index + 1).join("/") }))];
  const headerPathControl = <Button
    variant="ghost"
    className="file-manager-header-path"
    aria-label={`Current path ${headerPath}`}
    aria-haspopup="menu"
    aria-expanded={headerMenu?.kind === "location"}
    title={headerPath}
    onClick={(event) => openHeaderMenu("location", event)}
  ><span>{headerPath}</span><FileMenuIcon name="chevron" /></Button>;
  const headerActions = <div className="file-manager-header-actions">
    <Button aria-label="Edit" aria-haspopup="menu" aria-expanded={headerMenu?.kind === "edit"} onClick={(event) => openHeaderMenu("edit", event)}><span>Edit</span><FileMenuIcon name="chevron" /></Button>
    <Button aria-label="New" aria-haspopup="menu" aria-expanded={headerMenu?.kind === "create"} onClick={(event) => openHeaderMenu("create", event)}><span>New</span><FileMenuIcon name="chevron" /></Button>
    <Button aria-label="View" aria-haspopup="menu" aria-expanded={headerMenu?.kind === "view"} onClick={(event) => openHeaderMenu("view", event)}><span>View</span><FileMenuIcon name="chevron" /></Button>
    <Button aria-label="Back" disabled={historyIndex <= 0} onClick={() => { setHistoryIndex((value) => value - 1); setSelected([]); }}>←</Button>
    <Button aria-label="Forward" disabled={historyIndex < 0 || historyIndex >= history.length - 1} onClick={() => { setHistoryIndex((value) => value + 1); setSelected([]); }}>→</Button>
  </div>;

  return <section
    className={`file-manager fm-${view} fm-${sidePanel}-open ${propertiesVisible ? "fm-properties-visible" : "fm-properties-hidden"}`}
    aria-label={picker ? "File picker" : "File Manager"}
    data-file-manager-instance={instanceId}
    onPointerDownCapture={claimPendingAction}
  >
    {!paneChrome && !picker && <WindowHeader
      title="File Manager"
      info={{ primary: purpose, secondary: headerPathControl }}
      toolbar={headerActions}
      actions={closeable && app ? [[{ id: "close", label: "×", ariaLabel: "Close File Manager", onClick: () => app.dispatch({ type: "CLOSE_FILE_MANAGER" }) }]] : []}
    />}
    {paneChrome?.info && createPortal(headerPathControl, paneChrome.info)}
    {paneChrome?.toolbar && createPortal(headerActions, paneChrome.toolbar)}
    {headerMenu && createPortal(<div className="file-header-menu-layer" onPointerDown={(event) => event.target === event.currentTarget && closeHeaderMenu()}>
      <div className="file-header-menu" role="menu" aria-label={`${headerMenu.kind === "create" ? "New" : `${headerMenu.kind[0].toUpperCase()}${headerMenu.kind.slice(1)}`} menu`} style={{ top: headerMenu.anchor.bottom + 3, left: Math.max(3, Math.min(headerMenu.anchor.left, window.innerWidth - 230)) }}>
        {headerMenu.kind === "location" && locationChoices.map((choice) => <Button key={choice.path} role="menuitem" className="file-menu-location" active={choice.path === currentPath} onClick={() => menuAction(() => navigate({ rootId, path: choice.path }))}><FileMenuIcon name="folder"/><span>{choice.label}</span></Button>)}
        {headerMenu.kind === "edit" && <>
          <Button className="file-menu-rename" role="menuitem" disabled={selected.length !== 1} onClick={() => menuAction(() => beginOperation("rename"))}><FileMenuIcon name="rename"/><span>Rename</span></Button>
          <Button className="file-menu-copy" role="menuitem" disabled={!selected.length} onClick={() => menuAction(() => beginOperation("copy"))}><FileMenuIcon name="copy"/><span>Copy</span></Button>
          <Button className="file-menu-move" role="menuitem" disabled={!selected.length} onClick={() => menuAction(() => beginOperation("move"))}><FileMenuIcon name="move"/><span>Move</span></Button>
          <Button className="file-menu-delete" role="menuitem" disabled={!selected.length} onClick={() => menuAction(() => beginOperation("delete"))}><FileMenuIcon name="delete"/><span>Delete</span></Button>
        </>}
        {headerMenu.kind === "create" && <>
          <Button className="file-menu-new-file" role="menuitem" onClick={() => menuAction(() => create(false))}><FileMenuIcon name="file-new"/><span>New File</span></Button>
          <Button className="file-menu-new-folder" role="menuitem" onClick={() => menuAction(() => create(true))}><FileMenuIcon name="folder-new"/><span>New Folder</span></Button>
        </>}
        {headerMenu.kind === "view" && <>
          <Button role="menuitemradio" aria-checked={view === "list"} onClick={() => menuAction(() => setView("list"))}><span className="file-menu-check" aria-hidden="true">{view === "list" ? "✓" : ""}</span><FileMenuIcon name="list"/><span>List</span></Button>
          <Button role="menuitemradio" aria-checked={view === "grid"} onClick={() => menuAction(() => setView("grid"))}><span className="file-menu-check" aria-hidden="true">{view === "grid" ? "✓" : ""}</span><FileMenuIcon name="grid"/><span>Grid</span></Button>
          <div className="file-menu-divider" role="separator" />
          <Button role="menuitemcheckbox" aria-checked={hidden} onClick={() => menuAction(() => setHidden(!hidden))}><span className="file-menu-checkbox" aria-hidden="true">{hidden ? "✓" : ""}</span><span>Show Hidden Files</span></Button>
          <Button role="menuitemcheckbox" aria-checked={propertiesVisible} onClick={() => menuAction(() => { setPropertiesVisible((value) => !value); setSidePanel("none"); })}><span className="file-menu-checkbox" aria-hidden="true">{propertiesVisible ? "✓" : ""}</span><span>Show Properties Sidebar</span></Button>
        </>}
      </div>
    </div>, document.body)}
    <div className="file-toolbar">
      <Button className="file-navigation-toggle" active={sidePanel === "navigation"} onClick={() => setSidePanel((value) => value === "navigation" ? "none" : "navigation")}>Navigation</Button>
      <nav aria-label="Breadcrumb">
        <Button variant="ghost" onClick={() => rootId && navigate({ rootId, path: "" })}>{currentRoot?.label ?? "Location"}</Button>
        {breadcrumbs.map((part, index) => <Button variant="ghost" key={`${part}-${index}`} onClick={() => navigate({ rootId, path: breadcrumbs.slice(0, index + 1).join("/") })}>/ {part}</Button>)}
      </nav>
      <Button className="file-info-toggle" active={sidePanel === "info"} onClick={() => { setPropertiesVisible(true); setSidePanel((value) => value === "info" ? "none" : "info"); }}>Info</Button>
      {operation && <div className="file-operation-actions" aria-label={`${operationLabel} operation`}>
        {(operation.kind === "copy" || operation.kind === "move") && <Button variant="primary" disabled={!operation.sources.length || busy} onClick={() => void completeOperation()}>{operationLabel} Here</Button>}
        {operation.kind === "rename" && <Button variant="primary" disabled={!operation.sources.length || !validItemName(operation.renameDraft) || busy} onClick={() => void completeOperation()}>Rename</Button>}
        {operation.kind === "delete" && <Button variant="primary" disabled={!operation.sources.length || busy} onClick={() => void completeOperation()}>Delete</Button>}
        <Button disabled={busy} onClick={() => cancelOperation()}>Cancel</Button>
      </div>}
      {picker && !operation && <div className="file-picker-actions">
        <Button variant="primary" disabled={!pickerValid} onClick={() => pickerValid && picker.onSelect(selected)}>{picker.selectLabel ?? "Select"}</Button>
        {!picker.hideCancel && <Button onClick={picker.onCancel}>{picker.cancelLabel ?? "Cancel"}</Button>}
      </div>}
    </div>
    {(message || busy || operation) && <div className={`file-message ${busy ? "is-busy" : ""}`} role="status">
      {busy ? "Working…" : message || `${operationLabel}: ${operation?.sources.length ?? 0} source item(s) selected.`}
    </div>}
    <div className="file-columns">
      <aside className="file-roots" aria-label="Folder navigation">
        <h3>Locations</h3>
        {roots.map((root) => {
          const key = `${root.id}:`;
          const expanded = treeExpanded.has(key);
          return <div className="file-tree-root" key={root.id}>
            <Button
              variant="ghost"
              className={root.id === rootId && !currentPath ? "active" : ""}
              aria-expanded={expanded}
              onClick={() => {
                navigate({ rootId: root.id, path: "" });
                void loadTreeFolder({ rootId: root.id, path: "" });
              }}
            ><span aria-hidden="true">{expanded ? "▾" : "▸"} {rootIcon(root)}</span> {root.label}{root.removable ? " (Removable)" : ""}</Button>
            {renderTreeFolders(root.id, "", 1)}
          </div>;
        })}
        {!roots.length && <p>No configured or removable locations are available.</p>}
      </aside>
      <main className={view === "grid" ? "file-grid" : "file-list"} aria-label="Directory contents">
        {view === "list" && <div className="file-list-head" role="row"><b>Name</b><b>Type</b><b>Size</b><b>Modified</b></div>}
        {listing?.entries.map((item) => {
          const value = { rootId, entry: item };
          const key = selectionKey(value);
          const selectedItem = selectedKeys.has(key) || sourceKeys.has(key);
          const pickerAllowed = !picker || pickerSelectionIsValid([value], { ...picker, multiple: false });
          return <Button
            variant="ghost"
            key={item.path}
            className={`${selectedItem ? "selected" : ""} ${picker && !pickerAllowed ? "picker-invalid" : ""}`}
            aria-pressed={selectedItem}
            aria-label={`${item.name}, ${item.kind}`}
            onClick={(event) => selectEntry(item, event)}
            onDoubleClick={() => {
              if (item.kind === "folder") navigate({ rootId, path: item.path });
              else if (!picker && textExtensions.has(extension(item.name))) void openText(value);
            }}
          >
            <span className="file-item-name">
              {view === "grid" && item.kind === "file" && imageExtensions.has(extension(item.name))
                ? <RasterThumbnail rootId={rootId} entry={item} load={server.fileThumbnail} />
                : <span className="file-item-icon" aria-hidden="true">{itemIcon(item)}</span>}
              <span>{item.name}</span>
            </span>
            {view === "list" && <>
              <span>{item.kind === "folder" ? "Folder" : extension(item.name).toUpperCase() || "File"}</span>
              <span>{item.kind === "file" ? formatSize(item.size) : "—"}</span>
              <span>{formatTime(item.modified_millis)}</span>
            </>}
          </Button>;
        })}
        {listing && !listing.entries.length && <p className="file-empty-directory">This folder is empty.</p>}
        {!listing && rootId && !busy && <p className="file-empty-directory">The directory is unavailable.</p>}
      </main>
      <aside className="file-properties" aria-label="Selection properties">
        <h3>Properties</h3>
        {selected.length === 1 ? <FileProperties selection={selected[0]} previewUrl={previewUrl} nativeNote={nativeNote} noteDraft={noteDraft} busy={busy} onNoteDraft={setNoteDraft} onSaveNote={() => void saveNativeNote()} onOpenText={picker ? undefined : openText} /> : <p>{selected.length ? `${selected.length} items selected` : "Select an item"}</p>}
      </aside>
    </div>
    {operation?.kind === "rename" && operation.sources.length === 1 && <section className="file-operation-panel file-rename-editor" aria-label="Rename item">
      <strong>Rename {operation.sources[0].entry.name}</strong>
      <TextInput
        aria-label="New name"
        autoFocus
        value={operation.renameDraft}
        onChange={(event) => setOperation({ ...operation, renameDraft: event.target.value })}
        onKeyDown={(event) => {
          if (event.key === "Enter") { event.preventDefault(); void completeOperation(); }
          if (event.key === "Escape") { event.preventDefault(); cancelOperation(); }
        }}
      />
      {!validItemName(operation.renameDraft) && <span role="alert">Enter a name without path separators.</span>}
    </section>}
    {operation?.kind === "delete" && operation.confirming && <section className="file-confirmation" role="dialog" aria-modal="true" aria-label={trashForOperation ? "Confirm move to trash" : "Confirm permanent deletion"}>
      <h3>Delete {operation.sources.length} item{operation.sources.length === 1 ? "" : "s"}?</h3>
      <p>{trashForOperation ? "The selected item(s) will be moved to the platform Trash." : "Trash is unavailable on this filesystem. This deletion is permanent."}</p>
      <div><Button variant="primary" disabled={busy} onClick={() => void completeOperation()}>{trashForOperation ? "Move to Trash" : "Delete Permanently"}</Button><Button disabled={busy} onClick={() => cancelOperation()}>Cancel</Button></div>
    </section>}
    {conflict && <section className="file-confirmation" role="dialog" aria-modal="true" aria-label="Resolve name conflict">
      <h3>An item with that name already exists</h3>
      <p>Choose how this conflict should be handled.</p>
      {conflict.operation.sources.length > 1 && <CheckboxField label="Apply to All" checked={conflict.applyToAll} onChange={(event) => setConflict({ ...conflict, applyToAll: event.target.checked })} />}
      <div><Button variant="primary" disabled={busy} onClick={() => void resolveReplace()}>Replace</Button><Button disabled={busy} onClick={() => void resolveKeepBoth()}>Keep Both</Button><Button disabled={busy} onClick={() => void resolveSkip()}>Skip</Button><Button disabled={busy} onClick={() => cancelOperation()}>Cancel</Button></div>
    </section>}
    {editor && <div className="file-editor">
      <header><b>{editor.path}</b><span role="status">{editorMissing ? "Missing" : editorConflict ? "Conflict" : editorText !== editor.text ? "Unsaved" : "Saved"}</span><Button disabled={editorText === editor.text || editor.read_only || busy || Boolean(editorConflict) || editorMissing} onClick={() => void saveText()}>Save</Button>{editorMissing && <Button disabled={busy} onClick={() => void recreateText()}>Recreate File</Button>}<Button onClick={() => { if (confirmDiscardEditor()) { setEditor(null); setEditorConflict(null); setEditorMissing(false); } }}>Close</Button></header>
      {editorConflict && <div className="file-message" role="alert">A newer file revision is available. Your unsaved text has not been overwritten. <Button onClick={() => { setEditor(editorConflict); setEditorText(editorConflict.text); setEditorConflict(null); setMessage("Reloaded the newer file revision."); }}>Reload Newer Version</Button></div>}
      {editorMissing && <div className="file-message" role="alert">The associated file is missing. The last loaded text is retained until you recreate or close it.</div>}
      <TextArea aria-label="File text" value={editorText} readOnly={editor.read_only || editorMissing} onChange={(event) => setEditorText(event.target.value)} />
    </div>}
  </section>;
}

function FileProperties({ selection, previewUrl, nativeNote, noteDraft, busy, onNoteDraft, onSaveNote, onOpenText }: { selection: FileManagerSelection; previewUrl: string; nativeNote: FileNativeNote | null; noteDraft: string; busy: boolean; onNoteDraft: (value: string) => void; onSaveNote: () => void; onOpenText?: (selection: FileManagerSelection) => void }) {
  const { entry } = selection;
  const image = entry.kind === "file" && imageExtensions.has(extension(entry.name));
  const audio = entry.kind === "file" && audioExtensions.has(extension(entry.name));
  const text = entry.kind === "file" && textExtensions.has(extension(entry.name));
  return <>
    <b>{entry.name}</b>
    <dl>
      <dt>Type</dt><dd>{entry.kind === "folder" ? "Folder" : extension(entry.name).toUpperCase() || "File"}</dd>
      <dt>Size</dt><dd>{entry.kind === "file" ? formatSize(entry.size) : "—"}</dd>
      <dt>Created</dt><dd>{formatTime(entry.created_millis)}</dd>
      <dt>Modified</dt><dd>{formatTime(entry.modified_millis)}</dd>
      <dt>Access</dt><dd>{entry.writable ? "Read and write" : "Read only"}</dd>
    </dl>
    {entry.note_supported && nativeNote?.supported
      ? <label className="file-notes"><span>Notes</span><TextArea aria-label="Notes" value={noteDraft} onChange={(event) => onNoteDraft(event.target.value)} /><Button disabled={busy || noteDraft === (nativeNote.note ?? "")} onClick={onSaveNote}>Save Note</Button></label>
      : <label className="file-notes"><span>Notes</span><TextArea aria-label="Notes" value="Notes unavailable on this filesystem" disabled readOnly /></label>}
    {text && onOpenText && <Button onClick={() => onOpenText(selection)}>Edit Text</Button>}
    {previewUrl && image && <img className="file-preview" src={previewUrl} alt={`Preview of ${entry.name}`} />}
    {previewUrl && audio && <audio aria-label={`Audio preview of ${entry.name}`} src={previewUrl} controls preload="metadata" />}
    {!image && !audio && !text && entry.kind === "file" && <p>Preview unavailable for this file type.</p>}
  </>;
}
