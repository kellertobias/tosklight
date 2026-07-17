import {
  createContext,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent,
  type ReactNode,
  type Ref,
} from "react";
import { createPortal } from "react-dom";
import { Button, ModalTitleBar } from "../common";

export interface WindowInfo { primary: ReactNode; secondary?: ReactNode }
export interface WindowAction { id: string; label: ReactNode; onClick: () => void; active?: boolean; disabled?: boolean; ariaLabel?: string }
export interface WindowSettingsTab { id: string; label: string; content: ReactNode }
export interface WindowEmptyState { title: ReactNode; description?: ReactNode; icon?: ReactNode; action?: ReactNode }

const WindowSettingsContext = createContext<(() => void) | null>(null);
export const useWindowSettings = () => useContext(WindowSettingsContext);

export function WindowHeader({ title, info, search, toolbar, actions = [], settings, onSettings, dragHandleProps }: {
  title: ReactNode;
  info?: WindowInfo;
  search?: ReactNode;
  toolbar?: ReactNode;
  actions?: WindowAction[][];
  settings?: boolean;
  onSettings?: (anchor: HTMLElement) => void;
  dragHandleProps?: React.HTMLAttributes<HTMLElement>;
}) {
  return <header className="ui-window-header" {...dragHandleProps}>
    <strong className="ui-window-title">{title}</strong>
    {info && <span className="ui-window-info"><b>{info.primary}</b>{info.secondary != null && <small>{info.secondary}</small>}</span>}
    <span className="ui-window-header-spacer" />
    {search && <div className="ui-window-header-search">{search}</div>}
    {toolbar}
    <div className="ui-window-action-groups">
      {actions.filter((group) => group.length).map((group, groupIndex) => <div className="ui-window-action-group" key={groupIndex}>
        {group.map((action) => <Button key={action.id} aria-label={action.ariaLabel} disabled={action.disabled} className={action.active ? "active" : ""} onClick={action.onClick}>{action.label}</Button>)}
      </div>)}
      {settings && <div className="ui-window-action-group ui-window-settings-action"><Button aria-label="Settings" onClick={(event) => onSettings?.(event.currentTarget)}><span aria-hidden="true">⚙</span><span>Settings</span></Button></div>}
    </div>
  </header>;
}

export function WindowSettings({ title = "Settings", tabs, initialTab, onClose, modal = true, anchor }: { title?: string; tabs: WindowSettingsTab[]; initialTab?: string; onClose: () => void; modal?: boolean; anchor?: DOMRect | null }) {
  const [active, setActive] = useState(initialTab ?? tabs[0]?.id);
  useEffect(() => { if (!tabs.some((tab) => tab.id === active)) setActive(tabs[0]?.id); }, [tabs, active]);
  const panel = <section className={`ui-window-settings ${modal ? "modal" : "popover"}`} style={!modal && anchor ? { top: anchor.bottom + 3, right: Math.max(3, window.innerWidth - anchor.right) } : undefined} role="dialog" aria-modal={modal || undefined} aria-label={title}>
      <ModalTitleBar title={title} tabs={tabs.length > 1 ? tabs.map(({ id, label }) => ({ id, label })) : undefined} activeTab={active} onTabChange={setActive} closeLabel="Close settings" onClose={onClose}/>
      <div className="ui-window-settings-content">{tabs.find((tab) => tab.id === active)?.content}</div>
    </section>;
  return createPortal(modal ? <div className="ui-window-settings-backdrop" onPointerDown={(event) => event.target === event.currentTarget && onClose()}>{panel}</div> : panel, document.body);
}

export function WindowFrame({ title, info, search, actions, settingsTabs = [], settingsTitle = "Settings", navigation, infoSection, bottom, className = "", children }: {
  title: ReactNode;
  info?: WindowInfo;
  search?: ReactNode;
  actions?: WindowAction[][];
  settingsTabs?: WindowSettingsTab[];
  settingsTitle?: string;
  navigation?: ReactNode;
  infoSection?: ReactNode;
  bottom?: ReactNode;
  className?: string;
  children: ReactNode;
}) {
  const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
  const [leftOpen, setLeftOpen] = useState(false);
  const [rightOpen, setRightOpen] = useState(false);
  return <WindowSettingsContext.Provider value={settingsTabs.length ? () => setSettingsAnchor(new DOMRect(window.innerWidth - 90, 39, 88, 38)) : null}>
    <section className={`ui-window ${className}`}>
      <WindowHeader title={title} info={info} search={search} actions={actions} settings={settingsTabs.length > 0} onSettings={(anchor) => setSettingsAnchor(anchor.getBoundingClientRect())} />
      <div className={`ui-window-layout ${navigation ? "has-navigation" : ""} ${infoSection ? "has-info-section" : ""}`}>
        {navigation && <><Button className="ui-window-side-toggle navigation-toggle" onClick={() => setLeftOpen(true)}>☰ Navigation</Button><aside className={`ui-window-navigation ${leftOpen ? "open" : ""}`}><Button className="ui-window-side-close" onClick={() => setLeftOpen(false)}>×</Button>{navigation}</aside></>}
        <main className="ui-window-center">{children}</main>
        {infoSection && <><Button className="ui-window-side-toggle info-toggle" onClick={() => setRightOpen(true)}>ⓘ Info</Button><aside className={`ui-window-info-section ${rightOpen ? "open" : ""}`}><Button className="ui-window-side-close" onClick={() => setRightOpen(false)}>×</Button>{infoSection}</aside></>}
      </div>
      {bottom && <footer className="ui-window-bottom">{bottom}</footer>}
    </section>
    {settingsAnchor && <WindowSettings modal={false} anchor={settingsAnchor} title={settingsTitle} tabs={settingsTabs} onClose={() => setSettingsAnchor(null)} />}
  </WindowSettingsContext.Provider>;
}

export function WindowScrollArea({ children, className = "", emptyState }: { children?: ReactNode; className?: string; emptyState?: WindowEmptyState | null }) {
  const scroller = useRef<HTMLDivElement>(null);
  const [metrics, setMetrics] = useState({ top: 0, size: 1, overflow: false });
  const drag = useRef<{ pointerId: number; startY: number; startScroll: number } | null>(null);
  const hasEmptyState = emptyState != null;
  const measure = () => {
    const node = scroller.current;
    if (!node) return;
    const overflow = node.scrollHeight > node.clientHeight + 1;
    setMetrics({ overflow, size: Math.max(.08, node.clientHeight / Math.max(node.scrollHeight, 1)), top: node.scrollTop / Math.max(node.scrollHeight - node.clientHeight, 1) });
  };
  useLayoutEffect(() => {
    const node = scroller.current;
    if (!node) return;
    if (typeof ResizeObserver === "undefined") { measure(); return; }
    const observer = new ResizeObserver(measure);
    observer.observe(node);
    if (node.firstElementChild) observer.observe(node.firstElementChild);
    measure();
    return () => observer.disconnect();
  }, [hasEmptyState]);
  const move = (event: PointerEvent<HTMLButtonElement>) => {
    const active = drag.current, node = scroller.current;
    if (!active || !node || active.pointerId !== event.pointerId) return;
    const track = event.currentTarget.parentElement?.clientHeight ?? 1;
    node.scrollTop = active.startScroll + (event.clientY - active.startY) / Math.max(track * (1 - metrics.size), 1) * (node.scrollHeight - node.clientHeight);
  };
  return <div className={`ui-window-scroll-area ${metrics.overflow ? "overflowing" : ""} ${hasEmptyState ? "empty" : ""} ${className}`}>
    <div ref={scroller} className="ui-window-scroller" onScroll={measure}>{hasEmptyState ? <div className="ui-window-empty-state" role="status">
      {emptyState.icon != null && <span className="icon" aria-hidden="true">{emptyState.icon}</span>}
      <strong>{emptyState.title}</strong>
      {emptyState.description != null && <p>{emptyState.description}</p>}
      {emptyState.action != null && <div className="action">{emptyState.action}</div>}
    </div> : children}</div>
    {metrics.overflow && <div className="ui-touch-scrollbar" onPointerDown={(event) => {
      if (event.target !== event.currentTarget || !scroller.current) return;
      const rect = event.currentTarget.getBoundingClientRect();
      scroller.current.scrollBy({ top: event.clientY < rect.top + metrics.top * rect.height ? -scroller.current.clientHeight * .85 : scroller.current.clientHeight * .85, behavior: "smooth" });
    }}><Button aria-label="Scroll window" style={{ "--scroll-top": metrics.top, "--scroll-size": metrics.size } as CSSProperties} onPointerDown={(event) => { drag.current = { pointerId: event.pointerId, startY: event.clientY, startScroll: scroller.current?.scrollTop ?? 0 }; event.currentTarget.setPointerCapture(event.pointerId); }} onPointerMove={move} onPointerUp={() => { drag.current = null; }} onPointerCancel={() => { drag.current = null; }} /></div>}
  </div>;
}

export interface DataTableColumn<T> { id: string; header: ReactNode; width?: string; align?: "left" | "center" | "right"; render: (row: T, index: number) => ReactNode }
export function DataTable<T>({ columns, rows, rowKey, selected, rowClassName, rowDataAttributes, activeIndex, onActiveIndexChange, onActivate, emptyRows = 0, className = "" }: {
  columns: DataTableColumn<T>[]; rows: T[]; rowKey: (row: T, index: number) => string; selected?: (row: T) => boolean; rowClassName?: (row: T, index: number) => string; rowDataAttributes?: (row: T, index: number) => Record<string, string | undefined>; activeIndex?: number; onActiveIndexChange?: (index: number) => void; onActivate?: (row: T, index: number) => void; emptyRows?: number; className?: string;
}) {
  const host = useRef<HTMLDivElement>(null);
  const [fillRows, setFillRows] = useState(emptyRows);
  useLayoutEffect(() => { const node = host.current; if (!node) return; const measure = () => { if (node.clientHeight > 40) setFillRows(Math.max(0, Math.floor((node.clientHeight - 40) / 43) - rows.length)); }; if (typeof ResizeObserver === "undefined") { measure(); return; } const observer = new ResizeObserver(measure); observer.observe(node); measure(); return () => observer.disconnect(); }, [rows.length]);
  const total = rows.length + fillRows;
  const keyDown = (event: KeyboardEvent, index: number) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); onActiveIndexChange?.(Math.max(0, Math.min(total - 1, index + (event.key === "ArrowDown" ? 1 : -1)))); }
    if ((event.key === "Enter" || event.key === " ") && rows[index]) { event.preventDefault(); onActivate?.(rows[index], index); }
  };
  const template = columns.map((column) => column.width ?? "minmax(0,1fr)").join(" ");
  return <div ref={host} className={`ui-data-table ${className}`} role="table" style={{ "--table-columns": template } as CSSProperties}>
    <div className="ui-data-table-row header" role="row">{columns.map((column) => <span role="columnheader" className={column.align ?? "left"} key={column.id}>{column.header}</span>)}</div>
    {Array.from({ length: total }, (_, index) => { const row = rows[index]; const isEmpty = row == null; const dataAttributes = row ? rowDataAttributes?.(row, index) : undefined; return <div {...dataAttributes} key={row ? rowKey(row, index) : `empty-${index}`} role="row" tabIndex={index === (activeIndex ?? 0) ? 0 : -1} className={`ui-data-table-row ${isEmpty ? "empty" : ""} ${row && selected?.(row) ? "selected" : ""} ${row ? rowClassName?.(row, index) ?? "" : ""} ${index === activeIndex ? "active" : ""}`} onFocus={() => onActiveIndexChange?.(index)} onClick={() => { onActiveIndexChange?.(index); if (row) onActivate?.(row, index); }} onKeyDown={(event) => keyDown(event, index)}>{columns.map((column) => <span role="cell" className={column.align ?? "left"} key={column.id}>{row ? column.render(row, index) : null}</span>)}</div>; })}
  </div>;
}

export function ButtonGrid({ children, minimum = 88, className = "", style, ref }: { children: ReactNode; minimum?: number; className?: string; style?: CSSProperties; ref?: Ref<HTMLDivElement> }) {
  const host = useRef<HTMLDivElement>(null);
  const setHost = (node: HTMLDivElement | null) => {
    host.current = node;
    if (typeof ref === "function") ref(node);
    else if (ref) (ref as { current: HTMLDivElement | null }).current = node;
  };
  useLayoutEffect(() => {
    const node = host.current;
    if (!node) return;
    const measure = () => {
      const firstButton = node.firstElementChild;
      const width = firstButton instanceof HTMLElement ? Number.parseFloat(getComputedStyle(firstButton).width) : 0;
      if (!Number.isFinite(width) || width <= 0) return;
      const rowSize = `${Math.round(width * 1000) / 1000}px`;
      if (node.style.getPropertyValue("--grid-row-size") !== rowSize) node.style.setProperty("--grid-row-size", rowSize);
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(node);
    return () => observer.disconnect();
  }, [children, minimum]);
  return <div ref={setHost} className={`ui-button-grid ${className}`} style={{ "--grid-cell-min": `${minimum}px`, ...style } as CSSProperties}>{children}</div>;
}
export function GridButton({ number, primary, secondary, icon, state = "filled", onClick }: { number: ReactNode; primary?: ReactNode; secondary?: ReactNode; icon?: ReactNode; state?: "empty" | "filled" | "disabled" | "active" | "selected" | "store-target"; onClick?: () => void }) {
  return <Button disabled={state === "disabled"} className={`ui-grid-button ${state}`} onClick={onClick}><span className="number">{number}</span>{primary != null && <b>{primary}</b>}{secondary != null && <small>{secondary}</small>}{icon != null && <span className="icon">{icon}</span>}</Button>;
}
export function FaderView({ rows, children, className = "" }: { rows: number; children: ReactNode; className?: string }) {
  return <div className={`ui-fader-view ${className}`} style={{ "--fader-rows": rows } as CSSProperties}>{children}</div>;
}
