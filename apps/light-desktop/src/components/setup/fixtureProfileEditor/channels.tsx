import { useState } from "react";
import type {
	AttributeDescriptor,
	FixtureChannel,
	FixtureMode,
} from "../../../api/types";
import { derivePrimarySlots } from "../fixtureProfileModel";
import { ChannelEditorModal } from "./channelDetails";
import {
	addChannel,
	changeChannelResolution,
	replaceChannel,
} from "./channelOperations";
import { ChannelSplitTable } from "./channelSplitTable";
import { ControlActionsEditor } from "./controlActions";
import { SplitAccordions, SplitManager } from "./splits";

export function ChannelsEditor({
	mode,
	attributeRegistry,
	openSplit,
	onOpenSplit,
	onChange,
}: {
	mode: FixtureMode;
	attributeRegistry: AttributeDescriptor[];
	openSplit: number;
	onOpenSplit: (split: number) => void;
	onChange: (mode: FixtureMode) => void;
}) {
	const [editingChannelId, setEditingChannelId] = useState<string | null>(null);
	const primary = derivePrimarySlots(mode);
	const activeSplit = mode.splits.some((split) => split.number === openSplit)
		? openSplit
		: mode.splits[0]?.number;
	const editedChannel =
		mode.channels.find((channel) => channel.id === editingChannelId) ?? null;
	const setChannel = (channel: FixtureChannel) => {
		onChange(replaceChannel(mode, channel));
		if (channel.split !== openSplit) onOpenSplit(channel.split);
	};
	const addSplit = () => {
		const number = Math.max(0, ...mode.splits.map((split) => split.number)) + 1;
		onChange({ ...mode, splits: [...mode.splits, { number, footprint: 1 }] });
		onOpenSplit(number);
	};
	const renderSplit = (split: number) => (
		<ChannelSplitTable
			mode={mode}
			split={split}
			primarySlots={primary.slots}
			attributeRegistry={attributeRegistry}
			onChange={onChange}
			onEdit={(channel) => setEditingChannelId(channel.id)}
			onAdd={() => onChange(addChannel(mode, split))}
		/>
	);
	return (
		<div className="fixture-channels-editor">
			<SplitManager mode={mode} onAdd={addSplit} onChange={onChange} />
			<SplitAccordions
				mode={mode}
				activeSplit={activeSplit}
				onOpen={onOpenSplit}
				renderSplit={renderSplit}
			/>
			<ControlActionsEditor mode={mode} onChange={onChange} />
			{primary.errors.length > 0 && (
				<div className="fixture-inline-errors" role="alert">
					{primary.errors.map((error) => (
						<p key={error}>{error}</p>
					))}
				</div>
			)}
			{editedChannel && (
				<ChannelEditorModal
					mode={mode}
					channel={editedChannel}
					attributeRegistry={attributeRegistry}
					onChange={setChannel}
					onResolution={(resolution) =>
						onChange(changeChannelResolution(mode, editedChannel, resolution))
					}
					onClose={() => setEditingChannelId(null)}
				/>
			)}
		</div>
	);
}
