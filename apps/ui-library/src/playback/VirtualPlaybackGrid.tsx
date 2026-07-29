import {
	type CSSProperties,
	type MouseEvent as ReactMouseEvent,
	type PointerEvent as ReactPointerEvent,
	useEffect,
	useLayoutEffect,
	useRef,
	useState,
} from "react";
import { ButtonGrid } from "../grids";
import {
	PoolCard,
	type PoolCardState,
	type ResolvedPoolPresentation,
} from "../pools";

export type VirtualPlaybackBoxAvailability =
	| "assigned"
	| "empty"
	| "unavailable";

export interface VirtualPlaybackExclusionFence {
	top: boolean;
	right: boolean;
	bottom: boolean;
	left: boolean;
}

export interface VirtualPlaybackBoxViewModel {
	slot: number;
	position: number;
	availability: VirtualPlaybackBoxAvailability;
	label?: string;
	icon?: string;
	color?: string;
	backgroundImage?: string;
	actionLabel?: string;
	heldAction?: boolean;
	running?: boolean;
	currentCue?: string;
	configurationTarget?: boolean;
	assignmentTarget?: boolean;
	updateTarget?: boolean;
	exclusionMember?: boolean;
	exclusionZones?: readonly string[];
	exclusionFence?: VirtualPlaybackExclusionFence;
	exclusionSelected?: boolean;
	selectingExclusionZone?: boolean;
	poolPresentation?: ResolvedPoolPresentation;
}

export interface VirtualPlaybackGridCallbacks {
	onPointerDown?(slot: number, position: number): void;
	onPointerUp?(slot: number, position: number): void;
	onPointerCancel?(slot: number, position: number): void;
	onClick?(
		slot: number,
		position: number,
		interaction: { shiftKey: boolean },
	): unknown;
	onAction?(slot: number, position: number): void;
	onActionPress?(slot: number, position: number): void;
	onActionRelease?(slot: number, position: number): void;
	onConfigure?(slot: number, position: number): void;
	onAssign?(slot: number, position: number): void;
	onUpdate?(slot: number, position: number): void;
	onZoneSelection?(slot: number, position: number): void;
}

export interface VirtualPlaybackGridProps {
	page: number;
	rows: number;
	columns: number;
	boxes?: readonly VirtualPlaybackBoxViewModel[];
	boxAt?: (position: number) => VirtualPlaybackBoxViewModel | undefined;
	minimumBoxWidth?: number;
	callbacks?: VirtualPlaybackGridCallbacks;
	className?: string;
}

export function VirtualPlaybackGridView({
	page,
	rows,
	columns,
	boxes = [],
	boxAt,
	minimumBoxWidth = 88,
	callbacks = {},
	className = "",
}: VirtualPlaybackGridProps) {
	const count = Math.max(0, rows * columns);
	const boxesByPosition = new Map(
		boxes.map((box) => [box.position, box] as const),
	);
	const resolveBox = (position: number) =>
		boxesByPosition.get(position) ??
		boxAt?.(position) ?? {
			slot: position + 1,
			position,
			availability: "empty" as const,
		};

	if (count > VIRTUALIZE_AFTER_CELLS)
		return (
			<VirtualizedPlaybackGrid
				page={page}
				rows={rows}
				columns={columns}
				minimumBoxWidth={minimumBoxWidth}
				resolveBox={resolveBox}
				callbacks={callbacks}
				className={className}
			/>
		);

	const resolved = Array.from({ length: count }, (_, position) =>
		resolveBox(position),
	);

	return (
		<ButtonGrid
			className={`virtual-playback-grid ${className}`.trim()}
			minimum={minimumBoxWidth}
			square={false}
			style={{
				gridTemplateColumns: `repeat(${Math.max(columns, 1)}, minmax(var(--grid-cell-min), 1fr))`,
				gridTemplateRows: `repeat(${Math.max(rows, 1)}, minmax(36px, 1fr))`,
				overflow: "auto",
			}}
		>
			{resolved.map((box) => (
				<VirtualPlaybackBox
					key={box.position}
					page={page}
					box={box}
					callbacks={callbacks}
				/>
			))}
		</ButtonGrid>
	);
}

const VIRTUALIZE_AFTER_CELLS = 200;
const VIRTUAL_GRID_GAP = 2;
const VIRTUAL_GRID_OVERSCAN = 2;
const FALLBACK_VISIBLE_TRACKS = 6;

function VirtualizedPlaybackGrid({
	page,
	rows,
	columns,
	minimumBoxWidth,
	resolveBox,
	callbacks,
	className,
}: {
	page: number;
	rows: number;
	columns: number;
	minimumBoxWidth: number;
	resolveBox(position: number): VirtualPlaybackBoxViewModel;
	callbacks: VirtualPlaybackGridCallbacks;
	className: string;
}) {
	const host = useRef<HTMLDivElement>(null);
	const [viewport, setViewport] = useState({
		scrollTop: 0,
		scrollLeft: 0,
		width: 0,
		height: 0,
	});
	const cellWidth = Math.max(64, minimumBoxWidth);
	const rowHeight = cellWidth;
	const columnStride = cellWidth + VIRTUAL_GRID_GAP;
	const rowStride = rowHeight + VIRTUAL_GRID_GAP;
	const viewportWidth =
		viewport.width || columnStride * FALLBACK_VISIBLE_TRACKS;
	const viewportHeight = viewport.height || rowStride * FALLBACK_VISIBLE_TRACKS;
	const firstRow = Math.max(
		0,
		Math.floor(viewport.scrollTop / rowStride) - VIRTUAL_GRID_OVERSCAN,
	);
	const lastRow = Math.min(
		rows,
		Math.ceil((viewport.scrollTop + viewportHeight) / rowStride) +
			VIRTUAL_GRID_OVERSCAN,
	);
	const firstColumn = Math.max(
		0,
		Math.floor(viewport.scrollLeft / columnStride) - VIRTUAL_GRID_OVERSCAN,
	);
	const lastColumn = Math.min(
		columns,
		Math.ceil((viewport.scrollLeft + viewportWidth) / columnStride) +
			VIRTUAL_GRID_OVERSCAN,
	);
	const positions: number[] = [];
	for (let row = firstRow; row < lastRow; row++)
		for (let column = firstColumn; column < lastColumn; column++)
			positions.push(row * columns + column);
	const rangeKey = positions.join(",");
	const [settledPositions, setSettledPositions] = useState(positions);
	const visiblePositions = new Set(positions);
	const retainedPositions = settledPositions.filter(
		(position) => !visiblePositions.has(position),
	);

	// biome-ignore lint/correctness/useExhaustiveDependencies: rangeKey owns semantic array equality while scrolling.
	useEffect(() => {
		const timeout = globalThis.setTimeout(
			() => setSettledPositions(positions),
			250,
		);
		return () => globalThis.clearTimeout(timeout);
	}, [rangeKey]);

	useLayoutEffect(() => {
		const node = host.current;
		if (!node) return;
		const update = () =>
			setViewport((current) => {
				const next = {
					scrollTop: node.scrollTop,
					scrollLeft: node.scrollLeft,
					width: node.clientWidth,
					height: node.clientHeight,
				};
				return sameViewport(current, next) ? current : next;
			});
		update();
		node.addEventListener("scroll", update, { passive: true });
		const observer =
			typeof ResizeObserver === "undefined" ? null : new ResizeObserver(update);
		observer?.observe(node);
		return () => {
			node.removeEventListener("scroll", update);
			observer?.disconnect();
		};
	}, []);

	return (
		<div
			ref={host}
			className={`ui-button-grid compact-grid virtual-playback-grid virtual-playback-grid-virtualized ${className}`.trim()}
			data-logical-cells={rows * columns}
			data-visible-first-row={firstRow + 1}
			data-visible-last-row={lastRow}
			style={
				{
					"--grid-cell-min": `${minimumBoxWidth}px`,
					display: "block",
					height: "100%",
					minHeight: 0,
					overflow: "auto",
					position: "relative",
				} as CSSProperties
			}
		>
			<div
				className="virtual-playback-grid-canvas"
				style={{
					position: "relative",
					width: columns * columnStride - VIRTUAL_GRID_GAP,
					height: rows * rowStride - VIRTUAL_GRID_GAP,
					minWidth: "100%",
					minHeight: "100%",
				}}
			>
				<div
					className="virtual-playback-visible-cells"
					style={{
						position: "absolute",
						left: firstColumn * columnStride,
						top: firstRow * rowStride,
						display: "grid",
						gridTemplateColumns: `repeat(${Math.max(
							lastColumn - firstColumn,
							1,
						)}, ${cellWidth}px)`,
						gridAutoRows: `${rowHeight}px`,
						gap: VIRTUAL_GRID_GAP,
					}}
				>
					{positions.map((position) => {
						const box = resolveBox(position);
						return (
							<VirtualPlaybackBox
								key={box.position}
								page={page}
								box={box}
								callbacks={callbacks}
							/>
						);
					})}
				</div>
				{retainedPositions.map((position) => {
					const box = resolveBox(position);
					return (
						<div
							key={`retained-${box.position}`}
							className="virtual-playback-retained-cell"
							style={{
								position: "absolute",
								left: (position % columns) * columnStride,
								top: Math.floor(position / columns) * rowStride,
								width: cellWidth,
								height: rowHeight,
								display: "grid",
							}}
						>
							<VirtualPlaybackBox page={page} box={box} callbacks={callbacks} />
						</div>
					);
				})}
			</div>
		</div>
	);
}

function sameViewport(
	left: {
		scrollTop: number;
		scrollLeft: number;
		width: number;
		height: number;
	},
	right: {
		scrollTop: number;
		scrollLeft: number;
		width: number;
		height: number;
	},
) {
	return (
		left.scrollTop === right.scrollTop &&
		left.scrollLeft === right.scrollLeft &&
		left.width === right.width &&
		left.height === right.height
	);
}

function VirtualPlaybackBox({
	page,
	box,
	callbacks,
}: {
	page: number;
	box: VirtualPlaybackBoxViewModel;
	callbacks: VirtualPlaybackGridCallbacks;
}) {
	const held = useRef(false);
	const [actionHeld, setActionHeld] = useState(false);
	const suppressClick = useRef(false);
	const releaseCallback = useRef(callbacks.onActionRelease);
	releaseCallback.current = callbacks.onActionRelease;
	const release = (cancelled: boolean) => {
		if (!held.current) return;
		held.current = false;
		setActionHeld(false);
		suppressClick.current = true;
		releaseCallback.current?.(box.slot, box.position);
		if (cancelled) callbacks.onPointerCancel?.(box.slot, box.position);
		else callbacks.onPointerUp?.(box.slot, box.position);
	};
	useEffect(
		() => () => {
			if (held.current) releaseCallback.current?.(box.slot, box.position);
		},
		[box.position, box.slot],
	);

	const unavailable = box.availability === "unavailable";
	const assigned = box.availability === "assigned";
	const vacant = !assigned;
	const states: PoolCardState[] = [
		...(unavailable ? ["disabled" as const] : []),
		...(box.running || actionHeld ? ["active" as const] : []),
		...(box.assignmentTarget ? ["record-target" as const] : []),
		...(box.updateTarget ? ["update-target" as const] : []),
	];
	const secondary = assigned
		? [box.actionLabel, box.currentCue].filter(Boolean).join(" · ")
		: "Unassigned";

	const activate = (event: ReactMouseEvent<HTMLButtonElement>) => {
		if (
			callbacks.onClick?.(box.slot, box.position, {
				shiftKey: event.shiftKey,
			}) === true
		)
			return;
		if (box.updateTarget) callbacks.onUpdate?.(box.slot, box.position);
		else if (box.selectingExclusionZone)
			callbacks.onZoneSelection?.(box.slot, box.position);
		else if (box.configurationTarget)
			callbacks.onConfigure?.(box.slot, box.position);
		else if (box.assignmentTarget) callbacks.onAssign?.(box.slot, box.position);
		else if (assigned && !box.heldAction)
			callbacks.onAction?.(box.slot, box.position);
	};
	const pointerDown = (event: ReactPointerEvent<HTMLButtonElement>) => {
		callbacks.onPointerDown?.(box.slot, box.position);
		if (!assigned || !box.heldAction || held.current) return;
		held.current = true;
		setActionHeld(true);
		event.currentTarget.setPointerCapture?.(event.pointerId);
		callbacks.onActionPress?.(box.slot, box.position);
	};

	return (
		<PoolCard
			aria-label={boxLabel(page, box)}
			aria-pressed={box.exclusionSelected || undefined}
			data-page={page}
			data-virtual-playback-slot={box.slot}
			data-grid-position={box.position}
			data-availability={box.availability}
			data-exclusion-zones={box.exclusionZones?.join(", ") ?? ""}
			data-exclusion-fence={
				box.exclusionFence
					? (["top", "right", "bottom", "left"] as const)
							.filter((side) => box.exclusionFence?.[side])
							.join(" ")
					: ""
			}
			disabled={unavailable}
			className={[
				"virtual-playback-box",
				assigned && "playback-colored",
				vacant && "vacant",
				box.running && "running",
				actionHeld && "held-active",
				box.configurationTarget && "configuration-armed",
				box.assignmentTarget && "assignment-pending",
				box.updateTarget && "update-target",
				box.exclusionMember && "exclusion-member",
				box.exclusionFence?.top && "exclusion-fence-top",
				box.exclusionFence?.right && "exclusion-fence-right",
				box.exclusionFence?.bottom && "exclusion-fence-bottom",
				box.exclusionFence?.left && "exclusion-fence-left",
				box.exclusionSelected && "exclusion-selected",
				box.poolPresentation?.className,
			]
				.filter(Boolean)
				.join(" ")}
			model={{
				number: box.slot,
				primary: box.label ?? (vacant ? "Empty" : ""),
				secondary,
				color: box.color,
				iconColor: box.color,
				icon: box.backgroundImage ? undefined : box.icon,
				image: box.backgroundImage
					? {
							src: box.backgroundImage,
							alt: `${box.label ?? "Playback"} artwork`,
						}
					: undefined,
				status: box.running
					? "Running"
					: actionHeld
						? `${box.actionLabel ?? "Action"} held`
						: box.exclusionSelected
							? "Exclusion selected"
							: undefined,
				workflow: box.configurationTarget
					? "Configure Playback"
					: box.assignmentTarget
						? "Record"
						: box.updateTarget
							? "Update"
							: undefined,
				states,
			}}
			style={{ ...boxStyle(box), ...box.poolPresentation?.style }}
			onPointerDown={pointerDown}
			onContextMenu={(event) => {
				event.preventDefault();
				callbacks.onConfigure?.(box.slot, box.position);
			}}
			onPointerUp={() => {
				if (held.current) release(false);
				else callbacks.onPointerUp?.(box.slot, box.position);
			}}
			onPointerCancel={() => {
				if (held.current) release(true);
				else callbacks.onPointerCancel?.(box.slot, box.position);
			}}
			onClick={(event) => {
				if (suppressClick.current) {
					suppressClick.current = false;
					return;
				}
				activate(event);
			}}
		/>
	);
}

function boxLabel(page: number, box: VirtualPlaybackBoxViewModel) {
	return `Virtual playback page ${page} cell ${box.slot}${
		box.availability === "assigned" ? ` ${box.label ?? ""}` : " empty"
	}`.trim();
}

function boxStyle(box: VirtualPlaybackBoxViewModel) {
	const color = box.color ?? box.poolPresentation?.color;
	return {
		"--playback-color": color ?? "#20c997",
		"--playback-contrast": contrastTextColor(color),
	} as CSSProperties;
}

function contrastTextColor(color: string | undefined) {
	const match = color?.match(/^#([\da-f]{6})$/iu);
	if (!match) return "#ffffff";
	const value = Number.parseInt(match[1], 16);
	const red = value >> 16;
	const green = (value >> 8) & 255;
	const blue = value & 255;
	return red * 0.299 + green * 0.587 + blue * 0.114 > 155
		? "#071014"
		: "#ffffff";
}
