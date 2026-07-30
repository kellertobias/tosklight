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
	type ButtonVariant,
	ModalTitleBar,
	SearchBar,
	type SearchFeatureProps,
	TitleBarSearchDivider,
} from "../common";
import { ModalLayer } from "../modals/ModalStack";

export interface WindowInfo {
	primary: ReactNode;
	secondary?: ReactNode;
}
export interface WindowAction {
	id: string;
	label: ReactNode;
	onClick: () => void;
	active?: boolean;
	disabled?: boolean;
	ariaLabel?: string;
	onLongPress?: () => void;
	variant?: ButtonVariant;
	className?: string;
}
export interface WindowSettingsTab {
	id: string;
	label: string;
	content: ReactNode;
}
export interface WindowEmptyState {
	title: ReactNode;
	description?: ReactNode;
	icon?: ReactNode;
	action?: ReactNode;
}

const WindowSettingsContext = createContext<(() => void) | null>(null);
export const useWindowSettings = () => useContext(WindowSettingsContext);

function WindowActionButton({ action }: { action: WindowAction }) {
	const holdTimer = useRef<number | null>(null);
	const held = useRef(false);
	const clearHold = () => {
		if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
		holdTimer.current = null;
	};
	return (
		<Button
			aria-label={action.ariaLabel}
			disabled={action.disabled}
			variant={action.variant}
			className={`${action.active ? "active" : ""} ${action.className ?? ""}`.trim()}
			onPointerDown={
				action.onLongPress &&
				(() => {
					held.current = false;
					clearHold();
					holdTimer.current = window.setTimeout(() => {
						held.current = true;
						action.onLongPress?.();
					}, 650);
				})
			}
			onPointerUp={action.onLongPress && clearHold}
			onPointerCancel={action.onLongPress && clearHold}
			onPointerLeave={action.onLongPress && clearHold}
			onClick={() => {
				if (held.current) {
					held.current = false;
					return;
				}
				action.onClick();
			}}
		>
			{action.label}
		</Button>
	);
}

type WindowHeaderProps = {
	title: ReactNode;
	info?: WindowInfo;
	toolbar?: ReactNode;
	actions?: WindowAction[][];
	settings?: boolean;
	onSettings?: (anchor: HTMLElement) => void;
	dragHandleProps?: React.HTMLAttributes<HTMLElement>;
	onTitleClick?: () => void;
	titleActionLabel?: string;
} & SearchFeatureProps;

export function WindowHeader({
	title,
	info,
	toolbar,
	actions = [],
	settings,
	onSettings,
	dragHandleProps,
	onTitleClick,
	titleActionLabel,
	...searchFeature
}: WindowHeaderProps) {
	const { className: dragHandleClassName = "", ...resolvedDragHandleProps } =
		dragHandleProps ?? {};
	const hasFollowingSearchActions = Boolean(
		toolbar || actions.some((group) => group.length > 0) || settings,
	);
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
			{searchFeature.onSearch && (
				<div className="ui-window-header-search">
					<SearchBar
						{...searchFeature.search}
						ariaLabel={
							searchFeature.search.ariaLabel ??
							(typeof title === "string" ? `Search ${title}` : "Search")
						}
						settingsTitle={
							searchFeature.search.settingsTitle ??
							(typeof title === "string"
								? `${title} search settings`
								: "Search settings")
						}
						onChange={searchFeature.onSearch}
					/>
				</div>
			)}
			{searchFeature.onSearch && hasFollowingSearchActions && (
				<TitleBarSearchDivider />
			)}
			{toolbar}
			<div className="ui-window-action-groups">
				{actions
					.filter((group) => group.length)
					.map((group, groupIndex) => (
						<div className="ui-window-action-group" key={groupIndex}>
							{group.map((action) => (
								<WindowActionButton key={action.id} action={action} />
							))}
						</div>
					))}
				{settings && (
					<div className="ui-window-action-group ui-window-settings-action">
						<Button
							aria-label="Settings"
							onClick={(event) => onSettings?.(event.currentTarget)}
						>
							<span aria-hidden="true">⚙</span>
							<span>Settings</span>
						</Button>
					</div>
				)}
			</div>
		</header>
	);
}

export function WindowSettings({
	title = "Settings",
	tabs,
	initialTab,
	onClose,
	modal = true,
	anchor,
}: {
	title?: string;
	tabs: WindowSettingsTab[];
	initialTab?: string;
	onClose: () => void;
	modal?: boolean;
	anchor?: DOMRect | null;
}) {
	const [active, setActive] = useState(initialTab ?? tabs[0]?.id);
	useEffect(() => {
		if (!tabs.some((tab) => tab.id === active)) setActive(tabs[0]?.id);
	}, [tabs, active]);
	const content = (
		<>
			<ModalTitleBar
				title={title}
				tabs={
					tabs.length > 1
						? tabs.map(({ id, label }) => ({ id, label }))
						: undefined
				}
				activeTab={active}
				onTabChange={setActive}
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
	actions?: WindowAction[][];
	settingsTabs?: WindowSettingsTab[];
	settingsTitle?: string;
	navigation?: ReactNode;
	infoSection?: ReactNode;
	bottom?: ReactNode;
	className?: string;
	children: ReactNode;
} & SearchFeatureProps;

export function WindowFrame({
	title,
	info,
	actions,
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
					{...searchFeature}
					actions={actions}
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

function dataTableIndices(start: number, end: number) {
	return Array.from(
		{ length: Math.max(0, end - start) },
		(_, offset) => start + offset,
	);
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
}) {
	const tableWindow = useDataTableWindow({
		activeIndex,
		emptyRows,
		rowCount: rows.length,
		virtualize,
	});
	const {
		host,
		reportedViewport,
		tableFocused,
		total,
		usesViewport,
		viewport,
	} = tableWindow;
	const keyDown = (event: KeyboardEvent, index: number) => {
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
	};
	const template = columns
		.map((column) => column.width ?? "minmax(0,1fr)")
		.join(" ");
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
			style={{ "--table-columns": template } as CSSProperties}
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
					style={{ height: start * 43 }}
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
						onKeyDown={(event) => keyDown(event, index)}
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
					style={{ height: (total - end) * 43 }}
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
}: {
	activeIndex?: number;
	emptyRows: number;
	rowCount: number;
	virtualize: boolean;
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
					Math.max(0, Math.floor((node.clientHeight - 40) / 43) - rowCount),
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
	}, [rowCount]);
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
				Math.floor(Math.max(0, scroller.scrollTop - 40) / 43),
			);
			const visibleRows = Math.max(
				1,
				Math.min(64, Math.ceil(scroller.clientHeight / 43)),
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
	}, [rowCount, total, usesViewport]);
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
