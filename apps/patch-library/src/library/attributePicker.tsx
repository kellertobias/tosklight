import { Button, ModalRegistration, ModalTitleBar } from "@tosklight/ui";
import { useState } from "react";
import type { AttributeDescriptor } from "../wire";

/**
 * Choosing what a channel controls: the programmer tab, then the activation group, then the
 * attribute.
 *
 * The registry holds well over a hundred attributes, and a single list sorted by name puts Pan next
 * to Palette. The encoder tab (Intensity, Color, Position, Beam…) and the activation group inside
 * it are the groupings an operator already works in on the desk, so the choice narrows the same way
 * the programmer does.
 */

/** The value a channel's attribute takes when it outputs a fixed level and is never controlled. */
export const STATIC_ATTRIBUTE = "__static";

const ENCODER_LABELS: Record<string, string> = {
	intensity: "Intensity",
	color: "Color",
	position: "Position",
	beam: "Beam",
	shapers: "Shapers",
	focus: "Focus",
	control: "Control",
	media: "Media",
	custom: "Other",
};

type AttributeChoice = { id: string; label: string };
type ActivationGroup = { id: string; label: string; attributes: AttributeChoice[] };
type EncoderGroup = { id: string; label: string; groups: ActivationGroup[] };

function titleFromId(id: string) {
	return id
		.split(/[_.\-\s]+/u)
		.filter(Boolean)
		.map((word) => word[0].toUpperCase() + word.slice(1))
		.join(" ");
}

function push<T extends { id: string }>(list: T[], id: string, create: () => T) {
	const found = list.find((entry) => entry.id === id);
	if (found) return found;
	const made = create();
	list.push(made);
	return made;
}

/** Every attribute under its encoder tab and activation group, in registry order. */
export function attributeTree(
	registry: readonly AttributeDescriptor[],
	current?: string,
	includeStatic = true,
): EncoderGroup[] {
	const tree: EncoderGroup[] = [];
	for (const descriptor of registry) {
		if (descriptor.retired) continue;
		// A show may move an attribute to another tab; without a placement its family is the tab.
		const tab = descriptor.encoder_group ?? descriptor.family;
		const encoder = push(tree, tab, () => ({
			id: tab,
			label: ENCODER_LABELS[tab] ?? titleFromId(tab),
			groups: [],
		}));
		const groupId = descriptor.activation_group_id ?? `single:${descriptor.id}`;
		const group = push(encoder.groups, groupId, () => ({
			id: groupId,
			label: descriptor.activation_group_id
				? (descriptor.activation_group_label ??
					titleFromId(descriptor.activation_group_id))
				: descriptor.label,
			attributes: [],
		}));
		group.attributes.push({ id: descriptor.id, label: descriptor.label });
	}
	// A profile from another desk may name an attribute this registry does not know; keep it choosable.
	if (
		current &&
		current !== STATIC_ATTRIBUTE &&
		!registry.some((descriptor) => descriptor.id === current)
	)
		tree.push({
			id: "unknown",
			label: "Not in this registry",
			groups: [
				{ id: "unknown", label: current, attributes: [{ id: current, label: current }] },
			],
		});
	if (includeStatic)
		tree.push({
			id: "static",
			label: "Static",
			groups: [
				{
					id: "static",
					label: "Static",
					attributes: [{ id: STATIC_ATTRIBUTE, label: "Static output" }],
				},
			],
		});
	return tree;
}

function filterTree(tree: EncoderGroup[], needle: string) {
	if (!needle) return tree;
	return tree
		.map((encoder) => ({
			...encoder,
			groups: encoder.groups
				.map((group) => ({
					...group,
					attributes: group.attributes.filter((attribute) =>
						`${encoder.label} ${group.label} ${attribute.label} ${attribute.id}`
							.toLocaleLowerCase()
							.includes(needle),
					),
				}))
				.filter((group) => group.attributes.length),
		}))
		.filter((encoder) => encoder.groups.length);
}

function ChoiceColumn<T extends { id: string; label: string }>({
	label,
	entries,
	activeId,
	detail,
	onChoose,
}: {
	label: string;
	entries: readonly T[];
	activeId: string | undefined;
	detail: (entry: T) => string | number | null;
	onChoose: (entry: T) => void;
}) {
	return (
		<div role="listbox" aria-label={label}>
			{entries.map((entry) => {
				const extra = detail(entry);
				return (
					<Button
						key={entry.id}
						role="option"
						aria-selected={entry.id === activeId}
						className={entry.id === activeId ? "is-active" : undefined}
						onClick={() => onChoose(entry)}
					>
						<span>{entry.label}</span>
						{extra != null && <small>{extra}</small>}
					</Button>
				);
			})}
		</div>
	);
}

export function AttributePickerModal({
	title,
	value,
	registry,
	includeStatic = true,
	onSelect,
	onClose,
}: {
	title: string;
	/** The chosen attribute id, or {@link STATIC_ATTRIBUTE}. */
	value: string;
	registry: readonly AttributeDescriptor[];
	/** Whether Static output is a choice; it is for a channel, not for what moves a part. */
	includeStatic?: boolean;
	onSelect: (attribute: string) => void;
	onClose: () => void;
}) {
	const tree = attributeTree(registry, value, includeStatic);
	const [query, setQuery] = useState("");
	const shown = filterTree(tree, query.trim().toLocaleLowerCase());
	const holding = (encoder: EncoderGroup) =>
		encoder.groups.find((group) =>
			group.attributes.some((attribute) => attribute.id === value),
		);
	const [encoderId, setEncoderId] = useState(
		() => tree.find((encoder) => holding(encoder))?.id ?? tree[0]?.id,
	);
	const [groupId, setGroupId] = useState(
		() => tree.map(holding).find(Boolean)?.id,
	);
	const encoder = shown.find((entry) => entry.id === encoderId) ?? shown[0];
	const group =
		encoder?.groups.find((entry) => entry.id === groupId) ?? encoder?.groups[0];
	return (
		<ModalRegistration onClose={onClose}>
			<div
				className="stacked-modal-layer fixture-attribute-picker-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && onClose()
				}
			>
				<section
					className="nested-modal fixture-attribute-picker"
					role="dialog"
					aria-modal="true"
					aria-label={title}
				>
					<ModalTitleBar
						title={title}
						search={{
							value: query,
							onSearch: setQuery,
							ariaLabel: "Search attributes",
							placeholder: "Search attributes",
						}}
						closeLabel="Close attribute picker"
						onClose={onClose}
					/>
					<div className="fixture-attribute-picker-columns">
						<ChoiceColumn
							label="Encoder groups"
							entries={shown}
							activeId={encoder?.id}
							detail={(entry) =>
								entry.groups.reduce((count, item) => count + item.attributes.length, 0)
							}
							onChoose={(entry) => {
								setEncoderId(entry.id);
								setGroupId(undefined);
							}}
						/>
						<ChoiceColumn
							label="Activation groups"
							entries={encoder?.groups ?? []}
							activeId={group?.id}
							detail={(entry) => entry.attributes.length}
							onChoose={(entry) => setGroupId(entry.id)}
						/>
						<div className="fixture-attribute-picker-last">
							<ChoiceColumn
								label={group ? `${group.label} attributes` : "Attributes"}
								entries={group?.attributes ?? []}
								activeId={value}
								detail={(entry) => (entry.id === STATIC_ATTRIBUTE ? null : entry.id)}
								onChoose={(entry) => onSelect(entry.id)}
							/>
							{!shown.length && (
								<p role="status">No attribute matches this search.</p>
							)}
						</div>
					</div>
				</section>
			</div>
		</ModalRegistration>
	);
}
