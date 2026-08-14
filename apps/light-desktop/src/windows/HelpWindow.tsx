import { useCallback, useEffect, useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { HelpCatalog, HelpCatalogEntry, HelpTopic } from "../api/types";
import { createLightApi } from "../api/client/api";
import type { WindowProps } from "./windowTypes";
import { prepareHelpMarkdown, safeHelpUrl } from "./helpMarkdown";
import { WindowHeader, WindowScrollArea } from "@tosklight/ui/window-kit";
import { Button } from "@tosklight/ui";

export type HelpUrlTransform = (url: string, kind: "link" | "image") => string | undefined;

function HelpLoadingIcon() {
  return <svg className="help-state-icon" viewBox="0 0 48 48" aria-hidden="true">
    <circle cx="24" cy="24" r="17"/>
    <path d="M24 14v11l7 5"/>
    <path d="M17 6h14M17 42h14"/>
  </svg>;
}

function HelpEmptyIcon() {
  return <svg className="help-state-icon" viewBox="0 0 48 48" aria-hidden="true">
    <path d="M7 10.5c6.5-2 12.2-.7 17 3.8v26c-4.8-4.5-10.5-5.8-17-3.8z"/>
    <path d="M41 10.5c-6.5-2-12.2-.7-17 3.8v26c4.8-4.5 10.5-5.8 17-3.8z"/>
    <path d="M14 18h4M30 18h4"/>
  </svg>;
}

function HelpWarningIcon() {
  return <svg viewBox="0 0 24 24" aria-hidden="true">
    <path d="M12 3 2.8 20h18.4z"/>
    <path d="M12 9v5M12 17.5v.1"/>
  </svg>;
}

export function HelpMarkdown({
  markdown,
  topicId,
  urlTransform = safeHelpUrl,
}: {
  markdown: string;
  topicId?: string;
  urlTransform?: HelpUrlTransform;
}) {
  return <ReactMarkdown
    remarkPlugins={[remarkGfm]}
    urlTransform={(url, key) =>
      (urlTransform === safeHelpUrl
        ? safeHelpUrl(url, key === "src" ? "image" : "link", topicId)
        : urlTransform(url, key === "src" ? "image" : "link")) ?? ""
    }
    components={{
      code({ className, children, ...props }) {
        const value = String(children).replace(/\n$/, "");
        if (!className && value.startsWith("help-keyboard:")) {
          return <span className="help-key keyboard-key"><small>keyboard</small><kbd>{value.slice(14)}</kbd></span>;
        }
        if (!className && value.startsWith("help-key:")) {
          const key = value.slice(9);
          const modifier = key.length > 1 ? key.at(-1) : undefined;
          const state = modifier === "+" ? "held" : modifier === "*" ? "optional" : "";
          const label = state ? key.slice(0, -1) : key;
          const category = /^(?:\d|0-9|\.)$/.test(label) ? "number" : label === "CLR" ? "clear" : label === "REC" ? "record" : "command";
          return <span className={`help-key desk-key desk-key-${category} ${state}`.trim()}><kbd>{label}</kbd>{state && <small>{state === "held" ? "hold" : "optional"}</small>}</span>;
        }
        if (!className && value.startsWith("help-placeholder:")) return <span className="help-placeholder">&lt;{value.slice(17)}&gt;</span>;
        return <code className={className} {...props}>{children}</code>;
      },
      a({ href, children, ...props }) { return href ? <a href={href} target={href.startsWith("https://") ? "_blank" : undefined} rel="noreferrer" {...props}>{children}</a> : <span>{children}</span>; },
      img({ src, alt, ...props }) { return src ? <img src={src} alt={alt ?? ""} loading="lazy" {...props} /> : <span className="help-image-error">Image unavailable: {alt}</span>; },
    }}
  >{prepareHelpMarkdown(markdown)}</ReactMarkdown>;
}

function containsTopic(items: HelpCatalogEntry[], id: string): boolean {
  return items.some((item) => item.id === id || containsTopic(item.children, id));
}

function firstTopic(items: HelpCatalogEntry[]): string | null {
  for (const item of items) {
    if (item.id) return item.id;
    const child = firstTopic(item.children);
    if (child) return child;
  }
  return null;
}

export function filterHelpEntries(entries: HelpCatalogEntry[], query: string): HelpCatalogEntry[] {
  const normalized = query.trim().toLocaleLowerCase();
  if (!normalized) return entries;
  return entries.flatMap((entry) => {
    if (entry.title.toLocaleLowerCase().includes(normalized)) return [entry];
    const children = filterHelpEntries(entry.children, normalized);
    return children.length ? [{ ...entry, children }] : [];
  });
}

export function HelpNavigation({
  entries,
  expanded,
  selected,
  depth = 0,
  onSelect,
  onToggle,
}: {
  entries: HelpCatalogEntry[];
  expanded: Set<string>;
  selected: string | null;
  depth?: number;
  onSelect: (id: string) => void;
  onToggle: (id: string) => void;
}) {
  return entries.map((entry) => {
    const open = entry.kind === "folder" && expanded.has(entry.id ?? entry.title);
    const key = entry.id ?? `folder:${entry.title}`;
    return <div className="help-nav-entry" key={key}>
      <div className={`help-nav-row ${entry.kind}`} style={{ paddingLeft: `${10 + depth * 18}px` }}>
        {entry.id
          ? <Button className={`help-nav-title ${entry.id === selected ? "active" : ""}`} onClick={() => onSelect(entry.id!)}>{entry.title}</Button>
          : <span className="help-nav-title">{entry.title}</span>}
        {entry.kind === "folder" && <Button
          className={`help-nav-chevron ${open ? "open" : ""}`}
          aria-label={`${open ? "Collapse" : "Expand"} ${entry.title}`}
          aria-expanded={open}
          onClick={() => onToggle(key)}
        ><span aria-hidden="true">›</span></Button>}
      </div>
      {open && <div className="help-nav-children">
        <HelpNavigation entries={entry.children} expanded={expanded} selected={selected} depth={depth + 1} onSelect={onSelect} onToggle={onToggle}/>
      </div>}
    </div>;
  });
}

export function HelpWindow({ compact }: WindowProps) {
  const client = useMemo(() => createLightApi().help, []);
  const [catalog, setCatalog] = useState<HelpCatalog | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [topic, setTopic] = useState<HelpTopic | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");

  const loadCatalog = useCallback(async () => {
    try {
      const next = await client.helpCatalog();
      setCatalog(next);
      setSelected((current) => current && containsTopic(next.topics, current) ? current : firstTopic(next.topics));
      setError(null);
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
  }, [client]);
  const loadTopic = useCallback(async (id: string) => {
    try { setTopic(await client.helpTopic(id)); setError(null); }
    catch (reason) { setTopic(null); setError(reason instanceof Error ? reason.message : String(reason)); }
  }, [client]);

  useEffect(() => { void loadCatalog(); }, [loadCatalog]);
  useEffect(() => { if (selected) void loadTopic(selected); else setTopic(null); }, [selected, loadTopic]);
  useEffect(() => {
    if (!catalog?.live) return;
    const timer = window.setInterval(() => { void loadCatalog(); if (selected) void loadTopic(selected); }, 1_000);
    return () => window.clearInterval(timer);
  }, [catalog?.live, selected, loadCatalog, loadTopic]);

  return <HelpWindowView
    catalog={catalog}
    compact={compact}
    error={error}
    loading={!catalog && !error}
    onSelect={setSelected}
    onQueryChange={setQuery}
    query={query}
    selected={selected}
    topic={topic}
  />;
}

export interface HelpWindowViewProps {
  catalog: HelpCatalog | null;
  topic: HelpTopic | null;
  selected: string | null;
  loading?: boolean;
  error?: string | null;
  compact?: boolean;
  defaultExpanded?: readonly string[];
  onSelect: (id: string) => void;
  query: string;
  onQueryChange: (value: string) => void;
  urlTransform?: HelpUrlTransform;
}

export function HelpWindowView({
  catalog,
  topic,
  selected,
  loading = false,
  error = null,
  compact,
  defaultExpanded = [],
  onSelect,
  query,
  onQueryChange,
  urlTransform,
}: HelpWindowViewProps) {
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set(defaultExpanded));
  const toggleFolder = useCallback((id: string) => {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  }, []);
  const visibleTopics = useMemo(
    () => filterHelpEntries(catalog?.topics ?? [], query),
    [catalog?.topics, query],
  );
  const emptyState = loading && !error
    ? { icon: <HelpLoadingIcon/>, title: "Loading help" }
    : catalog && catalog.topics.length === 0 && !error
      ? { icon: <HelpEmptyIcon/>, title: "No help topics found" }
      : null;

  return <div className={`help-window ${compact ? "compact" : ""}`}>
    {!compact && <WindowHeader
      title="Help"
      info={catalog?.live ? { primary: "Live documentation" } : undefined}
      search={{ value: query, onSearch: onQueryChange, ariaLabel: "Search Help" }}
    />}
    <div className="help-layout">
      <nav aria-label="Help topics">
        {catalog && <HelpNavigation entries={visibleTopics} expanded={expanded} selected={selected} onSelect={onSelect} onToggle={toggleFolder}/>}
        {catalog && catalog.topics.length > 0 && visibleTopics.length === 0 && <p>No matching help topics.</p>}
      </nav>
      <WindowScrollArea className="help-topic-pane" emptyState={emptyState}><main className="help-content">
        {error && <p className="modal-error">Unable to load help: {error}</p>}
        {catalog?.errors.map((message) => <div className="help-catalog-warning" role="status" key={message}>
          <HelpWarningIcon/>
          <span><strong>Help catalog warning</strong><small>{message}</small></span>
        </div>)}
        {topic && <HelpMarkdown markdown={topic.markdown} topicId={topic.id} urlTransform={urlTransform}/>}
      </main></WindowScrollArea>
    </div>
  </div>;
}
