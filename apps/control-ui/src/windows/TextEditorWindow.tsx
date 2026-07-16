import { useEffect, useMemo, useState } from "react";
import { useServer } from "../api/ServerContext";
import type { FileEntry, FileRoot, TextDocument } from "../api/types";
import { useApp } from "../state/AppContext";
import { Button, Select, TextArea } from "../components/common";
import type { WindowProps } from "./windowTypes";

export function TextEditorWindow({ paneId }: WindowProps) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const pane = state.desks.flatMap((desk) => desk.panes).find((candidate) => candidate.id === paneId);
  const [roots, setRoots] = useState<FileRoot[]>([]);
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [document, setDocument] = useState<TextDocument | null>(null);
  const [text, setText] = useState("");
  const [dirty, setDirty] = useState(false);
  const [message, setMessage] = useState("");
  const selectedRoot = pane?.textFileRoot ?? roots[0]?.id ?? "";
  useEffect(() => { void server.fileRoots().then(setRoots).catch((error) => setMessage(String(error))); }, []);
  useEffect(() => { if (!selectedRoot) return; void server.fileEntries(selectedRoot).then((listing) => setFiles(listing.entries.filter((entry) => entry.kind === "file"))).catch((error) => setMessage(String(error))); }, [selectedRoot]);
  useEffect(() => { if (!selectedRoot || !pane?.textFilePath) { setDocument(null); setText(""); setDirty(false); return; } void server.readTextFile(selectedRoot, pane.textFilePath).then((next) => { setDocument(next); setText(next.text); setDirty(false); setMessage(""); }).catch((error) => { setDocument(null); setMessage(String(error)); }); }, [selectedRoot, pane?.textFilePath]);
  useEffect(() => { const warn = (event: BeforeUnloadEvent) => { if (dirty) event.preventDefault(); }; addEventListener("beforeunload", warn); return () => removeEventListener("beforeunload", warn); }, [dirty]);
  const save = async () => { if (!document) return; try { const next = await server.saveTextFile(document.root_id, document.path, text, document.revision); setDocument(next); setText(next.text); setDirty(false); setMessage("Saved"); window.dispatchEvent(new CustomEvent("light:text-file-saved", { detail: next })); } catch (error) { setMessage(String(error)); } };
  const choose = (path: string) => { if (dirty && !confirm("Discard unsaved changes?")) return; if (paneId) dispatch({ type: "SET_TEXT_EDITOR_FILE", id: paneId, root: selectedRoot, path }); };
  const label = useMemo(() => document?.path ?? pane?.textFilePath ?? "No file selected", [document, pane?.textFilePath]);
  return <section className="text-editor" aria-label="Text Editor">
    <div className="text-editor-toolbar"><Select aria-label="File root" value={selectedRoot} onChange={(event) => { if (dirty && !confirm("Discard unsaved changes?")) return; if (paneId) dispatch({ type: "SET_TEXT_EDITOR_FILE", id: paneId, root: event.target.value, path: "" }); }}>{roots.map((root) => <option key={root.id} value={root.id}>{root.label}</option>)}</Select><Select aria-label="Choose File" value={pane?.textFilePath ?? ""} onChange={(event) => choose(event.target.value)}><option value="">Choose File…</option>{files.map((file) => <option key={file.path} value={file.path}>{file.name}</option>)}</Select><strong className={`text-save-state ${dirty ? "dirty" : ""}`} role="status">{dirty ? "Unsaved" : document ? "Saved" : "No file"}</strong><span title={label}>{label}</span><Button disabled={!document || !dirty || document.read_only} onClick={() => void save()}>Save</Button></div>
    {message && <div className="file-message" role="status">{message}</div>}
    <TextArea aria-label="File text" value={text} readOnly={!document || document.read_only} onChange={(event) => { setText(event.target.value); setDirty(true); }} placeholder="Choose a UTF-8 text file to begin." />
  </section>;
}
