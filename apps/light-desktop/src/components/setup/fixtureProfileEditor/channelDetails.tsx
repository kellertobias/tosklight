import { useState } from "react";
import type {
	AttributeDescriptor,
	ChannelResolution,
	FixtureChannel,
	FixtureMode,
} from "../../../api/types";
import { Button, ModalTitleBar } from "../../common";
import { ChannelCoreFields, ChannelFields } from "./channelFields";
import { ChannelFunctionsModal } from "./channelFunctions";

export function ChannelEditorModal({
	mode,
	channel,
	attributeRegistry,
	onChange,
	onResolution,
	onClose,
}: {
	mode: FixtureMode;
	channel: FixtureChannel;
	attributeRegistry: AttributeDescriptor[];
	onChange: (channel: FixtureChannel) => void;
	onResolution: (resolution: ChannelResolution) => void;
	onClose: () => void;
}) {
	const [functionsOpen, setFunctionsOpen] = useState(false);
	return (
		<div
			className="stacked-modal-layer fixture-channel-editor-layer"
			onPointerDown={(event) =>
				event.target === event.currentTarget && onClose()
			}
		>
			<section
				className="nested-modal fixture-channel-editor-modal"
				role="dialog"
				aria-modal="true"
				aria-label={`Edit ${channel.attribute} channel`}
			>
				<ModalTitleBar
					title="Edit channel"
					actions={
						<Button onClick={() => setFunctionsOpen(true)}>
							Channel functions ({channel.functions.length})
						</Button>
					}
					closeLabel="Close channel editor"
					onClose={onClose}
				/>
				<div className="fixture-channel-editor-body">
					<ChannelCoreFields
						mode={mode}
						channel={channel}
						attributeRegistry={attributeRegistry}
						onChange={onChange}
						onResolution={onResolution}
					/>
					<ChannelFields channel={channel} onChange={onChange} />
				</div>
				{functionsOpen && (
					<ChannelFunctionsModal
						channel={channel}
						attributeRegistry={attributeRegistry}
						actionIds={mode.control_actions}
						onChange={onChange}
						onClose={() => setFunctionsOpen(false)}
					/>
				)}
			</section>
		</div>
	);
}
