import type { FixtureMode } from "../wire";
import { Button } from "@tosklight/ui";
import { useState } from "react";
import { reorder } from "../sheet/fixtureProfileModel";
import { ConfirmDialog } from "./dialogs";
import { TrashIcon } from "./trashIcon";

function channelCount(mode: FixtureMode, split: number) {
	const count = mode.channels.filter((channel) => channel.split === split).length;
	return `${count} ${count === 1 ? "channel" : "channels"}`;
}

/**
 * Each split as one line — its name and how many channels it holds, which opens it — with its own
 * move and remove at the end of the line.
 *
 * A split's footprint is what its slots take up, so it is kept by the slots rather than typed. A
 * single split needs no header at all and shows its table directly.
 */
export function SplitAccordions({
	mode,
	activeSplit,
	onOpen,
	onChange,
	renderSplit,
}: {
	mode: FixtureMode;
	activeSplit: number | undefined;
	onOpen: (split: number) => void;
	onChange: (mode: FixtureMode) => void;
	renderSplit: (split: number) => React.ReactNode;
}) {
	const [removing, setRemoving] = useState<number | null>(null);
	if (mode.splits.length === 1) return renderSplit(mode.splits[0].number);
	return (
		<div className="fixture-split-accordions">
			{mode.splits.map((split, index) => {
				const open = activeSplit === split.number;
				const used = mode.channels.some((channel) => channel.split === split.number);
				return (
					<section key={split.number} className={open ? "open" : ""}>
						<div className="fixture-split-header">
							<Button
								className="fixture-split-accordion-title"
								aria-expanded={open}
								onClick={() => onOpen(split.number)}
							>
								<span>Split {split.number}</span>
								<small>{channelCount(mode, split.number)}</small>
							</Button>
							<Button
								iconOnly
								aria-label={`Move split ${split.number} up`}
								disabled={index === 0}
								onClick={() =>
									onChange({ ...mode, splits: reorder(mode.splits, index, index - 1) })
								}
							>
								▲
							</Button>
							<Button
								iconOnly
								aria-label={`Move split ${split.number} down`}
								disabled={index === mode.splits.length - 1}
								onClick={() =>
									onChange({ ...mode, splits: reorder(mode.splits, index, index + 1) })
								}
							>
								▼
							</Button>
							<Button
								iconOnly
								variant="danger"
								className="fixture-trash-button"
								aria-label={`Remove split ${split.number}`}
								disabled={used}
								title={
									used ? "Remove or move its channels before removing this split" : "Remove split"
								}
								onClick={() => setRemoving(split.number)}
							>
								<TrashIcon />
							</Button>
						</div>
						{open && renderSplit(split.number)}
					</section>
				);
			})}
			{removing !== null && (
				<ConfirmDialog
					title={`Remove split ${removing}?`}
					description="The split has no channels, so nothing else changes."
					primary="Remove split"
					danger
					onPrimary={() => {
						onChange({
							...mode,
							splits: mode.splits.filter((candidate) => candidate.number !== removing),
						});
						setRemoving(null);
					}}
					secondary="Keep split"
					onSecondary={() => setRemoving(null)}
				/>
			)}
		</div>
	);
}
