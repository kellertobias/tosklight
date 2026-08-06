import type {
	AttributeConfiguration,
	ConfiguredAttributeDescriptor,
} from "../../api/client/attributeConfiguration";

export type ActivationPreset = "none" | "all" | "encoder-group" | "intelligent";

export const ACTIVATION_PRESETS: ReadonlyArray<{
	id: ActivationPreset;
	label: string;
	detail: string;
}> = [
	{
		id: "none",
		label: "None",
		detail: "No activation groups; every attribute activates on its own.",
	},
	{
		id: "all",
		label: "All",
		detail: "One group holding every attribute.",
	},
	{
		id: "encoder-group",
		label: "By Encoder Group",
		detail: "The semantic default grouping; anything else stays on its own.",
	},
	{
		id: "intelligent",
		label: "Intelligent",
		detail: "The server-projected recommendation for this show.",
	},
];

/**
 * The documented default grouping. Members are matched by operator-facing label so the
 * preset survives attribute-id changes; an unlisted attribute keeps its own group.
 */
const ENCODER_GROUP_PRESET: ReadonlyArray<{
	label: string;
	members: readonly string[];
}> = [
	{ label: "Color", members: ["Color Mix", "Color Wheel 1", "Color Wheel 2"] },
	{
		label: "Position",
		members: ["Pan", "Tilt", "Position", "Position Movement"],
	},
	{ label: "Media Position", members: ["Media Position", "Rotation"] },
	{ label: "Scale", members: ["Scale X", "Scale Y"] },
	{
		label: "Prism / Animation",
		members: [
			"Prism 1",
			"Prism 1 Rotation",
			"Prism 2",
			"Prism 2 Rotation",
			"Animation Wheel",
			"Animation Rotation",
		],
	},
	{ label: "Media Mask", members: ["Invert Mask", "Media Mask Source"] },
];

function activeDescriptors(descriptors: ConfiguredAttributeDescriptor[]) {
	return descriptors.filter((descriptor) => !descriptor.retired);
}

function singleton(descriptor: ConfiguredAttributeDescriptor) {
	return {
		id: descriptor.id,
		label: descriptor.label,
		members: [descriptor.id],
	};
}

function byEncoderGroup(descriptors: ConfiguredAttributeDescriptor[]) {
	const active = activeDescriptors(descriptors);
	const claimed = new Set<string>();
	const groups = ENCODER_GROUP_PRESET.flatMap(({ label, members }) => {
		const matched = members.flatMap((name) => {
			const descriptor = active.find(
				(candidate) =>
					candidate.label.toLowerCase() === name.toLowerCase() &&
					!claimed.has(candidate.id),
			);
			if (!descriptor) return [];
			claimed.add(descriptor.id);
			return [descriptor.id];
		});
		return matched.length
			? [{ id: `activation.${label.toLowerCase().replace(/\W+/g, ".")}`, label, members: matched }]
			: [];
	});
	return [
		...groups,
		...active
			.filter((descriptor) => !claimed.has(descriptor.id))
			.map(singleton),
	];
}

/**
 * Presets are a starting point: they replace the activation groups outright, and the
 * operator edits the result afterwards.
 */
export function applyActivationPreset(
	preset: ActivationPreset,
	configuration: AttributeConfiguration,
	descriptors: ConfiguredAttributeDescriptor[],
	recommended: AttributeConfiguration["activation_groups"],
): AttributeConfiguration {
	const active = activeDescriptors(descriptors);
	if (preset === "none") return { ...configuration, activation_groups: [] };
	if (preset === "all")
		return {
			...configuration,
			activation_groups: active.length
				? [
						{
							id: "activation.all",
							label: "All attributes",
							members: active.map((descriptor) => descriptor.id),
						},
					]
				: [],
		};
	if (preset === "encoder-group")
		return { ...configuration, activation_groups: byEncoderGroup(descriptors) };
	return { ...configuration, activation_groups: recommended };
}

/** Removes one member, dropping the group once it would be empty. */
export function removeActivationMember(
	configuration: AttributeConfiguration,
	groupId: string,
	member: string,
): AttributeConfiguration {
	return {
		...configuration,
		activation_groups: configuration.activation_groups
			.map((group) =>
				group.id === groupId
					? {
							...group,
							members: group.members.filter(
								(candidate) => candidate !== member,
							),
						}
					: group,
			)
			.filter((group) => group.members.length),
	};
}

/** One attribute belongs to at most one activation group, so adding moves it. */
export function addActivationMember(
	configuration: AttributeConfiguration,
	groupId: string,
	member: string,
): AttributeConfiguration {
	return {
		...configuration,
		activation_groups: configuration.activation_groups
			.map((group) => {
				if (group.id === groupId)
					return group.members.includes(member)
						? group
						: { ...group, members: [...group.members, member] };
				return {
					...group,
					members: group.members.filter((candidate) => candidate !== member),
				};
			})
			.filter((group) => group.members.length),
	};
}

export function renameActivationGroup(
	configuration: AttributeConfiguration,
	groupId: string,
	label: string,
): AttributeConfiguration {
	return {
		...configuration,
		activation_groups: configuration.activation_groups.map((group) =>
			group.id === groupId ? { ...group, label } : group,
		),
	};
}

export function deleteActivationGroup(
	configuration: AttributeConfiguration,
	groupId: string,
): AttributeConfiguration {
	return {
		...configuration,
		activation_groups: configuration.activation_groups.filter(
			(group) => group.id !== groupId,
		),
	};
}
