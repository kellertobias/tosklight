import {
	type CSSProperties,
	cloneElement,
	Fragment,
	isValidElement,
	type ReactElement,
	type ReactNode,
	useLayoutEffect,
	useState,
} from "react";
import { ButtonGrid } from "../grids";
import type { TitleActionGroup } from "../common";
import {
	WindowFrame,
	type WindowInfo,
	WindowScrollArea,
	type WindowSettingsTab,
} from "../window-kit";
import { PoolCard, type PoolCardViewModel } from "./PoolCard";

export interface PoolSlotViewModel<SlotId extends string | number> {
	id: SlotId;
	position: number;
	card: PoolCardViewModel;
}

export interface PoolGridProps<SlotId extends string | number> {
	slots: readonly PoolSlotViewModel<SlotId>[];
	slotCount?: number;
	emptySlot(index: number): PoolSlotViewModel<SlotId>;
	fillEmptySlots?: boolean;
	className?: string;
	minimumCardWidth?: number;
	/** Measured sizing; overrides `minimumCardWidth` unless `columns` is fixed. */
	cardSizing?: PoolCardSizing;
	columns?: number;
	appearance?: Partial<PoolGridAppearance>;
	onSlotClick?(id: SlotId, index: number): void;
	onSlotPressHold?(id: SlotId, index: number): void;
	renderSlot?(slot: PoolSlotViewModel<SlotId>, index: number): ReactNode;
}

export interface PoolGridAppearance {
	filledStyle: "tinted" | "outline";
	uncoloredColor: string;
	recordColor: string;
	updateColor: string;
	setColor: string;
}

export const DEFAULT_POOL_GRID_APPEARANCE: Readonly<PoolGridAppearance> = {
	filledStyle: "tinted",
	uncoloredColor: "#65717b",
	recordColor: "#ff4e55",
	updateColor: "#f4b942",
	setColor: "#1bd6ec",
};

export const DEFAULT_POOL_CARD_MINIMUM_WIDTH = 100;

/** Tiles wider than this multiple of the default width take an extra column. */
export const POOL_CARD_STRETCH_LIMIT = 1.5;

export interface PoolCardSizing {
	defaultWidth: number;
	minimumWidth: number;
}

/**
 * Columns fit at the default width and stretch to fill the row. Only when that
 * stretch exceeds the limit does one more column squeeze in, as long as its
 * tiles stay at or above the minimum width.
 */
export function resolvePoolGridColumns(
	available: number,
	gap: number,
	{ defaultWidth, minimumWidth }: PoolCardSizing,
): number {
	const tileWidth = (columns: number) =>
		(available - (columns - 1) * gap) / columns;
	let columns = Math.max(
		1,
		Math.floor((available + gap) / (defaultWidth + gap)),
	);
	while (
		tileWidth(columns) > defaultWidth * POOL_CARD_STRETCH_LIMIT &&
		tileWidth(columns + 1) >= Math.min(minimumWidth, defaultWidth)
	)
		columns += 1;
	return columns;
}

function useMeasuredPoolColumns(sizing: PoolCardSizing | undefined) {
	const [node, setNode] = useState<HTMLDivElement | null>(null);
	const [columns, setColumns] = useState<number>();
	const defaultWidth = sizing?.defaultWidth;
	const minimumWidth = sizing?.minimumWidth;
	useLayoutEffect(() => {
		if (!node || defaultWidth === undefined || minimumWidth === undefined) {
			setColumns(undefined);
			return;
		}
		const measure = () => {
			const style = getComputedStyle(node);
			const available =
				node.clientWidth -
				(Number.parseFloat(style.paddingLeft) || 0) -
				(Number.parseFloat(style.paddingRight) || 0);
			if (!(available > 0)) return;
			setColumns(
				resolvePoolGridColumns(
					available,
					Number.parseFloat(style.columnGap) || 0,
					{ defaultWidth, minimumWidth },
				),
			);
		};
		measure();
		if (typeof ResizeObserver === "undefined") return;
		const observer = new ResizeObserver(measure);
		observer.observe(node);
		return () => observer.disconnect();
	}, [node, defaultWidth, minimumWidth]);
	return [setNode, columns] as const;
}

export interface PoolWindowProps<SlotId extends string | number>
	extends PoolGridProps<SlotId> {
	title: ReactNode;
	info?: WindowInfo;
	groups?: TitleActionGroup[];
	settingsTabs?: WindowSettingsTab[];
}

export function PoolGrid<SlotId extends string | number>({
	slots,
	slotCount,
	emptySlot,
	fillEmptySlots = true,
	className = "",
	minimumCardWidth = DEFAULT_POOL_CARD_MINIMUM_WIDTH,
	cardSizing,
	columns: fixedColumns,
	appearance,
	onSlotClick,
	onSlotPressHold,
	renderSlot,
}: PoolGridProps<SlotId>) {
	const resolvedAppearance = {
		...DEFAULT_POOL_GRID_APPEARANCE,
		...appearance,
	};
	const [measureGrid, measuredColumns] = useMeasuredPoolColumns(
		fixedColumns ? undefined : cardSizing,
	);
	const columns = fixedColumns ?? measuredColumns;
	const resolved = fillEmptySlots
		? resolveFixedSlots(slots, slotCount, emptySlot)
		: [...slots];

	return (
		<ButtonGrid
			ref={measureGrid}
			className={`card-pool pool-window-grid pool-filled-${resolvedAppearance.filledStyle} ${className}`.trim()}
			minimum={cardSizing?.defaultWidth ?? minimumCardWidth}
			style={
				{
					...(columns
						? { gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }
						: {}),
					"--pool-card-uncolored-color": resolvedAppearance.uncoloredColor,
					"--pool-record-color": resolvedAppearance.recordColor,
					"--pool-update-color": resolvedAppearance.updateColor,
					"--pool-set-color": resolvedAppearance.setColor,
				} as CSSProperties
			}
		>
			{resolved.map((slot, index) => (
				<Fragment key={String(slot.id)}>
					{renderSlot ? (
						withSlotIdentity(renderSlot(slot, index), slot.id, index)
					) : (
						<PoolCard
							data-pool-slot-id={String(slot.id)}
							data-pool-position={index}
							model={slot.card}
							onClick={
								onSlotClick ? () => onSlotClick(slot.id, index) : undefined
							}
							onPressHold={
								onSlotPressHold
									? () => onSlotPressHold(slot.id, index)
									: undefined
							}
						/>
					)}
				</Fragment>
			))}
		</ButtonGrid>
	);
}

function resolveFixedSlots<SlotId extends string | number>(
	slots: readonly PoolSlotViewModel<SlotId>[],
	slotCount: number | undefined,
	emptySlot: (index: number) => PoolSlotViewModel<SlotId>,
) {
	const count = Math.max(200, slotCount ?? 200);
	const storedByPosition = new Map(
		slots.map((slot) => [slot.position, slot] as const),
	);
	return Array.from(
		{ length: count },
		(_, index) => storedByPosition.get(index) ?? emptySlot(index),
	);
}

function withSlotIdentity<SlotId extends string | number>(
	node: ReactNode,
	id: SlotId,
	position: number,
) {
	if (!isValidElement(node)) return node;
	return cloneElement(node as ReactElement<Record<string, unknown>>, {
		"data-pool-slot-id": String(id),
		"data-pool-position": position,
	});
}

export function PoolWindow<SlotId extends string | number>({
	title,
	info,
	groups,
	settingsTabs,
	...gridProps
}: PoolWindowProps<SlotId>) {
	return (
		<WindowFrame
			title={title}
			info={info}
			groups={groups}
			settingsTabs={settingsTabs}
			className="pool-window"
		>
			<WindowScrollArea className="pool-window-scroll-area">
				<PoolGrid {...gridProps} />
			</WindowScrollArea>
		</WindowFrame>
	);
}
