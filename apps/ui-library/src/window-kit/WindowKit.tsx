import {
	type CSSProperties,
	createContext,
	type KeyboardEvent,
	type PointerEvent,
	type ReactNode,
	type Ref,
	useContext,
	useEffect,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { createPortal } from "react-dom";
import {
	Button,
	ModalTitleBar,
	type TitleActionGroup,
	TitleChrome,
	type TitleSearch,
} from "../common";
import { ModalLayer } from "../modals/ModalStack";

export interface WindowInfo {
	primary: ReactNode;
	secondary?: ReactNode;
}
export interface WindowSettingsTab {
	id: string;
	label: string;
	content: ReactNode;
}
export interface WindowDropdownItem {
	id: string;
	label: ReactNode;
	onSelect: () => void;
	disabled?: boolean;
}
export interface WindowEmptyState {
	title: ReactNode;
	description?: ReactNode;
	icon?: ReactNode;
	action?: ReactNode;
}

const WindowSettingsContext = createContext<(() => void) | null>(null);
export const useWindowSettings = () => useContext(WindowSettingsContext);

export function WindowDropdown({
	label,
	ariaLabel,
	items,
	className,
}: {
	label: ReactNode;
	ariaLabel: string;
	items: readonly WindowDropdownItem[];
	className?: string;
}) {
	const [anchor, setAnchor] = useState<DOMRect | null>(null);
	useEffect(() => {
		if (!anchor) return;
		const close = (event: globalThis.KeyboardEvent) => {
			if (event.key === "Escape") setAnchor(null);
		};
		window.addEventListener("keydown", close);
		return () => window.removeEventListener("keydown", close);
	}, [anchor]);
	return (
		<>
			<Button
				className={`ui-window-dropdown-trigger ${className ?? ""}`.trim()}
				aria-label={ariaLabel}
				aria-haspopup="menu"
				aria-expanded={Boolean(anchor)}
				onClick={(event) => {
					const nextAnchor = event.currentTarget.getBoundingClientRect();
					setAnchor((current) => (current ? null : nextAnchor));
				}}
			>
				{label}
				<span aria-hidden="true">⌄</span>
			</Button>
			{anchor &&
				createPortal(
					<div
						className="ui-window-dropdown-layer"
						onPointerDown={(event) =>
							event.target === event.currentTarget && setAnchor(null)
						}
					>
						<div
							className="ui-window-dropdown-menu"
							role="menu"
							aria-label={ariaLabel}
							style={{
								top: anchor.bottom + 3,
								left: Math.max(
									3,
									Math.min(anchor.left, window.innerWidth - 230),
								),
							}}
						>
							{items.map((item) => (
								<Button
									key={item.id}
									role="menuitem"
									disabled={item.disabled}
									onClick={() => {
										setAnchor(null);
										item.onSelect();
									}}
								>
									{item.label}
								</Button>
							))}
						</div>
					</div>,
					document.body,
				)}
		</>
	);
}

type WindowHeaderProps = {
	title: ReactNode;
	info?: WindowInfo;
	toolbar?: ReactNode;
	groups?: TitleActionGroup[];
	settings?: boolean;
	onSettings?: (anchor: HTMLElement) => void;
	dragHandleProps?: React.HTMLAttributes<HTMLElement> & {
		"data-tauri-drag-region"?: boolean | "";
	};
	onTitleClick?: () => void;
	titleActionLabel?: string;
	search?: TitleSearch;
};

export function WindowHeader({
	title,
	info,
	toolbar,
	groups = [],
	settings,
	onSettings,
	dragHandleProps,
	onTitleClick,
	titleActionLabel,
	search,
}: WindowHeaderProps) {
	const { className: dragHandleClassName = "", ...resolvedDragHandleProps } =
		dragHandleProps ?? {};
	const resolvedSearch = search
		? {
				...search,
				ariaLabel:
					search.ariaLabel ??
					(typeof title === "string" ? `Search ${title}` : "Search"),
			}
		: undefined;
	return (
		<header
			{...resolvedDragHandleProps}
			className={`ui-window-header ${dragHandleClassName}`.trim()}
		>
			<strong
				className={`ui-window-title ${onTitleClick ? "ui-window-title-action" : ""}`}
				role={onTitleClick ? "button" : undefined}
				tabIndex={onTitleClick ? 0 : undefined}
				aria-label={onTitleClick ? titleActionLabel : undefined}
				onPointerDown={
					onTitleClick ? (event) => event.stopPropagation() : undefined
				}
				onClick={onTitleClick}
				onKeyDown={
					onTitleClick
						? (event) => {
								if (event.key !== "Enter" && event.key !== " ") return;
								event.preventDefault();
								onTitleClick();
							}
						: undefined
				}
			>
				{title}
			</strong>
			{info && (
				<span className="ui-window-info">
					<b>{info.primary}</b>
					{info.secondary != null && <small>{info.secondary}</small>}
				</span>
			)}
			<span className="ui-window-header-spacer" />
			{toolbar}
			<TitleChrome
				groups={groups}
				search={resolvedSearch}
				terminalActions={
					settings
						? [
								{
									id: "settings",
									icon: <span aria-hidden="true">⚙</span>,
									ariaLabel: "Settings",
									disabled: !onSettings,
									className: "ui-window-settings-action",
									onPress: (anchor) => onSettings?.(anchor),
								},
							]
						: []
				}
				className="ui-window-action-groups"
				groupClassName="ui-window-action-group"
				searchClassName="ui-window-header-search"
				terminalClassName="ui-window-action-group ui-window-terminal-actions"
			/>
		</header>
	);
}

export function WindowSettings({
	title = "Settings",
	tabs,
	initialTab,
	activeTab,
	onTabChange,
	onClose,
	modal = true,
	anchor,
}: {
	title?: string;
	tabs: WindowSettingsTab[];
	initialTab?: string;
	/**
	 * Which tab is open, when the caller owns that rather than the panel.
	 *
	 * A panel that keeps its own tab state is right while the tabs are only pages of settings. It
	 * is wrong as soon as a tab *is* a setting — where opening a tab changes what the window is
	 * showing, the window already knows which one is open and two answers would drift apart.
	 */
	activeTab?: string;
	onTabChange?: (id: string) => void;
	onClose: () => void;
	modal?: boolean;
	anchor?: DOMRect | null;
}) {
	const [internal, setInternal] = useState(initialTab ?? tabs[0]?.id);
	const controlled = activeTab !== undefined;
	const active = controlled ? activeTab : internal;
	const setActive = (id: string) => {
		if (!controlled) setInternal(id);
		onTabChange?.(id);
	};
	useEffect(() => {
		if (!controlled && !tabs.some((tab) => tab.id === internal))
			setInternal(tabs[0]?.id);
	}, [controlled, tabs, internal]);
	const content = (
		<>
			<ModalTitleBar
				title={title}
				groups={
					tabs.length > 1
						? [
								{
									id: "settings-tabs",
									kind: "tabs",
									activeId: active ?? tabs[0]?.id ?? "",
									onActiveChange: setActive,
									actions: tabs.map(({ id, label }) => ({ id, label })),
								},
							]
						: undefined
				}
				closeLabel="Close settings"
				onClose={onClose}
			/>
			<div className="ui-window-settings-content">
				{tabs.find((tab) => tab.id === active)?.content}
			</div>
		</>
	);
	if (modal) {
		return (
			<ModalLayer
				ariaLabel={title}
				className="ui-window-settings-layer"
				dialogClassName="ui-window-settings modal"
				onClose={onClose}
			>
				{content}
			</ModalLayer>
		);
	}
	const panel = (
		<section
			className="ui-window-settings popover"
			style={
				anchor
					? {
							top: anchor.bottom + 3,
							right: Math.max(3, window.innerWidth - anchor.right),
						}
					: undefined
			}
			role="dialog"
			aria-label={title}
		>
			{content}
		</section>
	);
	return createPortal(panel, document.body);
}

type WindowFrameProps = {
	title: ReactNode;
	info?: WindowInfo;
	toolbar?: ReactNode;
	groups?: TitleActionGroup[];
	settingsTabs?: WindowSettingsTab[];
	settingsTitle?: string;
	navigation?: ReactNode;
	infoSection?: ReactNode;
	bottom?: ReactNode;
	className?: string;
	children: ReactNode;
	search?: TitleSearch;
};

export function WindowFrame({
	title,
	info,
	toolbar,
	groups,
	settingsTabs = [],
	settingsTitle = "Settings",
	navigation,
	infoSection,
	bottom,
	className = "",
	children,
	...searchFeature
}: WindowFrameProps) {
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
	const [leftOpen, setLeftOpen] = useState(false);
	const [rightOpen, setRightOpen] = useState(false);
	return (
		<WindowSettingsContext.Provider
			value={
				settingsTabs.length
					? () =>
							setSettingsAnchor(new DOMRect(window.innerWidth - 90, 39, 88, 38))
					: null
			}
		>
			<section className={`ui-window ${className}`}>
				<WindowHeader
					title={title}
					info={info}
					toolbar={toolbar}
					{...searchFeature}
					groups={groups}
					settings={settingsTabs.length > 0}
					onSettings={(anchor) =>
						setSettingsAnchor(anchor.getBoundingClientRect())
					}
				/>
				<div
					className={`ui-window-layout ${navigation ? "has-navigation" : ""} ${infoSection ? "has-info-section" : ""}`}
				>
					{navigation && (
						<>
							<Button
								className="ui-window-side-toggle navigation-toggle"
								onClick={() => setLeftOpen(true)}
							>
								☰ Navigation
							</Button>
							<aside
								className={`ui-window-navigation ${leftOpen ? "open" : ""}`}
							>
								<Button
									className="ui-window-side-close"
									onClick={() => setLeftOpen(false)}
								>
									×
								</Button>
								{navigation}
							</aside>
						</>
					)}
					<main className="ui-window-center">{children}</main>
					{infoSection && (
						<>
							<Button
								className="ui-window-side-toggle info-toggle"
								onClick={() => setRightOpen(true)}
							>
								ⓘ Info
							</Button>
							<aside
								className={`ui-window-info-section ${rightOpen ? "open" : ""}`}
							>
								<Button
									className="ui-window-side-close"
									onClick={() => setRightOpen(false)}
								>
									×
								</Button>
								{infoSection}
							</aside>
						</>
					)}
				</div>
				{bottom && <footer className="ui-window-bottom">{bottom}</footer>}
			</section>
			{settingsAnchor && (
				<WindowSettings
					modal={false}
					anchor={settingsAnchor}
					title={settingsTitle}
					tabs={settingsTabs}
					onClose={() => setSettingsAnchor(null)}
				/>
			)}
		</WindowSettingsContext.Provider>
	);
}

export function WindowScrollArea({
	children,
	className = "",
	emptyState,
}: {
	children?: ReactNode;
	className?: string;
	emptyState?: WindowEmptyState | null;
}) {
	const scroller = useRef<HTMLDivElement>(null);
	const [metrics, setMetrics] = useState({ top: 0, size: 1, overflow: false });
	const drag = useRef<{
		pointerId: number;
		startY: number;
		startScroll: number;
	} | null>(null);
	const hasEmptyState = emptyState != null;
	const measure = () => {
		const node = scroller.current;
		if (!node) return;
		const overflow = node.scrollHeight > node.clientHeight + 1;
		setMetrics({
			overflow,
			size: Math.max(0.08, node.clientHeight / Math.max(node.scrollHeight, 1)),
			top: node.scrollTop / Math.max(node.scrollHeight - node.clientHeight, 1),
		});
	};
	useLayoutEffect(() => {
		const node = scroller.current;
		if (!node) return;
		if (typeof ResizeObserver === "undefined") {
			measure();
			return;
		}
		const observer = new ResizeObserver(measure);
		observer.observe(node);
		if (node.firstElementChild) observer.observe(node.firstElementChild);
		measure();
		return () => observer.disconnect();
	}, [hasEmptyState]);
	const move = (event: PointerEvent<HTMLButtonElement>) => {
		const active = drag.current,
			node = scroller.current;
		if (!active || !node || active.pointerId !== event.pointerId) return;
		const track = event.currentTarget.parentElement?.clientHeight ?? 1;
		node.scrollTop =
			active.startScroll +
			((event.clientY - active.startY) /
				Math.max(track * (1 - metrics.size), 1)) *
				(node.scrollHeight - node.clientHeight);
	};
	return (
		<div
			className={`ui-window-scroll-area ${metrics.overflow ? "overflowing" : ""} ${hasEmptyState ? "empty" : ""} ${className}`}
		>
			<div ref={scroller} className="ui-window-scroller" onScroll={measure}>
				{hasEmptyState ? (
					<div className="ui-window-empty-state" role="status">
						{emptyState.icon != null && (
							<span className="icon" aria-hidden="true">
								{emptyState.icon}
							</span>
						)}
						<strong>{emptyState.title}</strong>
						{emptyState.description != null && <p>{emptyState.description}</p>}
						{emptyState.action != null && (
							<div className="action">{emptyState.action}</div>
						)}
					</div>
				) : (
					children
				)}
			</div>
			{metrics.overflow && (
				<div
					className="ui-touch-scrollbar"
					onPointerDown={(event) => {
						if (event.target !== event.currentTarget || !scroller.current)
							return;
						const rect = event.currentTarget.getBoundingClientRect();
						scroller.current.scrollBy({
							top:
								event.clientY < rect.top + metrics.top * rect.height
									? -scroller.current.clientHeight * 0.85
									: scroller.current.clientHeight * 0.85,
							behavior: "smooth",
						});
					}}
				>
					<Button
						aria-label="Scroll window"
						style={
							{
								"--scroll-top": metrics.top,
								"--scroll-size": metrics.size,
							} as CSSProperties
						}
						onPointerDown={(event) => {
							drag.current = {
								pointerId: event.pointerId,
								startY: event.clientY,
								startScroll: scroller.current?.scrollTop ?? 0,
							};
							event.currentTarget.setPointerCapture(event.pointerId);
						}}
						onPointerMove={move}
						onPointerUp={() => {
							drag.current = null;
						}}
						onPointerCancel={() => {
							drag.current = null;
						}}
					/>
				</div>
			)}
		</div>
	);
}

export interface DataTableColumn<T> {
	id: string;
	header: ReactNode;
	width?: string;
	align?: "left" | "center" | "right";
	render: (row: T, index: number) => ReactNode;
}

function dataTableMinimumWidth<T>(columns: readonly DataTableColumn<T>[]) {
	return columns.reduce((total, column) => {
		const width = column.width?.trim();
		if (!width) return total;
		const pixels = width.match(
			/^(?:minmax\(\s*)?(\d+(?:\.\d+)?)px(?:\s*,|\s*$)/u,
		)?.[1];
		return total + (pixels ? Number(pixels) : 0);
	}, 0);
}

function dataTableStyle<T>(
	columns: readonly DataTableColumn<T>[],
	rowHeight: number,
) {
	const minimumWidth = dataTableMinimumWidth(columns);
	return {
		"--table-columns": columns
			.map((column) => column.width ?? "minmax(0,1fr)")
			.join(" "),
		"--table-row-height": `${rowHeight}px`,
		minWidth: minimumWidth > 0 ? `${minimumWidth}px` : undefined,
	} as CSSProperties;
}

function dataTableIndices(start: number, end: number) {
	return Array.from(
		{ length: Math.max(0, end - start) },
		(_, offset) => start + offset,
	);
}

function handleDataTableKeyDown<T>(
	event: KeyboardEvent,
	index: number,
	total: number,
	rows: readonly T[],
	onActiveIndexChange?: (index: number) => void,
	onActivate?: (row: T, index: number) => void,
) {
	if (event.key === "ArrowDown" || event.key === "ArrowUp") {
		event.preventDefault();
		onActiveIndexChange?.(
			Math.max(
				0,
				Math.min(total - 1, index + (event.key === "ArrowDown" ? 1 : -1)),
			),
		);
	}
	if ((event.key === "Enter" || event.key === " ") && rows[index]) {
		event.preventDefault();
		onActivate?.(rows[index], index);
	}
}

export function DataTable<T>({
	columns,
	rows,
	rowKey,
	selected,
	rowClassName,
	rowDataAttributes,
	activeIndex,
	onActiveIndexChange,
	onActivate,
	onVisibleRowsChange,
	emptyRows = 0,
	className = "",
	virtualize = false,
	rowHeight = 43,
}: {
	columns: DataTableColumn<T>[];
	rows: T[];
	rowKey: (row: T, index: number) => string;
	selected?: (row: T) => boolean;
	rowClassName?: (row: T, index: number) => string;
	rowDataAttributes?: (
		row: T,
		index: number,
	) => Record<string, string | undefined>;
	activeIndex?: number;
	onActiveIndexChange?: (index: number) => void;
	onActivate?: (row: T, index: number) => void;
	onVisibleRowsChange?: (rows: readonly T[]) => void;
	emptyRows?: number;
	className?: string;
	virtualize?: boolean;
	rowHeight?: number;
}) {
	const tableWindow = useDataTableWindow({
		activeIndex,
		emptyRows,
		rowCount: rows.length,
		virtualize,
		rowHeight,
	});
	const {
		host,
		reportedViewport,
		tableFocused,
		total,
		usesViewport,
		viewport,
	} = tableWindow;
	const start = usesViewport ? viewport.start : 0;
	const end = usesViewport ? viewport.end : total;
	const visibleIndices = dataTableIndices(start, end);
	const visibleRows = useMemo(
		() => rows.slice(reportedViewport.start, reportedViewport.end),
		[reportedViewport.end, reportedViewport.start, rows],
	);
	useEffect(() => {
		onVisibleRowsChange?.(visibleRows);
	}, [onVisibleRowsChange, visibleRows]);
	return (
		<div
			ref={host}
			className={`ui-data-table ${usesViewport ? "virtualized" : ""} ${className}`}
			role="table"
			aria-rowcount={total + 1}
			onFocusCapture={() => {
				tableFocused.current = true;
			}}
			onBlurCapture={(event) => {
				if (!event.currentTarget.contains(event.relatedTarget as Node | null))
					tableFocused.current = false;
			}}
			style={dataTableStyle(columns, rowHeight)}
		>
			<div className="ui-data-table-row header" role="row" aria-rowindex={1}>
				{columns.map((column) => (
					<span
						role="columnheader"
						className={column.align ?? "left"}
						key={column.id}
					>
						{column.header}
					</span>
				))}
			</div>
			{usesViewport && start > 0 && (
				<div
					className="ui-data-table-spacer"
					style={{ height: start * rowHeight }}
					aria-hidden="true"
				/>
			)}
			{visibleIndices.map((index) => {
				const row = rows[index];
				const isEmpty = row == null;
				const dataAttributes = row
					? rowDataAttributes?.(row, index)
					: undefined;
				return (
					<div
						{...dataAttributes}
						key={row ? rowKey(row, index) : `empty-${index}`}
						role="row"
						aria-rowindex={index + 2}
						data-table-index={index}
						tabIndex={index === (activeIndex ?? 0) ? 0 : -1}
						className={`ui-data-table-row ${isEmpty ? "empty" : ""} ${row && selected?.(row) ? "selected" : ""} ${row ? (rowClassName?.(row, index) ?? "") : ""} ${index === activeIndex ? "active" : ""}`}
						onClick={() => {
							onActiveIndexChange?.(index);
							if (row) onActivate?.(row, index);
						}}
						onKeyDown={(event) =>
							handleDataTableKeyDown(
								event,
								index,
								total,
								rows,
								onActiveIndexChange,
								onActivate,
							)
						}
					>
						{columns.map((column) => (
							<span
								role="cell"
								className={column.align ?? "left"}
								key={column.id}
							>
								{row ? column.render(row, index) : null}
							</span>
						))}
					</div>
				);
			})}
			{usesViewport && end < total && (
				<div
					className="ui-data-table-spacer"
					style={{ height: (total - end) * rowHeight }}
					aria-hidden="true"
				/>
			)}
		</div>
	);
}

function useDataTableWindow({
	activeIndex,
	emptyRows,
	rowCount,
	virtualize,
	rowHeight,
}: {
	activeIndex?: number;
	emptyRows: number;
	rowCount: number;
	virtualize: boolean;
	rowHeight: number;
}) {
	const host = useRef<HTMLDivElement>(null);
	const tableFocused = useRef(false);
	const [fillRows, setFillRows] = useState(emptyRows);
	const [viewport, setViewport] = useState({ start: 0, end: 40 });
	const [reportedViewport, setReportedViewport] = useState({
		start: 0,
		end: 0,
	});
	useLayoutEffect(() => {
		const node = host.current;
		if (!node) return;
		const measure = () => {
			if (node.clientHeight > 40)
				setFillRows(
					Math.max(
						0,
						Math.floor((node.clientHeight - 40) / rowHeight) - rowCount,
					),
				);
		};
		if (typeof ResizeObserver === "undefined") {
			measure();
			return;
		}
		const observer = new ResizeObserver(measure);
		observer.observe(node);
		measure();
		return () => observer.disconnect();
	}, [rowCount, rowHeight]);
	const total = rowCount + fillRows;
	const usesViewport = virtualize && total > 100;
	useLayoutEffect(() => {
		if (!usesViewport) setViewport({ start: 0, end: total });
		const node = host.current;
		const scroller = node?.closest<HTMLElement>(".ui-window-scroller");
		if (!node || !scroller) return;
		const measureViewport = () => {
			const visibleStart = Math.max(
				0,
				Math.floor(Math.max(0, scroller.scrollTop - 40) / rowHeight),
			);
			const visibleRows = Math.max(
				1,
				Math.min(64, Math.ceil(scroller.clientHeight / rowHeight)),
			);
			const visibleEnd = Math.min(rowCount, visibleStart + visibleRows + 1);
			setReportedViewport((current) =>
				current.start === visibleStart && current.end === visibleEnd
					? current
					: { start: visibleStart, end: visibleEnd },
			);
			if (!usesViewport) return;
			const start = Math.max(0, visibleStart - 12);
			const end = Math.min(total, visibleStart + visibleRows + 12);
			setViewport((current) =>
				current.start === start && current.end === end
					? current
					: { start, end },
			);
		};
		const observer =
			typeof ResizeObserver === "undefined"
				? null
				: new ResizeObserver(measureViewport);
		scroller.addEventListener("scroll", measureViewport, { passive: true });
		observer?.observe(scroller);
		measureViewport();
		return () => {
			scroller.removeEventListener("scroll", measureViewport);
			observer?.disconnect();
		};
	}, [rowCount, rowHeight, total, usesViewport]);
	// biome-ignore lint/correctness/useExhaustiveDependencies: viewport changes mount the virtualized active row before it can be scrolled into view.
	useLayoutEffect(() => {
		if (!usesViewport || !tableFocused.current || activeIndex == null) return;
		const row = host.current?.querySelector<HTMLElement>(
			`[data-table-index="${activeIndex}"]`,
		);
		row?.scrollIntoView({ block: "nearest" });
	}, [activeIndex, usesViewport, viewport]);
	return {
		host,
		reportedViewport,
		tableFocused,
		total,
		usesViewport,
		viewport,
	};
}

export function ButtonGrid({
	children,
	minimum = 88,
	square = true,
	className = "",
	style,
	ref,
}: {
	children: ReactNode;
	minimum?: number;
	square?: boolean;
	className?: string;
	style?: CSSProperties;
	ref?: Ref<HTMLDivElement>;
}) {
	const host = useRef<HTMLDivElement>(null);
	const setHost = (node: HTMLDivElement | null) => {
		host.current = node;
		if (typeof ref === "function") ref(node);
		else if (ref) (ref as { current: HTMLDivElement | null }).current = node;
	};
	useLayoutEffect(() => {
		const node = host.current;
		if (!node || !square) {
			node?.style.removeProperty("--grid-row-size");
			return;
		}
		let lastWidth = 0;
		let disposed = false;
		const measure = () => {
			if (disposed) return;
			const firstButton = node.firstElementChild;
			const width =
				firstButton instanceof HTMLElement
					? Number.parseFloat(getComputedStyle(firstButton).width)
					: 0;
			if (!Number.isFinite(width) || width <= 0) return;
			if (Math.abs(width - lastWidth) < 0.25) return;
			lastWidth = width;
			const rowSize = `${Math.round(width * 1000) / 1000}px`;
			if (node.style.getPropertyValue("--grid-row-size") !== rowSize)
				node.style.setProperty("--grid-row-size", rowSize);
		};
		measure();
		void document.fonts?.ready.then(measure);
		if (typeof ResizeObserver === "undefined")
			return () => {
				disposed = true;
			};
		const observer = new ResizeObserver(measure);
		const observeFirst = () => {
			measure();
			if (node.firstElementChild) observer.observe(node.firstElementChild);
		};
		observer.observe(node);
		observeFirst();
		const mutations =
			typeof MutationObserver === "undefined"
				? null
				: new MutationObserver(observeFirst);
		mutations?.observe(node, { childList: true });
		return () => {
			disposed = true;
			mutations?.disconnect();
			observer.disconnect();
		};
	}, [square]);
	return (
		<div
			ref={setHost}
			className={`ui-button-grid ${square ? "square-grid" : "compact-grid"} ${className}`}
			style={{ "--grid-cell-min": `${minimum}px`, ...style } as CSSProperties}
		>
			{children}
		</div>
	);
}
export function GridButton({
	number,
	primary,
	secondary,
	icon,
	state = "filled",
	onClick,
}: {
	number: ReactNode;
	primary?: ReactNode;
	secondary?: ReactNode;
	icon?: ReactNode;
	state?:
		| "empty"
		| "filled"
		| "disabled"
		| "active"
		| "selected"
		| "store-target";
	onClick?: () => void;
}) {
	return (
		<Button
			disabled={state === "disabled"}
			className={`ui-grid-button ${state}`}
			onClick={onClick}
		>
			<span className="number">{number}</span>
			{primary != null && <b>{primary}</b>}
			{secondary != null && <small>{secondary}</small>}
			{icon != null && <span className="icon">{icon}</span>}
		</Button>
	);
}
export function FaderView({
	rows,
	children,
	className = "",
	style,
}: {
	rows: number;
	children: ReactNode;
	className?: string;
	style?: CSSProperties;
}) {
	return (
		<div
			className={`ui-fader-view ${className}`}
			style={{ "--fader-rows": rows, ...style } as CSSProperties}
		>
			{children}
		</div>
	);
}
