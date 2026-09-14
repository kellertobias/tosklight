import { useState } from "react";
import type { AttributeDescriptor, FixtureMode } from "../wire";
import { derivePrimarySlots } from "../sheet/fixtureProfileModel";
import { ChannelMappingModal } from "./channelMapping";
import { replaceChannel } from "./channelOperations";
import { moveChannelToSplit } from "./channelSlots";
import { SlotTable } from "./slotTable";
import { SplitAccordions } from "./splits";

/**
 * A mode's DMX slots, split by split.
 *
 * Adding a split or a channel is a title-bar button of the mode editor; control actions have a tab
 * of their own. What is left here is the slots themselves.
 */
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
	const [mappingChannelId, setMappingChannelId] = useState<string | null>(null);
	const primary = derivePrimarySlots(mode);
	const activeSplit = mode.splits.some((split) => split.number === openSplit)
		? openSplit
		: mode.splits[0]?.number;
	const mappingChannel =
		mode.channels.find((channel) => channel.id === mappingChannelId) ?? null;
	const renderSplit = (split: number) => (
		<SlotTable
			mode={mode}
			split={split}
			attributeRegistry={attributeRegistry}
			onChange={onChange}
			onEditMapping={(channel) => setMappingChannelId(channel.id)}
		/>
	);
	return (
		<div className="fixture-channels-editor">
			<SplitAccordions
				mode={mode}
				activeSplit={activeSplit}
				onOpen={onOpenSplit}
				onChange={onChange}
				renderSplit={renderSplit}
			/>
			{primary.errors.length > 0 && (
				<div className="fixture-inline-errors" role="alert">
					{primary.errors.map((error) => (
						<p key={error}>{error}</p>
					))}
				</div>
			)}
			{mappingChannel && (
				<ChannelMappingModal
					mode={mode}
					channel={mappingChannel}
					attributeRegistry={attributeRegistry}
					onChange={(channel) => onChange(replaceChannel(mode, channel))}
					onSplit={(split) => {
						onChange(moveChannelToSplit(mode, mappingChannel, split));
						onOpenSplit(split);
					}}
					onClose={() => setMappingChannelId(null)}
				/>
			)}
		</div>
	);
}
