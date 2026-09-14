import {
	Button,
	ModalRegistration,
	ModalTitleBar,
	SwitchField,
} from "@tosklight/ui";
import type { FixtureChannel } from "../wire";

/** The masters after Virtual Intensity, in the order the output applies them. */
export const MASTER_FLAGS = [
	["reacts_to_sequence_master", "React to Sequence Master", "SM"],
	["reacts_to_group_master", "React to Group Master", "GM"],
	["reacts_to_grand_master", "React to Grand Master", "GR"],
] as const;

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

/** The masters a channel follows, short enough to sit in a table cell. */
export function mastersSummary(channel: FixtureChannel) {
	const vi = virtualIntensity(channel);
	const active = [
		...(vi === "follow" ? ["VI"] : vi === "inverse" ? ["−VI"] : []),
		...MASTER_FLAGS.filter(([key]) => channel[key]).map(([, , short]) => short),
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
						{MASTER_FLAGS.map(([key, name]) => (
							<SwitchField
								key={key}
								label={name}
								labelPlacement="side"
								offLabel="Ignores"
								onLabel="Reacts"
								checked={channel[key]}
								onChange={(event) =>
									onChange({ ...channel, [key]: event.target.checked })
								}
							/>
						))}
					</div>
				</section>
			</div>
		</ModalRegistration>
	);
}
