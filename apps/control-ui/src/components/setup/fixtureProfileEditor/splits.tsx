import type { FixtureMode } from "../../../api/types";
import { Button, NumberField } from "../../common";
import { channelSplit, reorder } from "../fixtureProfileModel";

export function SplitManager({
	mode,
	onAdd,
	onChange,
}: {
	mode: FixtureMode;
	onAdd: () => void;
	onChange: (mode: FixtureMode) => void;
}) {
	return (
		<section className="fixture-channel-split-manager">
			<header>
				<h3>Address splits</h3>
				<Button onClick={onAdd}>Add split</Button>
			</header>
			<div className="fixture-split-list">
				{mode.splits.map((split, index) => {
					const used = mode.channels.some(
						(channel) => channel.split === split.number,
					);
					return (
						<article key={split.number}>
							<strong>Split {split.number}</strong>
							<NumberField
								label="Footprint"
								min={1}
								max={512}
								value={split.footprint}
								onChange={(event) =>
									onChange({
										...mode,
										splits: mode.splits.map((candidate) =>
											candidate.number === split.number
												? {
														...candidate,
														footprint: Number(event.target.value),
													}
												: candidate,
										),
									})
								}
							/>
							<Button
								iconOnly
								aria-label={`Move split ${split.number} up`}
								disabled={index === 0}
								onClick={() =>
									onChange({
										...mode,
										splits: reorder(mode.splits, index, index - 1),
									})
								}
							>
								▲
							</Button>
							<Button
								iconOnly
								aria-label={`Move split ${split.number} down`}
								disabled={index === mode.splits.length - 1}
								onClick={() =>
									onChange({
										...mode,
										splits: reorder(mode.splits, index, index + 1),
									})
								}
							>
								▼
							</Button>
							<Button
								iconOnly
								aria-label={`Remove split ${split.number}`}
								disabled={mode.splits.length === 1 || used}
								title={
									used
										? "Reassign its channels before removing this split"
										: "Remove split"
								}
								onClick={() =>
									onChange({
										...mode,
										splits: mode.splits.filter(
											(candidate) => candidate.number !== split.number,
										),
									})
								}
							>
								×
							</Button>
						</article>
					);
				})}
			</div>
		</section>
	);
}

export function SplitAccordions({
	mode,
	activeSplit,
	onOpen,
	renderSplit,
}: {
	mode: FixtureMode;
	activeSplit: number | undefined;
	onOpen: (split: number) => void;
	renderSplit: (split: number) => React.ReactNode;
}) {
	if (mode.splits.length === 1) return renderSplit(mode.splits[0].number);
	return (
		<div className="fixture-split-accordions">
			{mode.splits.map((split) => (
				<section
					key={split.number}
					className={activeSplit === split.number ? "open" : ""}
				>
					<Button
						className="fixture-split-accordion-title"
						aria-expanded={activeSplit === split.number}
						onClick={() => onOpen(split.number)}
					>
						<span>Split {split.number}</span>
						<small>
							{split.footprint} slots ·{" "}
							{
								mode.channels.filter(
									(channel) => channelSplit(mode, channel) === split.number,
								).length
							}{" "}
							channels
						</small>
					</Button>
					{activeSplit === split.number && renderSplit(split.number)}
				</section>
			))}
		</div>
	);
}
