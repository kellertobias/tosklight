import { useState } from "react";
import type {
	AttributeDescriptor,
	FixtureChannel,
	FixtureMode,
} from "../../../api/types";
import { Button } from "../../common";
import { channelSplit } from "../fixtureProfileModel";
import {
	moveChannel,
	moveChannelById,
	removeChannel,
} from "./channelOperations";
import { ChannelRow } from "./channelRow";

export function ChannelSplitTable({
	mode,
	split,
	primarySlots,
	attributeRegistry,
	onChange,
	onEdit,
	onAdd,
}: {
	mode: FixtureMode;
	split: number;
	primarySlots: Map<string, number>;
	attributeRegistry: AttributeDescriptor[];
	onChange: (mode: FixtureMode) => void;
	onEdit: (channel: FixtureChannel) => void;
	onAdd: () => void;
}) {
	const [dragChannel, setDragChannel] = useState<string | null>(null);
	const channels = mode.channels.filter(
		(channel) => channelSplit(mode, channel) === split,
	);
	const moveById = (sourceId: string, targetId: string) => {
		const next = moveChannelById(mode, sourceId, targetId);
		if (next) onChange(next);
	};
	const drop = (targetId: string) => {
		if (dragChannel) moveById(dragChannel, targetId);
		setDragChannel(null);
	};
	return (
		<div className="fixture-channel-split">
			<div className="fixture-channel-table-wrap">
				<table className="fixture-channel-table fixture-channel-summary-table">
					<thead>
						<tr>
							<th>Slot</th>
							<th>Head</th>
							<th>Attribute</th>
							<th>Resolution</th>
							<th>Default</th>
							<th>Highlight</th>
							<th>Functions</th>
							<th>Order</th>
						</tr>
					</thead>
					<tbody>
						{channels.map((channel, index) => (
							<ChannelRow
								key={channel.id}
								mode={mode}
								channel={channel}
								index={index}
								channelCount={channels.length}
								primarySlot={primarySlots.get(channel.id)}
								attributeRegistry={attributeRegistry}
								onDragStart={() => setDragChannel(channel.id)}
								onDrop={() => drop(channel.id)}
								onTouchMove={moveById}
								onMove={(direction) => {
									const next = moveChannel(mode, channel, direction);
									if (next) onChange(next);
								}}
								onRemove={() => onChange(removeChannel(mode, channel.id))}
								onEdit={() => onEdit(channel)}
							/>
						))}
					</tbody>
				</table>
			</div>
			{!channels.length && (
				<p className="empty-editor-message">
					No logical channels are assigned to split {split}.
				</p>
			)}
			<Button onClick={onAdd}>Add channel</Button>
		</div>
	);
}
