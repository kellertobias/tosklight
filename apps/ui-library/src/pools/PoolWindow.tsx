import {
	type CSSProperties,
	cloneElement,
	Fragment,
	isValidElement,
	type ReactElement,
	type ReactNode,
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
	columns,
	appearance,
	onSlotClick,
	onSlotPressHold,
	renderSlot,
}: PoolGridProps<SlotId>) {
	const resolvedAppearance = {
		...DEFAULT_POOL_GRID_APPEARANCE,
		...appearance,
	};
	const resolved = fillEmptySlots
		? resolveFixedSlots(slots, slotCount, emptySlot)
		: [...slots];

	return (
		<ButtonGrid
			className={`card-pool pool-window-grid pool-filled-${resolvedAppearance.filledStyle} ${className}`.trim()}
			minimum={minimumCardWidth}
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
