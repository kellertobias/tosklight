import type {
	AttributeDescriptor,
	FixtureChannel,
	FixtureMode,
} from "../../../api/types";
import { Button } from "../../common";

function TouchChannelDragHandle({
	channelId,
	onMove,
}: {
	channelId: string;
	onMove: (sourceId: string, targetId: string) => void;
}) {
	return (
		<span
			className="drag-handle touch-drag-handle"
			aria-hidden="true"
			title="Drag to reorder channels"
			onPointerDown={(event) => {
				if (event.pointerType === "mouse") return;
				event.preventDefault();
				event.currentTarget.setPointerCapture(event.pointerId);
			}}
			onPointerMove={(event) => {
				if (
					event.pointerType === "mouse" ||
					!event.currentTarget.hasPointerCapture(event.pointerId)
				)
					return;
				const target = document
					.elementFromPoint(event.clientX, event.clientY)
					?.closest<HTMLElement>("[data-channel-reorder-id]")
					?.dataset.channelReorderId;
				if (target) onMove(channelId, target);
			}}
			onPointerUp={(event) =>
				event.currentTarget.hasPointerCapture(event.pointerId) &&
				event.currentTarget.releasePointerCapture(event.pointerId)
			}
		>
			⠿
		</span>
	);
}

function ChannelActions({
	channel,
	index,
	channelCount,
	onMove,
	onRemove,
}: {
	channel: FixtureChannel;
	index: number;
	channelCount: number;
	onMove: (direction: -1 | 1) => void;
	onRemove: () => void;
}) {
	return (
		<div className="reorder-actions">
			<Button
				iconOnly
				aria-label={`Move ${channel.attribute} up`}
				disabled={index === 0}
				onClick={() => onMove(-1)}
			>
				▲
			</Button>
			<Button
				iconOnly
				aria-label={`Move ${channel.attribute} down`}
				disabled={index === channelCount - 1}
				onClick={() => onMove(1)}
			>
				▼
			</Button>
			<Button
				iconOnly
				aria-label={`Remove ${channel.attribute}`}
				onClick={onRemove}
			>
				×
			</Button>
		</div>
	);
}

export function ChannelRow({
	mode,
	channel,
	index,
	channelCount,
	primarySlot,
	attributeRegistry,
	onDragStart,
	onDrop,
	onTouchMove,
	onMove,
	onRemove,
	onEdit,
}: {
	mode: FixtureMode;
	channel: FixtureChannel;
	index: number;
	channelCount: number;
	primarySlot: number | undefined;
	attributeRegistry: AttributeDescriptor[];
	onDragStart: () => void;
	onDrop: () => void;
	onTouchMove: (sourceId: string, targetId: string) => void;
	onMove: (direction: -1 | 1) => void;
	onRemove: () => void;
	onEdit: () => void;
}) {
	const head = mode.heads.find((candidate) => candidate.id === channel.head_id);
	const attribute =
		channel.behavior === "static"
			? "Static"
			: (attributeRegistry.find((item) => item.id === channel.attribute)
					?.label ?? channel.attribute);
	return (
		<tr
			className="fixture-channel-row"
			data-channel-reorder-id={channel.id}
			draggable
			onDragStart={onDragStart}
			onDragOver={(event) => event.preventDefault()}
			onDrop={(event) => {
				event.preventDefault();
				onDrop();
			}}
		>
			<td className="channel-primary-slot">
				<TouchChannelDragHandle channelId={channel.id} onMove={onTouchMove} />
				<Button
					aria-label={`Edit ${channel.attribute} channel`}
					onClick={onEdit}
				>
					{primarySlot ?? "!"}
				</Button>
			</td>
			<td>
				<Button onClick={onEdit}>{head?.name || "Missing head"}</Button>
			</td>
			<td>
				<Button onClick={onEdit}>{attribute}</Button>
			</td>
			<td>
				<Button onClick={onEdit}>{channel.resolution.slice(1)} bit</Button>
			</td>
			<td>
				<Button onClick={onEdit}>{channel.default_raw}</Button>
			</td>
			<td>
				<Button onClick={onEdit}>{channel.highlight_raw}</Button>
			</td>
			<td>
				<Button onClick={onEdit}>{channel.functions.length}</Button>
			</td>
			<td>
				<ChannelActions
					channel={channel}
					index={index}
					channelCount={channelCount}
					onMove={onMove}
					onRemove={onRemove}
				/>
			</td>
		</tr>
	);
}
