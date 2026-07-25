import type { AttributeDescriptor, FixtureChannel } from "../../../api/types";
import { Button, ModalTitleBar } from "../../common";
import { blankFunction, reorder } from "../fixtureProfileModel";
import { ChannelFunctionCard } from "./channelFunctionCard";

export function ChannelFunctionsModal({
	channel,
	attributeRegistry,
	actionIds,
	onChange,
	onClose,
}: {
	channel: FixtureChannel;
	attributeRegistry: AttributeDescriptor[];
	actionIds: Array<{ id: string; name: string }>;
	onChange: (channel: FixtureChannel) => void;
	onClose: () => void;
}) {
	const setFunction = (next: FixtureChannel["functions"][number]) =>
		onChange({
			...channel,
			functions: channel.functions.map((fn) => (fn.id === next.id ? next : fn)),
		});
	return (
		<div
			className="stacked-modal-layer fixture-functions-editor-layer"
			onPointerDown={(event) =>
				event.target === event.currentTarget && onClose()
			}
		>
			<section
				className="nested-modal fixture-functions-editor-modal"
				role="dialog"
				aria-modal="true"
				aria-label="Channel functions"
			>
				<ModalTitleBar
					title="Channel functions"
					actions={
						<Button
							onClick={() =>
								onChange({
									...channel,
									functions: [...channel.functions, blankFunction(channel)],
								})
							}
						>
							Add function
						</Button>
					}
					closeLabel="Close channel functions"
					onClose={onClose}
				/>
				<div className="fixture-functions-editor-body">
					{!channel.functions.length && (
						<p className="empty-editor-message">
							No functions are configured for this channel.
						</p>
					)}
					{channel.functions.map((fn, index) => (
						<ChannelFunctionCard
							key={fn.id}
							fn={fn}
							index={index}
							channel={channel}
							attributeRegistry={attributeRegistry}
							actionIds={actionIds}
							onChange={setFunction}
							onMove={(offset) =>
								onChange({
									...channel,
									functions: reorder(channel.functions, index, index + offset),
								})
							}
							onRemove={() =>
								onChange({
									...channel,
									functions: channel.functions.filter(
										(candidate) => candidate.id !== fn.id,
									),
								})
							}
						/>
					))}
				</div>
			</section>
		</div>
	);
}
