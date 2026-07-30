import {
	Button,
	FormLayout,
	SelectField,
	SwitchField,
	TextField,
} from "@tosklight/ui";
import { useState } from "react";
import type {
	AttributeConfiguration,
	AttributeEncoderGroup,
	ConfiguredAttributeDescriptor,
	CustomAttributeDescriptor,
} from "../../api/client/attributeConfiguration";
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
			<article>
				<header>
					<b>Attributes</b>
					<small>
						Show-owned IDs, metadata, and exact six-encoder page positions.
					</small>
				</header>
				<AttributeGroups snapshot={snapshot} onChange={update} />
			</article>
			<article>
				<header>
					<b>Attribute activation groups</b>
					<small>
						Record defaults only. Every attribute belongs to exactly one group
						within its encoder tab.
					</small>
				</header>
				<ActivationGroups snapshot={snapshot} onChange={update} />
			</article>
			{controller.attributeConfigurationError && (
				<p className="modal-error" role="alert">
					{controller.attributeConfigurationError}
				</p>
			)}
		</>
	);
}

function AttributeGroups({
	snapshot,
	onChange,
}: {
	snapshot: NonNullable<SetupWindowController["attributeConfiguration"]>;
	onChange(configuration: AttributeConfiguration): void;
}) {
	const [label, setLabel] = useState("");
	const [group, setGroup] = useState<AttributeEncoderGroup>("intensity");
	const descriptors = projectedDescriptors(snapshot);
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
				{ attribute: id, encoder_group: group, ...placement },
			],
			activation_groups: [
				...snapshot.configuration.activation_groups,
				{ id, label: trimmed, members: [id] },
			],
		});
		setLabel("");
	};
	return (
		<>
			{ENCODER_GROUPS.map(({ id, label: groupLabel }) => (
				<section key={id} className="attribute-registry-group">
					<h3>{groupLabel}</h3>
					<ul className="plain-list">
						{descriptors
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
				</section>
			))}
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
		</>
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
	const activationOptions = configuration.activation_groups
		.filter((candidate) =>
			candidate.members.every((member) =>
				configuration.placements.some(
					(memberPlacement) =>
						memberPlacement.attribute === member &&
						memberPlacement.encoder_group === descriptor.encoder_group,
				),
			),
		)
		.map((candidate) => ({ value: candidate.id, label: candidate.label }));
	return (
		<li>
			<div>
				<b>{custom?.label ?? descriptor.label}</b>
				<small>
					{descriptor.id} · page{" "}
					{placement?.encoder_page ?? descriptor.encoder_page}, encoder{" "}
					{placement?.encoder_slot ?? descriptor.encoder_slot}
					{descriptor.retired ? " · Retired" : ""}
				</small>
			</div>
			<SelectField
				ariaLabel={`${descriptor.label} activation group`}
				value={activationGroupId(configuration, descriptor.id)}
				options={activationOptions}
				onChange={(groupId) =>
					onChange(moveActivationMember(configuration, descriptor.id, groupId))
				}
			/>
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
			<ul className="plain-list">
				{configuration.activation_groups.map((group) => (
					<li key={group.id}>
						<div>
							<b>{group.label}</b>
							<small>{group.members.join(", ")}</small>
						</div>
					</li>
				))}
			</ul>
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
			<Button
				onClick={() => onChange(restoreRecommendedActivationGroups(snapshot))}
			>
				Restore recommended defaults
			</Button>
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

function moveActivationMember(
	configuration: AttributeConfiguration,
	attribute: string,
	targetId: string,
) {
	return {
		...configuration,
		activation_groups: configuration.activation_groups
			.map((group) => ({
				...group,
				members:
					group.id === targetId
						? [...new Set([...group.members, attribute])]
						: group.members.filter((member) => member !== attribute),
			}))
			.filter((group) => group.members.length),
	};
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
