import {
	Button,
	FormLayout,
	SelectField,
	SwitchField,
	TextField,
} from "@tosklight/ui";
import {
	type PointerEvent as ReactPointerEvent,
	useEffect,
	useState,
} from "react";
import type {
	AttributeConfiguration,
	AttributeEncoderGroup,
	ConfiguredAttributeDescriptor,
	CustomAttributeDescriptor,
} from "../../api/client/attributeConfiguration";
import type { FixtureSourceMapping } from "../../api/client/fixtures";
import {
	attributeEncoderGroups,
	projectPushTurnPlacements,
} from "../../components/control/parameterControls/attributeEncoderPages";
import { useFixtureLibrary } from "../../features/fixtureLibrary/FixtureLibraryContext";
import { useScreensOptional } from "../../features/screens/ScreensContext";
import {
	ACTIVATION_PRESETS,
	addActivationMember,
	applyActivationPreset,
	deleteActivationGroup,
	removeActivationMember,
	renameActivationGroup,
} from "./activationPresets";
import {
	type EncoderSlotTarget,
	moveAttributeToSlot,
	unplacedDescriptors,
} from "./encoderLayoutModel";
import { useEncoderSlotDrag } from "./useEncoderSlotDrag";
import type { SetupWindowController } from "./controller";

const ENCODER_GROUPS: Array<{
	id: AttributeEncoderGroup;
	label: string;
}> = [
	{ id: "intensity", label: "Intensity" },
	{ id: "color", label: "Color" },
	{ id: "position", label: "Position" },
	{ id: "beam", label: "Beam" },
	{ id: "shapers", label: "Shapers" },
	{ id: "focus", label: "Focus" },
	{ id: "control", label: "Control" },
	{ id: "media", label: "Media" },
];

export function AttributeRegistrySettings({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const activeTab = controller.attributeTab;
	const snapshot = controller.attributeConfiguration;
	if (!snapshot)
		return (
			<article>
				<header>
					<b>Attributes</b>
					<small>Loading the active show attribute registry…</small>
				</header>
			</article>
		);
	const update = (configuration: AttributeConfiguration) =>
		controller.editAttributeConfiguration(configuration);
	return (
		<>
			{activeTab === "encoder-groups" && (
				<div className="attribute-tabpanel" role="tabpanel">
					<EncoderLayoutEditor snapshot={snapshot} onChange={update} />
				</div>
			)}
			{activeTab === "activation-groups" && (
				<div className="attribute-tabpanel" role="tabpanel">
					<ActivationGroups snapshot={snapshot} onChange={update} />
				</div>
			)}
			{activeTab === "attributes" && (
				<div className="attribute-tabpanel" role="tabpanel">
					<AttributeGroups snapshot={snapshot} onChange={update} mode="all" />
					<SourceAttributeMappings snapshot={snapshot} />
				</div>
			)}
			{controller.attributeConfigurationError && (
				<p className="modal-error" role="alert">
					{controller.attributeConfigurationError}
				</p>
			)}
		</>
	);
}

/**
 * Desk-local aliases from a fixture-source attribute name onto an attribute this desk already
 * has. An import fills these in when it meets an unknown channel; the operator can also declare
 * one up front, so a GDTF that calls Media Folder "MediaRank" lands on the built-in control.
 */
function SourceAttributeMappings({
	snapshot,
}: {
	snapshot: NonNullable<SetupWindowController["attributeConfiguration"]>;
}) {
	const fixtureLibrary = useFixtureLibrary();
	const [mappings, setMappings] = useState<FixtureSourceMapping[]>([]);
	const [sourceAttribute, setSourceAttribute] = useState("");
	const [targetAttribute, setTargetAttribute] = useState("");
	const [error, setError] = useState<string | null>(null);
	useEffect(() => {
		let active = true;
		void fixtureLibrary
			?.fixtureSourceMappings?.()
			.then((next) => active && setMappings(next))
			.catch(
				(reason) =>
					active &&
					setError(reason instanceof Error ? reason.message : String(reason)),
			);
		return () => {
			active = false;
		};
	}, [fixtureLibrary]);
	if (!fixtureLibrary?.fixtureSourceMappings) return null;
	const targets = projectedDescriptors(snapshot)
		.filter((descriptor) => !descriptor.retired)
		.map((descriptor) => ({
			value: descriptor.id,
			label: `${descriptor.label} (${descriptor.id})`,
		}));
	const setTarget = async (
		sourceFormat: string,
		source: string,
		target: string | null,
	) => {
		try {
			const saved = await fixtureLibrary.rememberFixtureSourceMapping?.({
				sourceFormat,
				sourceAttribute: source,
				targetAttribute: target,
			});
			setMappings((current) => [
				...current.filter(
					(mapping) =>
						mapping.source_format !== sourceFormat ||
						mapping.source_attribute !== source,
				),
				...(saved ? [saved] : []),
			]);
			setError(null);
		} catch (reason) {
			setError(reason instanceof Error ? reason.message : String(reason));
		}
	};
	const create = async () => {
		const trimmed = sourceAttribute.trim();
		if (!trimmed || !targetAttribute) return;
		await setTarget("gdtf", trimmed, targetAttribute);
		setSourceAttribute("");
	};
	return (
		<article>
			<header>
				<b>Imported attribute names</b>
				<small>
					Desk-local, and kept out of the show file. Fixture revisions retain
					their already-resolved mapping.
				</small>
			</header>
			<p className="attribute-registry-note">
				Map a name a GDTF file uses onto the attribute this desk already
				programs — for example a GDTF <code>MediaRank</code> onto{" "}
				<code>media.folder</code>. Use <b>New custom attribute</b> instead when
				the source channel means something the desk does not have yet.
			</p>
			{mappings.length > 0 && (
				<ul className="plain-list attribute-mapping-list">
					{mappings.map((mapping) => (
						<li key={`${mapping.source_format}:${mapping.source_attribute}`}>
							<code>
								{mapping.source_format.toUpperCase()}:{mapping.source_attribute}
							</code>
							<SelectField
								ariaLabel={`Map ${mapping.source_format.toUpperCase()}:${mapping.source_attribute} to existing attribute`}
								value={mapping.target_attribute}
								options={targets}
								onChange={(target) =>
									void setTarget(
										mapping.source_format,
										mapping.source_attribute,
										target,
									)
								}
							/>
							<Button
								onClick={() =>
									void setTarget(
										mapping.source_format,
										mapping.source_attribute,
										null,
									)
								}
							>
								Forget mapping
							</Button>
						</li>
					))}
				</ul>
			)}
			<FormLayout labelPlacement="side">
				<TextField
					label="GDTF attribute name"
					value={sourceAttribute}
					onChange={(event) => setSourceAttribute(event.target.value)}
					description="The attribute name as the GDTF file spells it, without the GDTF: prefix."
				/>
				<SelectField
					label="Means this attribute"
					ariaLabel="Means this attribute"
					value={targetAttribute}
					options={[{ value: "", label: "Choose an attribute" }, ...targets]}
					onChange={setTargetAttribute}
				/>
			</FormLayout>
			<Button
				disabled={!sourceAttribute.trim() || !targetAttribute}
				onClick={() => void create()}
			>
				Map imported name
			</Button>
			{error && <p role="alert">{error}</p>}
		</article>
	);
}

function EncoderLayoutEditor({
	snapshot,
	onChange,
}: {
	snapshot: NonNullable<SetupWindowController["attributeConfiguration"]>;
	onChange(configuration: AttributeConfiguration): void;
}) {
	const screens = useScreensOptional();
	const width = screens?.screens?.programmer_control_surface.visible_encoders ?? 6;
	const configuration = snapshot.configuration;
	const placed = projectedDescriptors(snapshot).filter(
		(descriptor) => !descriptor.retired,
	);
	const { drag, begin, hover } = useEncoderSlotDrag({
		onCommit: (attribute, target) =>
			onChange(
				moveAttributeToSlot(configuration, placed, attribute, target, width),
			),
	});
	// The layout is rendered from the pending move, so the tiles visibly reshuffle while dragging.
	const previewConfiguration =
		drag?.active && drag.target
			? moveAttributeToSlot(
					configuration,
					placed,
					drag.attribute,
					drag.target,
					width,
				)
			: configuration;
	const descriptors =
		previewConfiguration === configuration
			? placed
			: projectedDescriptors({
					...snapshot,
					configuration: previewConfiguration,
				}).filter((descriptor) => !descriptor.retired);
	const encoderControls = projectPushTurnPlacements(descriptors);
	const groups = attributeEncoderGroups(
		encoderControls,
		new Set(descriptors.map(({ id }) => id)),
		width,
	);
	const unplaced = unplacedDescriptors(descriptors, configuration);
	return (
		<section
			className={`attribute-layout-preview ${drag?.active ? "is-dragging" : ""}`.trim()}
			aria-label={`${width}-encoder layout editor`}
		>
			<article>
				<header>
					<b>Encoder groups</b>
					<small>
						Drag an encoder onto any slot; the layout follows the pointer and
						keeps the move when you let go.
					</small>
				</header>
				<p className="attribute-layout-width-note">
					{`${width} encoders per page, from Screens & playback → Encoder placement.`}
				</p>
			</article>
			{ENCODER_GROUPS.map(({ id, label }) => {
				const group = groups.find((candidate) => candidate.id === id);
				const pages = group?.pages.length
					? group.pages
					: [{ number: 1, slots: Array.from({ length: width }, () => null) }];
				const placedHere = descriptors.filter(
					(descriptor) => descriptor.encoder_group === id,
				).length;
				return (
					<article key={id} className="attribute-layout-group">
						<header>
							<h3>{label}</h3>
							<small>
								{placedHere} {placedHere === 1 ? "encoder" : "encoders"} ·{" "}
								{pages.length} {pages.length === 1 ? "page" : "pages"}
							</small>
						</header>
						{pages.map((page) => (
							<div key={page.number} className="attribute-layout-page">
								<b>Page {page.number}</b>
								<div
									className="attribute-layout-slots"
									style={{
										gridTemplateColumns: `repeat(${width}, minmax(0, 1fr))`,
									}}
								>
									{Array.from({ length: width }, (_, index) => {
										const descriptor = page.slots[index] ?? null;
										const target = {
											group: id,
											page: page.number,
											slot: index + 1,
										};
										return (
											<EncoderSlot
												key={`${id}-${page.number}-${index}`}
												descriptor={descriptor}
												target={target}
												descriptors={descriptors}
												unplaced={unplaced}
												configuration={configuration}
												width={width}
												dragging={
													drag?.active && drag.attribute === descriptor?.id
												}
												dropTarget={Boolean(
													drag?.active &&
														drag.target?.group === target.group &&
														drag.target.page === target.page &&
														drag.target.slot === target.slot,
												)}
												onChange={onChange}
												onDragBegin={begin}
												onDragOver={hover}
											/>
										);
									})}
								</div>
							</div>
						))}
					</article>
				);
			})}
			{drag?.active && (
				<div
					className="attribute-layout-drag-chip"
					style={{ left: `${drag.x}px`, top: `${drag.y}px` }}
					aria-hidden="true"
				>
					{drag.label}
				</div>
			)}
		</section>
	);
}

function EncoderSlot({
	descriptor,
	target,
	descriptors,
	unplaced,
	configuration,
	width,
	dragging,
	dropTarget,
	onChange,
	onDragBegin,
	onDragOver,
}: {
	descriptor:
		| (ConfiguredAttributeDescriptor & { push_turn_label?: string | null })
		| null;
	target: EncoderSlotTarget;
	descriptors: ConfiguredAttributeDescriptor[];
	unplaced: ConfiguredAttributeDescriptor[];
	configuration: AttributeConfiguration;
	width: number;
	dragging?: boolean;
	dropTarget: boolean;
	onChange(configuration: AttributeConfiguration): void;
	onDragBegin(
		event: ReactPointerEvent,
		attribute: string,
		label: string,
	): void;
	onDragOver(target: EncoderSlotTarget): void;
}) {
	const { group, page, slot } = target;
	const classes = [
		"attribute-layout-slot",
		descriptor ? "" : "is-unassigned",
		descriptor ? "is-draggable" : "",
		dragging ? "is-dragging" : "",
		dropTarget ? "is-drop-target" : "",
	];
	return (
		// biome-ignore lint/a11y/useSemanticElements: The slot is a drop target that already contains its own controls.
		<div
			className={classes.filter(Boolean).join(" ")}
			aria-label={`${group} page ${page} encoder ${slot}`}
			onPointerDown={(event) =>
				descriptor && onDragBegin(event, descriptor.id, descriptor.label)
			}
			onPointerEnter={() => onDragOver(target)}
			onPointerMove={() => onDragOver(target)}
		>
			<small>E{slot}</small>
			<strong>{descriptor?.label ?? "Unassigned"}</strong>
			{descriptor?.push_turn_label && (
				<small>Push-turn · {descriptor.push_turn_label}</small>
			)}
			{descriptor ? (
				<>
					<small>{descriptor.built_in ? "Built-in" : "Custom"}</small>
					<AttributeOrderActions
						descriptor={descriptor}
						descriptors={descriptors}
						configuration={configuration}
						width={width}
						onChange={onChange}
					/>
					<details>
						<summary>Details</summary>
						<code>{descriptor.id}</code>
					</details>
				</>
			) : (
				unplaced.length > 0 && (
					<SelectField
						ariaLabel={`Assign ${group} page ${page} encoder ${slot}`}
						value=""
						options={[
							{ value: "", label: "Assign attribute" },
							...unplaced.map((candidate) => ({
								value: candidate.id,
								label: candidate.label,
							})),
						]}
						onChange={(value) =>
							value &&
							onChange(
								moveAttributeToSlot(
									configuration,
									descriptors,
									value,
									{ group, page, slot },
									width,
								),
							)
						}
					/>
				)
			)}
		</div>
	);
}

function AttributeOrderActions({
	descriptor,
	descriptors,
	configuration,
	width = 6,
	onChange,
}: {
	descriptor: ConfiguredAttributeDescriptor;
	descriptors: ConfiguredAttributeDescriptor[];
	configuration: AttributeConfiguration;
	width?: number;
	onChange(configuration: AttributeConfiguration): void;
}) {
	const move = (delta: -1 | 1) =>
		onChange(
			reorderAttribute(configuration, descriptors, descriptor.id, delta, width),
		);
	const companion = descriptors.find(
		(candidate) => candidate.push_turn_of === descriptor.id,
	);
	const companionOptions = descriptors
		.filter(
			(candidate) =>
				candidate.id !== descriptor.id &&
				candidate.encoder_group === descriptor.encoder_group &&
				!descriptors.some(
					(other) =>
						other.push_turn_of === candidate.id && other.id !== descriptor.id,
				) &&
				(!candidate.push_turn_of || candidate.push_turn_of === descriptor.id),
		)
		.map((candidate) => ({ value: candidate.id, label: candidate.label }));
	return (
		<>
			<div className="attribute-layout-order-actions">
				<Button
					aria-label={`Move ${descriptor.label} earlier`}
					disabled={isGroupBoundary(descriptors, descriptor.id, -1)}
					onClick={() => move(-1)}
				>
					←
				</Button>
				<Button
					aria-label={`Move ${descriptor.label} later`}
					disabled={isGroupBoundary(descriptors, descriptor.id, 1)}
					onClick={() => move(1)}
				>
					→
				</Button>
			</div>
			{companionOptions.length > 0 && (
				<SelectField
					ariaLabel={`${descriptor.label} push-turn companion`}
					value={companion?.id ?? ""}
					options={[
						{ value: "", label: "No push-turn companion" },
						...companionOptions,
					]}
					onChange={(value) =>
						onChange(
							updatePushTurnCompanion(
								configuration,
								descriptor.id,
								value || null,
							),
						)
					}
				/>
			)}
		</>
	);
}

export function updatePushTurnCompanion(
	configuration: AttributeConfiguration,
	primaryAttribute: string,
	companionAttribute: string | null,
) {
	return {
		...configuration,
		placements: configuration.placements.map((placement) => {
			if (placement.attribute === companionAttribute) {
				return { ...placement, push_turn_of: primaryAttribute };
			}
			if (placement.push_turn_of === primaryAttribute) {
				return { ...placement, push_turn_of: null };
			}
			return placement;
		}),
	};
}

function descriptorsInGroup(
	descriptors: ConfiguredAttributeDescriptor[],
	attribute: string,
) {
	const descriptor = descriptors.find(
		(candidate) => candidate.id === attribute,
	);
	return descriptor
		? descriptors.filter(
				(candidate) => candidate.encoder_group === descriptor.encoder_group,
			)
		: [];
}

function isGroupBoundary(
	descriptors: ConfiguredAttributeDescriptor[],
	attribute: string,
	delta: -1 | 1,
) {
	const ordered = descriptorsInGroup(descriptors, attribute);
	const index = ordered.findIndex((descriptor) => descriptor.id === attribute);
	return index < 0 || index + delta < 0 || index + delta >= ordered.length;
}

export function reorderAttribute(
	configuration: AttributeConfiguration,
	descriptors: ConfiguredAttributeDescriptor[],
	attribute: string,
	delta: -1 | 1,
	width = 6,
) {
	const ordered = descriptorsInGroup(descriptors, attribute);
	const index = ordered.findIndex((descriptor) => descriptor.id === attribute);
	const target = index + delta;
	if (index < 0 || target < 0 || target >= ordered.length) return configuration;
	[ordered[index], ordered[target]] = [ordered[target], ordered[index]];
	const reorderedIds = new Set(ordered.map((descriptor) => descriptor.id));
	return {
		...configuration,
		placements: [
			...configuration.placements.filter(
				(placement) => !reorderedIds.has(placement.attribute),
			),
			...ordered.map((descriptor, position) => ({
				attribute: descriptor.id,
				encoder_group: descriptor.encoder_group,
				encoder_page: Math.floor(position / width) + 1,
				encoder_slot: (position % width) + 1,
				push_turn_of: descriptor.push_turn_of,
			})),
		],
	};
}

function AttributeGroups({
	snapshot,
	onChange,
	mode,
}: {
	snapshot: NonNullable<SetupWindowController["attributeConfiguration"]>;
	onChange(configuration: AttributeConfiguration): void;
	mode: "built-in" | "custom" | "all";
}) {
	const descriptors = projectedDescriptors(snapshot);
	const visibleDescriptors = descriptors.filter((descriptor) =>
		mode === "all"
			? true
			: mode === "built-in"
				? descriptor.built_in
				: !descriptor.built_in,
	);
	return (
		<>
			{ENCODER_GROUPS.map(({ id, label: groupLabel }) => (
				<article key={id} className="attribute-registry-group">
					<header>
						<h3>{groupLabel}</h3>
						<small>
							{
								visibleDescriptors.filter(
									(descriptor) => descriptor.encoder_group === id,
								).length
							}{" "}
							attributes
						</small>
					</header>
					<ul className="plain-list">
						{visibleDescriptors
							.filter((descriptor) => descriptor.encoder_group === id)
							.map((descriptor) => (
								<AttributeRow
									key={descriptor.id}
									descriptor={descriptor}
									configuration={snapshot.configuration}
									onChange={onChange}
								/>
							))}
					</ul>
				</article>
			))}
			{mode !== "built-in" && (
				<NewAttribute snapshot={snapshot} onChange={onChange} />
			)}
		</>
	);
}

/**
 * A new attribute is either a genuinely new show-owned control or another name for an
 * attribute this desk already programs; the operator chooses which before filling the form.
 */
function NewAttribute({
	snapshot,
	onChange,
}: {
	snapshot: NonNullable<SetupWindowController["attributeConfiguration"]>;
	onChange(configuration: AttributeConfiguration): void;
}) {
	const [label, setLabel] = useState("");
	const [group, setGroup] = useState<AttributeEncoderGroup>("intensity");
	const create = () => {
		const trimmed = label.trim();
		if (!trimmed) return;
		const id = customAttributeId(trimmed);
		const placement = nextPlacement(snapshot.configuration, group);
		const custom: CustomAttributeDescriptor = {
			id,
			label: trimmed,
			value_type: "continuous",
			display_unit: null,
			physical_unit: null,
			normalized_bounds: { min: 0, max: 1 },
			domain_bounds: null,
			cyclic: false,
			recordable: true,
			lifecycle: "active",
		};
		onChange({
			...snapshot.configuration,
			custom_attributes: [...snapshot.configuration.custom_attributes, custom],
			placements: [
				...snapshot.configuration.placements,
				{
					attribute: id,
					encoder_group: group,
					...placement,
					push_turn_of: null,
				},
			],
			activation_groups: [
				...snapshot.configuration.activation_groups,
				{ id, label: trimmed, members: [id] },
			],
		});
		setLabel("");
	};
	return (
		<article>
			<header>
				<b>New custom attribute</b>
				<small>Show-owned, and placed on the encoder group you choose.</small>
			</header>
			<p className="attribute-registry-note">
				Create one when the control means something the desk does not have yet —
				a Media Group, say. When an imported file only spells an existing
				attribute differently, map the name under{" "}
				<b>Imported attribute names</b> instead of adding a second control for
				the same thing.
			</p>
			<FormLayout labelPlacement="side">
				<TextField
					label="New custom attribute"
					value={label}
					onChange={(event) => setLabel(event.target.value)}
				/>
				<SelectField
					label="Encoder group"
					value={group}
					options={ENCODER_GROUPS.map(({ id, label }) => ({
						value: id,
						label,
					}))}
					onChange={(value) => setGroup(value as AttributeEncoderGroup)}
				/>
			</FormLayout>
			<Button disabled={!label.trim()} onClick={create}>
				Create custom attribute
			</Button>
		</article>
	);
}

function AttributeRow({
	descriptor,
	configuration,
	onChange,
}: {
	descriptor: ConfiguredAttributeDescriptor;
	configuration: AttributeConfiguration;
	onChange(configuration: AttributeConfiguration): void;
}) {
	const custom = configuration.custom_attributes.find(
		(candidate) => candidate.id === descriptor.id,
	);
	const placement = configuration.placements.find(
		(candidate) => candidate.attribute === descriptor.id,
	);
	return (
		<li>
			<div>
				<b>{custom?.label ?? descriptor.label}</b>
				<small>
					{descriptor.built_in ? "Built-in" : "Custom"} · {descriptor.id} · page{" "}
					{placement?.encoder_page ?? descriptor.encoder_page}, encoder{" "}
					{placement?.encoder_slot ?? descriptor.encoder_slot}
					{descriptor.retired ? " · Retired" : ""}
				</small>
			</div>
			{custom && placement && (
				<CustomAttributeControls
					configuration={configuration}
					custom={custom}
					descriptor={descriptor}
					placement={placement}
					onChange={onChange}
				/>
			)}
		</li>
	);
}

function CustomAttributeControls({
	configuration,
	custom,
	descriptor,
	placement,
	onChange,
}: {
	configuration: AttributeConfiguration;
	custom: CustomAttributeDescriptor;
	descriptor: ConfiguredAttributeDescriptor;
	placement: AttributeConfiguration["placements"][number];
	onChange(configuration: AttributeConfiguration): void;
}) {
	return (
		<>
			<TextField
				aria-label={`${descriptor.id} label`}
				value={custom.label}
				onChange={(event) =>
					onChange(
						updateCustom(configuration, custom.id, {
							label: event.target.value,
						}),
					)
				}
			/>
			<SelectField
				ariaLabel={`${descriptor.id} value type`}
				value={custom.value_type}
				options={[
					{ value: "continuous", label: "Continuous" },
					{ value: "color", label: "Color" },
					{ value: "indexed", label: "Indexed" },
					{ value: "control", label: "Control" },
				]}
				onChange={(value) =>
					value !== custom.value_type &&
					window.confirm(
						`Change ${custom.label} from ${custom.value_type} to ${value}? This can change the meaning of fixture profiles, Programmer values, Presets, and Cues that use ${custom.id}.`,
					) &&
					onChange(
						updateCustom(configuration, custom.id, {
							value_type: value as CustomAttributeDescriptor["value_type"],
						}),
					)
				}
			/>
			<TextField
				aria-label={`${descriptor.id} display unit`}
				value={custom.display_unit ?? ""}
				onChange={(event) =>
					onChange(
						updateCustom(configuration, custom.id, {
							display_unit: event.target.value || null,
						}),
					)
				}
			/>
			<SelectField
				ariaLabel={`${descriptor.id} encoder group`}
				value={placement.encoder_group}
				options={ENCODER_GROUPS.map(({ id, label }) => ({ value: id, label }))}
				onChange={(value) =>
					onChange(
						moveCustomEncoderGroup(
							configuration,
							descriptor.id,
							value as AttributeEncoderGroup,
						),
					)
				}
			/>
			<SelectField
				ariaLabel={`${descriptor.id} encoder position`}
				value={`${placement.encoder_page}:${placement.encoder_slot}`}
				options={availablePositions(
					configuration,
					placement.encoder_group,
					descriptor.id,
				)}
				onChange={(value) => {
					const [encoder_page, encoder_slot] = value.split(":").map(Number);
					onChange(
						updatePlacement(configuration, descriptor.id, {
							encoder_page,
							encoder_slot,
						}),
					);
				}}
			/>
			<SwitchField
				label="Cyclic"
				offLabel="Bounded"
				onLabel="Cyclic"
				checked={custom.cyclic}
				onChange={(event) =>
					onChange(
						updateCustom(configuration, custom.id, {
							cyclic: event.target.checked,
						}),
					)
				}
			/>
			<SwitchField
				label="Recordable"
				offLabel="Transient"
				onLabel="Recordable"
				checked={custom.recordable}
				onChange={(event) =>
					onChange(
						updateCustom(configuration, custom.id, {
							recordable: event.target.checked,
						}),
					)
				}
			/>
			<Button
				onClick={() =>
					onChange(
						updateCustom(configuration, custom.id, {
							lifecycle: custom.lifecycle === "active" ? "retired" : "active",
						}),
					)
				}
			>
				{custom.lifecycle === "active" ? "Retire" : "Reactivate"}
			</Button>
		</>
	);
}

function ActivationGroups({
	snapshot,
	onChange,
}: {
	snapshot: NonNullable<SetupWindowController["attributeConfiguration"]>;
	onChange(configuration: AttributeConfiguration): void;
}) {
	const configuration = snapshot.configuration;
	const descriptors = projectedDescriptors(snapshot);
	const [name, setName] = useState("");
	const [member, setMember] = useState(descriptors[0]?.id ?? "");
	const label = (id: string) =>
		descriptors.find((descriptor) => descriptor.id === id)?.label ?? id;
	const unassigned = descriptors.filter(
		(descriptor) =>
			!descriptor.retired &&
			!configuration.activation_groups.some((group) =>
				group.members.includes(descriptor.id),
			),
	);
	const create = () => {
		const trimmed = name.trim();
		if (!trimmed || !member) return;
		const id = `activation.${crypto.randomUUID()}`;
		const withoutMember = configuration.activation_groups
			.map((group) => ({
				...group,
				members: group.members.filter((candidate) => candidate !== member),
			}))
			.filter((group) => group.members.length);
		onChange({
			...configuration,
			activation_groups: [
				...withoutMember,
				{ id, label: trimmed, members: [member] },
			],
		});
		setName("");
	};
	return (
		<>
			<article className="activation-presets">
				<header>
					<b>Start from a preset</b>
					<small>
						A preset replaces every group below; edit the result freely
						afterwards.
					</small>
				</header>
				<div className="activation-preset-buttons">
					{ACTIVATION_PRESETS.map((preset) => (
						<Button
							key={preset.id}
							title={preset.detail}
							onClick={() =>
								onChange(
									applyActivationPreset(
										preset.id,
										configuration,
										descriptors,
										restoreRecommendedActivationGroups(snapshot)
											.activation_groups,
									),
								)
							}
						>
							{preset.label}
						</Button>
					))}
					<Button
						onClick={() =>
							onChange(restoreRecommendedActivationGroups(snapshot))
						}
					>
						Restore recommended defaults
					</Button>
				</div>
			</article>
			{configuration.activation_groups.map((group) => (
				<article key={group.id} className="activation-group">
					<header className="activation-group-header">
						<TextField
							aria-label={`${group.label} name`}
							value={group.label}
							onChange={(event) =>
								onChange(
									renameActivationGroup(
										configuration,
										group.id,
										event.target.value,
									),
								)
							}
						/>
						<Button
							variant="danger"
							aria-label={`Delete ${group.label}`}
							onClick={() =>
								onChange(deleteActivationGroup(configuration, group.id))
							}
						>
							Delete
						</Button>
					</header>
					<div className="activation-group-members">
						{group.members.map((id) => (
							<Button
								key={id}
								className="activation-member"
								aria-label={`Remove ${label(id)} from ${group.label}`}
								onClick={() =>
									onChange(removeActivationMember(configuration, group.id, id))
								}
							>
								{label(id)} <span aria-hidden="true">×</span>
							</Button>
						))}
						{unassigned.length > 0 && (
							<SelectField
								ariaLabel={`Add attribute to ${group.label}`}
								value=""
								options={[
									{ value: "", label: "Add attribute" },
									...unassigned.map((descriptor) => ({
										value: descriptor.id,
										label: descriptor.label,
									})),
								]}
								onChange={(value) =>
									value &&
									onChange(addActivationMember(configuration, group.id, value))
								}
							/>
						)}
					</div>
				</article>
			))}
			<article>
				<header>
					<b>New activation group</b>
					<small>
						Exactly one member is active within a group, and groups never cross
						encoder tabs.
					</small>
				</header>
				{!configuration.activation_groups.length && (
					<p role="status">
						No activation groups. Every attribute activates on its own.
					</p>
				)}
				<FormLayout labelPlacement="side">
					<TextField
						label="New activation group"
						value={name}
						onChange={(event) => setName(event.target.value)}
					/>
					<SelectField
						label="First member"
						value={member}
						options={descriptors.map((descriptor) => ({
							value: descriptor.id,
							label: `${descriptor.label} (${descriptor.encoder_group})`,
						}))}
						onChange={setMember}
					/>
				</FormLayout>
				<Button disabled={!name.trim() || !member} onClick={create}>
					Create activation group
				</Button>
			</article>
		</>
	);
}

function restoreRecommendedActivationGroups(
	snapshot: NonNullable<SetupWindowController["attributeConfiguration"]>,
): AttributeConfiguration {
	const configuration = snapshot.configuration;
	const customIds = new Set(
		configuration.custom_attributes.map((descriptor) => descriptor.id),
	);
	const retainedCustomGroups = configuration.activation_groups.filter(
		(group) =>
			group.members.length > 0 &&
			group.members.every((member) => customIds.has(member)),
	);
	const assigned = new Set(
		retainedCustomGroups.flatMap((group) => group.members),
	);
	for (const descriptor of configuration.custom_attributes)
		if (!assigned.has(descriptor.id))
			retainedCustomGroups.push({
				id: descriptor.id,
				label: descriptor.label,
				members: [descriptor.id],
			});
	return {
		...configuration,
		activation_groups: [
			...snapshot.recommended_configuration.activation_groups,
			...retainedCustomGroups,
		],
	};
}

function projectedDescriptors(
	snapshot: NonNullable<SetupWindowController["attributeConfiguration"]>,
) {
	const customById = new Map(
		snapshot.configuration.custom_attributes.map((descriptor) => [
			descriptor.id,
			descriptor,
		]),
	);
	const descriptors = snapshot.descriptors.map((descriptor) => {
		const custom = customById.get(descriptor.id);
		const placement = snapshot.configuration.placements.find(
			(candidate) => candidate.attribute === descriptor.id,
		);
		return {
			...descriptor,
			label: custom?.label ?? descriptor.label,
			retired: custom?.lifecycle === "retired" || descriptor.retired,
			encoder_group: placement?.encoder_group ?? descriptor.encoder_group,
			encoder_page: placement?.encoder_page ?? descriptor.encoder_page,
			encoder_slot: placement?.encoder_slot ?? descriptor.encoder_slot,
		};
	});
	for (const custom of snapshot.configuration.custom_attributes) {
		if (descriptors.some((descriptor) => descriptor.id === custom.id)) continue;
		const placement = snapshot.configuration.placements.find(
			(candidate) => candidate.attribute === custom.id,
		);
		if (!placement) continue;
		descriptors.push({
			id: custom.id,
			label: custom.label,
			...placement,
			value_type: custom.value_type,
			display_unit: custom.display_unit,
			physical_unit: custom.physical_unit,
			normalized_min: custom.normalized_bounds?.min ?? null,
			normalized_max: custom.normalized_bounds?.max ?? null,
			domain_min: custom.domain_bounds?.min ?? null,
			domain_max: custom.domain_bounds?.max ?? null,
			cyclic: custom.cyclic,
			recordable: custom.recordable,
			built_in: false,
			retired: custom.lifecycle === "retired",
			activation_group_id: activationGroupId(snapshot.configuration, custom.id),
		});
	}
	return descriptors.sort(
		(left, right) =>
			ENCODER_GROUPS.findIndex((group) => group.id === left.encoder_group) -
				ENCODER_GROUPS.findIndex((group) => group.id === right.encoder_group) ||
			left.encoder_page - right.encoder_page ||
			left.encoder_slot - right.encoder_slot,
	);
}

function customAttributeId(label: string) {
	const slug =
		label
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, ".")
			.replace(/^\.+|\.+$/g, "") || "attribute";
	return `custom.${slug}.${crypto.randomUUID()}`;
}

function nextPlacement(
	configuration: AttributeConfiguration,
	group: AttributeEncoderGroup,
) {
	const occupied = new Set(
		configuration.placements
			.filter((placement) => placement.encoder_group === group)
			.map(
				(placement) => `${placement.encoder_page}:${placement.encoder_slot}`,
			),
	);
	for (let page = 1; ; page += 1)
		for (let slot = 1; slot <= 6; slot += 1)
			if (!occupied.has(`${page}:${slot}`))
				return { encoder_page: page, encoder_slot: slot };
}

function activationGroupId(
	configuration: AttributeConfiguration,
	attribute: string,
) {
	return (
		configuration.activation_groups.find((group) =>
			group.members.includes(attribute),
		)?.id ?? ""
	);
}

function updatePlacement(
	configuration: AttributeConfiguration,
	attribute: string,
	patch: Partial<{
		encoder_page: number;
		encoder_slot: number;
	}>,
) {
	return {
		...configuration,
		placements: configuration.placements.map((placement) =>
			placement.attribute === attribute
				? { ...placement, ...patch }
				: placement,
		),
	};
}

function updateCustom(
	configuration: AttributeConfiguration,
	attribute: string,
	patch: Partial<CustomAttributeDescriptor>,
) {
	return {
		...configuration,
		custom_attributes: configuration.custom_attributes.map((descriptor) =>
			descriptor.id === attribute ? { ...descriptor, ...patch } : descriptor,
		),
	};
}

function availablePositions(
	configuration: AttributeConfiguration,
	group: AttributeEncoderGroup,
	attribute: string,
) {
	const placements = configuration.placements.filter(
		(placement) => placement.encoder_group === group,
	);
	const maximumPage = Math.max(
		1,
		...placements.map((placement) => placement.encoder_page),
	);
	const occupied = new Set(
		placements
			.filter((placement) => placement.attribute !== attribute)
			.map(
				(placement) => `${placement.encoder_page}:${placement.encoder_slot}`,
			),
	);
	const options = [];
	for (let page = 1; page <= maximumPage + 1; page += 1)
		for (let slot = 1; slot <= 6; slot += 1) {
			const value = `${page}:${slot}`;
			if (!occupied.has(value))
				options.push({
					value,
					label: `Page ${page}, encoder ${slot}`,
				});
		}
	return options;
}

function moveCustomEncoderGroup(
	configuration: AttributeConfiguration,
	attribute: string,
	group: AttributeEncoderGroup,
) {
	const current = configuration.placements.find(
		(placement) => placement.attribute === attribute,
	);
	if (!current || current.encoder_group === group) return configuration;
	const next = nextPlacement(configuration, group);
	const activation_groups = configuration.activation_groups
		.map((activation) => ({
			...activation,
			members: activation.members.filter((member) => member !== attribute),
		}))
		.filter((activation) => activation.members.length);
	return {
		...configuration,
		placements: configuration.placements.map((placement) =>
			placement.attribute === attribute
				? { ...placement, encoder_group: group, ...next }
				: placement,
		),
		activation_groups: [
			...activation_groups,
			{
				id: `activation.${attribute}`,
				label:
					configuration.custom_attributes.find(
						(descriptor) => descriptor.id === attribute,
					)?.label ?? attribute,
				members: [attribute],
			},
		],
	};
}
