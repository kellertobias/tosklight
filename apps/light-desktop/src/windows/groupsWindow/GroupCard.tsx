import { PoolCard } from "@tosklight/ui/pools";
import { useEffect, useRef } from "react";
import type { PoolPresentationConfiguration } from "../../api/types";
import { resolveConfiguredPoolPresentation } from "../../features/poolPresentation/poolPresentation";
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
	poolSlotId,
	knownFixtureIds,
	capabilities,
	selected,
	storeArmed,
	updateArmed,
	poolPresentation,
	showId,
	surfaceKey,
	beginHold,
	cancelHold,
	consumeHold,
	openSettings,
	dereference,
	select,
}: {
	group: Group | null;
	index: number;
	poolSlotId: string;
	knownFixtureIds: Set<string>;
	capabilities: Map<string, Set<string>>;
	selected: boolean;
	storeArmed: boolean;
	updateArmed: boolean;
	poolPresentation: PoolPresentationConfiguration;
	showId: string;
	surfaceKey: string;
	beginHold: () => void;
	cancelHold: () => void;
	consumeHold: () => boolean;
	openSettings: () => void;
	dereference: () => void;
	select: () => void;
}) {
	const clickTimer = useRef<number | null>(null);
	useEffect(
		() => () => {
			if (clickTimer.current !== null) window.clearTimeout(clickTimer.current);
		},
		[],
	);
	const scheduleLiveSelection = () => {
		if (consumeHold()) return;
		if (clickTimer.current !== null) window.clearTimeout(clickTimer.current);
		clickTimer.current = window.setTimeout(() => {
			clickTimer.current = null;
			select();
		}, 240);
	};
	const selectFrozen = () => {
		if (consumeHold()) return;
		if (clickTimer.current !== null) window.clearTimeout(clickTimer.current);
		clickTimer.current = null;
		dereference();
	};
	const missing = missingFixtureCount(group, knownFixtureIds);
	const attributes = Object.keys(group?.body.programming ?? {});
	const unsupported = unsupportedValueCount(group, attributes, capabilities);
	const presentation = resolveConfiguredPoolPresentation(poolPresentation, {
		showId,
		surfaceKey,
		objectType: "group",
		itemColorKey: group?.id,
		itemColor: group?.body.color,
		states: [
			...(selected ? (["selected"] as const) : []),
			...(!group || !group.body.fixtures.length ? (["empty"] as const) : []),
			...(storeArmed && !group ? (["record-target"] as const) : []),
			...(storeArmed && !group ? (["store-target"] as const) : []),
			...(updateArmed ? (["update-target"] as const) : []),
		],
	});
	const attributesLabel =
		attributes.length > 0 ? `${attributes.length} portable attributes` : null;
	const details = [
		missing > 0 ? `⚠ ${missing} missing` : null,
		attributesLabel,
		unsupported > 0 ? `⚠ ${unsupported} unsupported values` : null,
	].filter((detail): detail is string => Boolean(detail));
	return (
		<PoolCard
			data-pool-slot-id={poolSlotId}
			data-pool-position={index}
			className={`group-card ${presentation.className}`}
			style={presentation.style}
			aria-pressed={selected}
			model={{
				number: index + 1,
				primary: group?.body.name ?? (group ? `Group ${index + 1}` : "Empty"),
				secondary: group
					? updateArmed
						? "Touch to choose Update mode"
						: group.body.fixtures.length
							? `${group.body.fixtures.length} fixtures · ordered`
							: "Group is empty"
					: emptyGroupHint(storeArmed, updateArmed),
				details,
				icon: group?.body.icon,
				iconColor: group?.body.color,
				color: group?.body.color,
				kind: "group",
				states: presentation.states,
				derived: Boolean(group?.body.derived_from),
				derivedLabel: group?.body.derived_from
					? `Derived · ${group.body.derived_from.rule.type}`
					: undefined,
				frozen: Boolean(group?.body.frozen_from),
				frozenLabel: group?.body.frozen_from
					? `Frozen · rev ${group.body.frozen_from.source_revision}`
					: undefined,
			}}
			onPointerDown={beginHold}
			onPointerUp={cancelHold}
			onPointerCancel={cancelHold}
			onContextMenu={(event) => {
				event.preventDefault();
				if (clickTimer.current !== null)
					window.clearTimeout(clickTimer.current);
				clickTimer.current = null;
				openSettings();
			}}
			onDoubleClick={selectFrozen}
			onClick={scheduleLiveSelection}
		/>
	);
}
