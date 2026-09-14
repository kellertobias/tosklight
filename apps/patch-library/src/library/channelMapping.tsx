import {
	ModalRegistration,
	ModalTitleBar,
	NumberField,
	SelectField,
} from "@tosklight/ui";
import type { AttributeDescriptor, FixtureChannel, FixtureMode } from "../wire";
import { resolutionBytes } from "../sheet/fixtureProfileModel";
import { EditorBreadcrumbs, useEditorTrail } from "./breadcrumbs";
import { channelLabel, channelUnit } from "./channelLabels";
import { FunctionTable } from "./functionTable";

function optionalNumber(value: string) {
	return value.trim() === "" ? null : Number(value);
}

/**
 * What a channel's DMX values mean: where it is addressed, the physical range its values span, and
 * the named ranges within that.
 *
 * Everything else about a channel — what it controls, which byte it is, its default and highlight,
 * inversion, snapping, masters — is a column of the channel table, set where the channel is read.
 */
export function ChannelMappingModal({
	mode,
	channel,
	attributeRegistry,
	onChange,
	onSplit,
	onClose,
}: {
	mode: FixtureMode;
	channel: FixtureChannel;
	attributeRegistry: AttributeDescriptor[];
	onChange: (channel: FixtureChannel) => void;
	onSplit: (split: number) => void;
	onClose: () => void;
}) {
	const name = channelLabel(channel, attributeRegistry);
	const unit = channelUnit(channel, attributeRegistry);
	const trail = useEditorTrail([name], onClose);
	const bits = resolutionBytes(channel.resolution) * 8;
	const setRange = (key: "physical_min" | "physical_max", value: string) =>
		onChange({ ...channel, [key]: optionalNumber(value), unit });
	return (
		<ModalRegistration onClose={onClose}>
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
					aria-label={`${name} mapping`}
				>
					<ModalTitleBar
						title={`Mapping · ${name}`}
						details={<EditorBreadcrumbs trail={trail} />}
						closeLabel="Close channel mapping"
						onClose={onClose}
					/>
					<div className="fixture-channel-editor-body">
						<section className="fixture-mapping-section">
							<h3>Physical range</h3>
							<p className="field-hint">
								What the lowest and highest value of this {bits}-bit channel
								mean{unit ? `, in ${unit}` : ""}. Leave both empty for a channel
								with no physical scale.
							</p>
							<div className="fixture-mapping-fields">
								<NumberField
									label="Physical minimum"
									allowDecimal
									value={channel.physical_min ?? ""}
									onChange={(event) =>
										setRange("physical_min", event.target.value)
									}
								/>
								<NumberField
									label="Physical maximum"
									allowDecimal
									value={channel.physical_max ?? ""}
									onChange={(event) =>
										setRange("physical_max", event.target.value)
									}
								/>
								<div className="fixture-mapping-unit">
									<span>Unit</span>
									<strong>{unit ?? "None"}</strong>
								</div>
								{mode.splits.length > 1 && (
									<SelectField
										label="Address split"
										value={String(channel.split)}
										options={mode.splits.map((split) => ({
											value: String(split.number),
											label: `Split ${split.number}`,
										}))}
										onChange={(value) => onSplit(Number(value))}
									/>
								)}
							</div>
						</section>
						<section className="fixture-mapping-section">
							<FunctionTable
								channel={channel}
								attributeRegistry={attributeRegistry}
								actionIds={mode.control_actions}
								onChange={onChange}
							/>
						</section>
					</div>
				</section>
			</div>
		</ModalRegistration>
	);
}
