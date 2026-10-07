import { Button, ModalRegistration, ModalTitleBar } from "@tosklight/ui";
import { isLevelAttribute } from "../sheet/fixturePatch/patchModel";
import type { FixtureChannel } from "../wire";

type VirtualIntensity = "ignore" | "follow" | "inverse";

const VIRTUAL_INTENSITY: { value: VirtualIntensity; label: string }[] = [
	{ value: "ignore", label: "Ignore" },
	{ value: "follow", label: "Follow" },
	{ value: "inverse", label: "Inverse" },
];

function virtualIntensity(channel: FixtureChannel): VirtualIntensity {
	if (!channel.reacts_to_virtual_intensity) return "ignore";
	return channel.virtual_intensity_inverted ? "inverse" : "follow";
}

/** A level channel: the masters scale its parameter (Intensity, Volume) before DMX. */
function isLevelChannel(channel: FixtureChannel) {
	return (
		isLevelAttribute(channel.attribute) ||
		isLevelAttribute(channel.fixture_attribute)
	);
}

/** How the masters reach a channel, short enough to sit in a table cell. */
export function mastersSummary(channel: FixtureChannel) {
	const vi = virtualIntensity(channel);
	const active = [
		...(isLevelChannel(channel) ? ["Level"] : []),
		...(vi === "follow" ? ["VI"] : vi === "inverse" ? ["−VI"] : []),
	];
	return active.length ? active.join(" · ") : "None";
}

export function MastersModal({
	channel,
	label,
	onChange,
	onClose,
}: {
	channel: FixtureChannel;
	label: string;
	onChange: (channel: FixtureChannel) => void;
	onClose: () => void;
}) {
	const title = `Masters · ${label}`;
	const vi = virtualIntensity(channel);
	return (
		<ModalRegistration onClose={onClose}>
			<div
				className="stacked-modal-layer fixture-masters-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && onClose()
				}
			>
				<section
					className="nested-modal fixture-masters-modal"
					role="dialog"
					aria-modal="true"
					aria-label={title}
				>
					<ModalTitleBar
						title={title}
						closeLabel="Close masters"
						onClose={onClose}
					/>
					<div className="fixture-masters-body">
						{/* Every master scales the level parameters before DMX; any other channel follows
						    them only through the virtual intensity. */}
						{isLevelChannel(channel) && (
							<p className="fixture-masters-note">
								The masters scale this level before DMX.
							</p>
						)}
						{/* Inverse is for a channel that should do the opposite of the dimmer: a lamp that
						    is dark while the virtual intensity is up and lit as it comes down. */}
						<div
							className="fixture-masters-choice"
							role="radiogroup"
							aria-label="React to Virtual Intensity"
						>
							<span>React to Virtual Intensity</span>
							<div>
								{VIRTUAL_INTENSITY.map(({ value, label: name }) => (
									<Button
										key={value}
										role="radio"
										aria-checked={vi === value}
										className={vi === value ? "is-active" : undefined}
										onClick={() =>
											onChange({
												...channel,
												reacts_to_virtual_intensity: value !== "ignore",
												virtual_intensity_inverted: value === "inverse",
											})
										}
									>
										{name}
									</Button>
								))}
							</div>
						</div>
					</div>
				</section>
			</div>
		</ModalRegistration>
	);
}
