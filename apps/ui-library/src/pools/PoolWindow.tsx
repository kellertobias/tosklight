import {
	cloneElement,
	Fragment,
	isValidElement,
	type ReactElement,
	type ReactNode,
} from "react";
import { ButtonGrid } from "../grids";
import {
	type WindowAction,
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
	className?: string;
	minimumCardWidth?: number;
	onSlotClick?(id: SlotId, index: number): void;
	onSlotPressHold?(id: SlotId, index: number): void;
	renderSlot?(slot: PoolSlotViewModel<SlotId>, index: number): ReactNode;
}

export interface PoolWindowProps<SlotId extends string | number>
	extends PoolGridProps<SlotId> {
	title: ReactNode;
	info?: WindowInfo;
	actions?: WindowAction[][];
	settingsTabs?: WindowSettingsTab[];
}

export function PoolGrid<SlotId extends string | number>({
	slots,
	slotCount,
	emptySlot,
	className = "",
	minimumCardWidth = 88,
	onSlotClick,
	onSlotPressHold,
	renderSlot,
}: PoolGridProps<SlotId>) {
	const count = Math.max(200, slotCount ?? 200);
	const storedByPosition = new Map(
		slots.map((slot) => [slot.position, slot] as const),
	);
	const resolved = Array.from(
		{ length: count },
		(_, index) => storedByPosition.get(index) ?? emptySlot(index),
	);

	return (
		<ButtonGrid
			className={`card-pool pool-window-grid ${className}`.trim()}
			minimum={minimumCardWidth}
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
	actions,
	settingsTabs,
	...gridProps
}: PoolWindowProps<SlotId>) {
	return (
		<WindowFrame
			title={title}
			info={info}
			actions={actions}
			settingsTabs={settingsTabs}
			className="pool-window"
		>
			<WindowScrollArea className="pool-window-scroll-area">
				<PoolGrid {...gridProps} />
			</WindowScrollArea>
		</WindowFrame>
	);
}
