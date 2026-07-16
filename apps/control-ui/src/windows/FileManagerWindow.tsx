import { useEffect, useMemo, useState } from "react";
import { useServer } from "../api/ServerContext";
import type { FileDirectory, FileEntry, FileRoot, TextDocument } from "../api/types";
import { Button, TextArea } from "../components/common";
import type { WindowProps } from "./windowTypes";

const textExtensions = new Set(["txt", "md", "csv", "log"]);
const imageExtensions = new Set(["png", "jpg", "jpeg", "gif", "webp"]);
const audioExtensions = new Set(["mp3", "wav"]);
const extension = (name: string) => name.split(".").pop()?.toLowerCase() ?? "";
const parentPath = (path: string) => path.split("/").slice(0, -1).join("/");

export function FileManagerWindow(_props: WindowProps) {
  const server = useServer();
  const [roots, setRoots] = useState<FileRoot[]>([]);
  const [rootId, setRootId] = useState("");
  const [listing, setListing] = useState<FileDirectory | null>(null);
  const [selected, setSelected] = useState<FileEntry[]>([]);
  const [history, setHistory] = useState<string[]>([""]);
  const [historyIndex, setHistoryIndex] = useState(0);
  const [hidden, setHidden] = useState(false);
  const [grid, setGrid] = useState(false);
  const [message, setMessage] = useState("");
  const [previewUrl, setPreviewUrl] = useState("");
  const [editor, setEditor] = useState<TextDocument | null>(null);
  const [editorText, setEditorText] = useState("");
  const currentPath = history[historyIndex] ?? "";
  const refresh = async (nextRoot = rootId, path = currentPath) => { if (!nextRoot) return; try { setListing(await server.fileEntries(nextRoot, path, hidden)); setSelected([]); setMessage(""); } catch (error) { setMessage(String(error)); } };
  useEffect(() => { void server.fileRoots().then((items) => { setRoots(items); setRootId((value) => value || items[0]?.id || ""); }).catch((error) => setMessage(String(error))); }, []);
  useEffect(() => { void refresh(); }, [rootId, currentPath, hidden]);
  useEffect(() => { setEditor(null); setEditorText(""); setPreviewUrl((value) => { if (value) URL.revokeObjectURL(value); return ""; }); const file = selected.length === 1 ? selected[0] : null; if (!file || file.kind !== "file") return; if (imageExtensions.has(extension(file.name)) || audioExtensions.has(extension(file.name))) void server.fileContent(rootId, file.path).then((blob) => setPreviewUrl(URL.createObjectURL(blob))).catch((error) => setMessage(String(error))); }, [rootId, selected]);
  const navigate = (path: string) => { const next = [...history.slice(0, historyIndex + 1), path]; setHistory(next); setHistoryIndex(next.length - 1); };
  const breadcrumbs = useMemo(() => currentPath ? currentPath.split("/") : [], [currentPath]);
  const openText = async (file: FileEntry) => { try { const doc = await server.readTextFile(rootId, file.path); setEditor(doc); setEditorText(doc.text); } catch (error) { setMessage(String(error)); } };
  const saveText = async () => { if (!editor) return; try { const doc = await server.saveTextFile(rootId, editor.path, editorText, editor.revision); setEditor(doc); setEditorText(doc.text); setMessage("Saved"); } catch (error) { setMessage(String(error)); } };
  const remove = async () => { if (!selected.length || !confirm(`Permanently delete ${selected.length} selected item${selected.length === 1 ? "" : "s"}? Trash is unavailable from this server operation.`)) return; try { await server.fileOperation(rootId, { operation: "delete", sources: selected.map((item) => item.path) }); await refresh(); } catch (error) { setMessage(String(error)); } };
  const rename = async () => { if (selected.length !== 1) return; const name = prompt("New name", selected[0].name); if (!name) return; try { await server.fileOperation(rootId, { operation: "rename", sources: [selected[0].path], name }); await refresh(); } catch (error) { setMessage(String(error)); } };
  const create = async (folder: boolean) => { const name = prompt(folder ? "Folder name" : "File name"); if (!name) return; try { await server.fileOperation(rootId, { operation: folder ? "create_folder" : "create_file", destination: currentPath, name }); await refresh(); } catch (error) { setMessage(String(error)); } };
  return <section className="file-manager" aria-label="File Manager">
    <div className="file-toolbar"><Button disabled={historyIndex === 0} onClick={() => setHistoryIndex((value) => value - 1)}>Back</Button><Button disabled={historyIndex >= history.length - 1} onClick={() => setHistoryIndex((value) => value + 1)}>Forward</Button><nav aria-label="Breadcrumb"><Button variant="ghost" onClick={() => navigate("")}>Root</Button>{breadcrumbs.map((part, index) => <Button variant="ghost" key={`${part}-${index}`} onClick={() => navigate(breadcrumbs.slice(0, index + 1).join("/"))}>/ {part}</Button>)}</nav><Button className={grid ? "" : "active"} onClick={() => setGrid(false)}>List</Button><Button className={grid ? "active" : ""} onClick={() => setGrid(true)}>Grid</Button><Button className={hidden ? "active" : ""} onClick={() => setHidden((value) => !value)}>Hidden</Button><Button onClick={() => void create(false)}>New File</Button><Button onClick={() => void create(true)}>New Folder</Button><Button disabled={selected.length !== 1} onClick={() => void rename()}>Rename</Button><Button disabled={!selected.length} onClick={() => void remove()}>Delete</Button></div>
    {message && <div className="file-message" role="status">{message}</div>}
    <div className="file-columns"><aside className="file-roots"><h3>Locations</h3>{roots.map((root) => <Button variant="ghost" className={root.id === rootId ? "active" : ""} key={root.id} onClick={() => { setRootId(root.id); setHistory([""]); setHistoryIndex(0); }}>{root.icon === "drive" ? "⏏" : "▣"} {root.label}</Button>)}</aside>
      <main className={grid ? "file-grid" : "file-list"}>{!grid && <div className="file-list-head"><b>Name</b><b>Type</b><b>Size</b><b>Modified</b></div>}{listing?.entries.map((item) => <Button variant="ghost" key={item.path} className={selected.some((value) => value.path === item.path) ? "selected" : ""} onClick={(event) => setSelected((values) => event.metaKey || event.ctrlKey ? values.some((value) => value.path === item.path) ? values.filter((value) => value.path !== item.path) : [...values, item] : [item])} onDoubleClick={() => item.kind === "folder" ? navigate(item.path) : textExtensions.has(extension(item.name)) ? void openText(item) : undefined}><span>{item.kind === "folder" ? "📁" : "▤"} {item.name}</span>{!grid && <><span>{item.kind}</span><span>{item.kind === "file" ? item.size.toLocaleString() : "—"}</span><span>{item.modified_millis ? new Date(item.modified_millis).toLocaleString() : "—"}</span></>}</Button>)}</main>
      <aside className="file-properties"><h3>Properties</h3>{selected.length === 1 ? <><b>{selected[0].name}</b><dl><dt>Type</dt><dd>{selected[0].kind}</dd><dt>Size</dt><dd>{selected[0].size.toLocaleString()}</dd><dt>Modified</dt><dd>{selected[0].modified_millis ? new Date(selected[0].modified_millis!).toLocaleString() : "Unavailable"}</dd></dl>{textExtensions.has(extension(selected[0].name)) && <Button onClick={() => void openText(selected[0])}>Edit Text</Button>}{previewUrl && imageExtensions.has(extension(selected[0].name)) && <img src={previewUrl} alt={`Preview of ${selected[0].name}`} />}{previewUrl && audioExtensions.has(extension(selected[0].name)) && <audio src={previewUrl} controls />}</> : <p>{selected.length ? `${selected.length} items selected` : "Select an item"}</p>}</aside>
    </div>
    {editor && <div className="file-editor"><header><b>{editor.path}</b><span>{editorText !== editor.text ? "Unsaved" : "Saved"}</span><Button disabled={editorText === editor.text || editor.read_only} onClick={() => void saveText()}>Save</Button><Button onClick={() => { if (editorText === editor.text || confirm("Discard unsaved changes?")) setEditor(null); }}>Close</Button></header><TextArea aria-label="File text" value={editorText} readOnly={editor.read_only} onChange={(event) => setEditorText(event.target.value)} /></div>}
  </section>;
}
