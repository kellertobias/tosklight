import { Button } from "../../components/common";
import type { Group } from "./model";

function missingFixtureCount(
	group: Group | null,
	knownFixtureIds: Set<string>,
) {
	return (
		group?.body.fixtures.filter((fixture) => !knownFixtureIds.has(fixture))
			.length ?? 0
	);
}

function unsupportedValueCount(
	group: Group | null,
	attributes: string[],
	capabilities: Map<string, Set<string>>,
) {
	return (
		group?.body.fixtures.reduce(
			(count, fixture) =>
				count +
				attributes.filter(
					(attribute) =>
						capabilities.has(fixture) &&
						!capabilities.get(fixture)?.has(attribute),
				).length,
			0,
		) ?? 0
	);
}

function emptyGroupHint(storeArmed: boolean, updateArmed: boolean) {
	if (updateArmed) return "Touch to check Update eligibility";
	if (storeArmed) return "Tap to record empty group";
	return "Press Record to use this slot";
}

export function GroupCard({
	group,
	index,
	knownFixtureIds,
	capabilities,
	selected,
	storeArmed,
	updateArmed,
	beginHold,
	cancelHold,
	openContext,
	dereference,
	select,
}: {
	group: Group | null;
	index: number;
	knownFixtureIds: Set<string>;
	capabilities: Map<string, Set<string>>;
	selected: boolean;
	storeArmed: boolean;
	updateArmed: boolean;
	beginHold: () => void;
	cancelHold: () => void;
	openContext: () => void;
	dereference: () => void;
	select: () => void;
}) {
	const missing = missingFixtureCount(group, knownFixtureIds);
	const attributes = Object.keys(group?.body.programming ?? {});
	const unsupported = unsupportedValueCount(group, attributes, capabilities);
	return (
		<Button
			className={`group-card pool-cell ${group?.body.derived_from ? "derived" : ""} ${group?.body.frozen_from ? "frozen" : ""} ${selected ? "selected" : !group || !group.body.fixtures.length ? "empty" : ""} ${storeArmed && !group ? "store-target" : ""} ${updateArmed ? "update-target" : ""}`}
			style={group?.body.color ? { borderColor: group.body.color } : undefined}
			onPointerDown={beginHold}
			onPointerUp={cancelHold}
			onPointerCancel={cancelHold}
			onContextMenu={(event) => {
				event.preventDefault();
				openContext();
			}}
			onDoubleClick={dereference}
			onClick={select}
		>
			<span className="number">{index + 1}</span>
			{group ? (
				<GroupCardContent
					group={group}
					index={index}
					attributes={attributes}
					missing={missing}
					unsupported={unsupported}
					updateArmed={updateArmed}
				/>
			) : (
				<>
					<b>Empty</b>
					<small>{emptyGroupHint(storeArmed, updateArmed)}</small>
				</>
			)}
		</Button>
	);
}

function GroupCardContent({
	group,
	index,
	attributes,
	missing,
	unsupported,
	updateArmed,
}: {
	group: Group;
	index: number;
	attributes: string[];
	missing: number;
	unsupported: number;
	updateArmed: boolean;
}) {
	return (
		<>
			<b>{group.body.name ?? `Group ${index + 1}`}</b>
			<small>
				{updateArmed
					? "Touch to choose Update mode"
					: group.body.fixtures.length
						? `${group.body.fixtures.length} fixtures · ordered`
						: "⚠ Group is empty"}
			</small>
			{missing > 0 && <em>⚠ {missing} missing</em>}
			{attributes.length > 0 && (
				<em>{attributes.length} portable attributes</em>
			)}
			{unsupported > 0 && <em>⚠ {unsupported} unsupported values</em>}
			{group.body.derived_from && (
				<em>Derived · {group.body.derived_from.rule.type}</em>
			)}
			{group.body.frozen_from && (
				<em>Frozen · rev {group.body.frozen_from.source_revision}</em>
			)}
			{group.body.color && (
				<span
					className="group-color"
					title={`Color ${group.body.color}`}
					style={{ background: group.body.color }}
				/>
			)}
			{group.body.icon && (
				<span className="group-icon" title={`Icon ${group.body.icon}`}>
					{group.body.icon}
				</span>
			)}
		</>
	);
}
