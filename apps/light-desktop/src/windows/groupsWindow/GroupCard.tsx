import { PoolCard } from "@tosklight/ui/pools";
import { useEffect, useRef } from "react";
import type {
	GroupReference,
	PoolPresentationConfiguration,
} from "../../api/types";
import type { PoolMutationOperation } from "../../features/controlSurfaceInteraction/poolCommandTarget";
import { resolveConfiguredPoolPresentation } from "../../features/poolPresentation/poolPresentation";
import type { StagedSelectionWrite } from "../../features/programmingInteraction/selectionWriter";
import { canonicalGroupSource } from "../../features/showObjects/groupProjection";
import { groupFixtureCountLabel } from "./groupFixtureCount";
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

export function groupReferencePresentation(references: readonly GroupReference[]) {
	const parts = references.map(({ group_id, rule }) => {
		const n = rule.type === "every_nth" ? rule.n : 2;
		const offset =
			rule.type === "every_nth" ? rule.offset : rule.type === "even" ? 1 : 0;
		return {
			compact:
				rule.type === "all"
					? group_id
					: `${group_id} % ${n}${offset ? ` + ${offset}` : ""}`,
			description:
				rule.type === "all"
					? `Group ${group_id}, all fixtures`
					: `Group ${group_id}, every ${n} fixtures starting at position ${offset + 1}`,
		};
	});
	return {
		compact: parts.map((part) => part.compact).join(", "),
		description: parts.map((part) => part.description).join("; "),
	};
}

function legacyGroupReference(group: Group | null): GroupReference[] {
	const derived = group?.body.derived_from;
	if (!derived) return [];
	const { type, n, offset } = derived.rule;
	if (type === "all" || type === "odd" || type === "even") {
		return [{ group_id: derived.source_group_id, rule: { type } }];
	}
	if (
		type === "every_nth" &&
		typeof n === "number" &&
		Number.isInteger(n) &&
		n > 0 &&
		typeof offset === "number" &&
		Number.isInteger(offset) &&
		offset >= 0
	) {
		return [{ group_id: derived.source_group_id, rule: { type, n, offset } }];
	}
	return [];
}

function groupReferenceModel(group: Group | null) {
	const canonicalSource = group ? canonicalGroupSource(group.body) : null;
	const presentationReferences =
		canonicalSource?.type === "references"
			? canonicalSource.references
			: canonicalSource
				? []
				: legacyGroupReference(group);
	const referencePresentation = groupReferencePresentation(presentationReferences);
	return {
		derived: presentationReferences.length > 0,
		derivedLabel:
			presentationReferences.length > 0 ? (
				<span
					className="group-reference-construction"
					title={referencePresentation.description}
				>
					<svg
						aria-hidden="true"
						width="11"
						height="11"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						strokeWidth="2"
					>
						<path d="M10 13a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-2 2M14 11a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l2-2" />
					</svg>
					<span className="group-reference-expression">
						{referencePresentation.compact}
					</span>
				</span>
			) : undefined,
		derivedDescription:
			presentationReferences.length > 0
				? referencePresentation.description
				: undefined,
	};
}

export function GroupCard({
	group,
	index,
	poolSlotId,
	knownFixtureIds,
	capabilities,
	selected,
	partiallySelected,
	fullySelected = false,
	selectedFixtureCount,
	storeArmed,
	recordLabel,
	updateArmed,
	setTarget,
	mutationOperation,
	poolPresentation,
	showId,
	surfaceKey,
	beginHold,
	cancelHold,
	consumeHold,
	openSettings,
	dereference,
	select,
	stageSelect,
}: {
	group: Group | null;
	index: number;
	poolSlotId: string;
	knownFixtureIds: Set<string>;
	capabilities: Map<string, Set<string>>;
	selected: boolean;
	partiallySelected: boolean;
	fullySelected?: boolean;
	selectedFixtureCount: number;
	storeArmed: boolean;
	recordLabel?: string;
	updateArmed: boolean;
	setTarget: boolean;
	mutationOperation: PoolMutationOperation | null;
	poolPresentation: PoolPresentationConfiguration;
	showId: string;
	surfaceKey: string;
	beginHold: () => void;
	cancelHold: () => void;
	consumeHold: () => boolean;
	openSettings: () => void;
	dereference: () => void;
	select: () => void;
	stageSelect?: () => StagedSelectionWrite | null;
}) {
	const { scheduleLiveSelection, selectFrozen, cancelSelection } =
		useGroupClickSelection({ stageSelect, consumeHold, select, dereference });
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
			...(storeArmed ? (["record-target"] as const) : []),
			...(storeArmed ? (["store-target"] as const) : []),
			...(updateArmed ? (["update-target"] as const) : []),
			...(setTarget ? (["set-target"] as const) : []),
			...(mutationOperation ? ([`${mutationOperation}-target`] as const) : []),
		],
	});
	const details = groupDetails(attributes, missing, unsupported);
	return (
		<PoolCard
			data-pool-slot-id={poolSlotId}
			data-group-selection={
				selected
					? "group"
					: fullySelected
						? "full"
						: partiallySelected
							? "partial"
							: "none"
			}
			data-group-membership={
				fullySelected ? "full" : partiallySelected ? "partial" : "none"
			}
			data-pool-position={index}
			className={`group-card ${presentation.className}`}
			style={presentation.style}
			aria-pressed={selected}
			model={{
				number: index + 1,
				primary: group?.body.name ?? (group ? `Group ${index + 1}` : "Empty"),
				secondary: group ? (
					<GroupFixtureCount
						total={group.body.fixtures.length}
						selected={selectedFixtureCount}
					/>
				) : undefined,
				details: group
					? details
					: [...details, emptyGroupHint(storeArmed, updateArmed)],
				icon: group?.body.icon,
				iconColor: group?.body.color,
				color: group?.body.color,
				kind: "group",
				workflow: storeArmed
					? (recordLabel ?? (group?.body.fixtures.length ? "REC MRG" : "REC"))
					: undefined,
				states: presentation.states,
				...groupReferenceModel(group),
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
				cancelSelection();
				openSettings();
			}}
			onDoubleClick={selectFrozen}
			onClick={scheduleLiveSelection}
		/>
	);
}

function GroupFixtureCount({
	total,
	selected,
}: {
	total: number;
	selected: number;
}) {
	const partial = selected > 0 && selected < total;
	return (
		<span className="group-fixture-count">
			<span
				className={selected > 0 ? "group-fixture-count-selected" : undefined}
			>
				{partial ? selected : groupFixtureCountLabel(total, selected)}
			</span>
			{partial ? `/${total}` : null}
		</span>
	);
}

function groupDetails(
	attributes: string[],
	missing: number,
	unsupported: number,
) {
	return [
		missing > 0 ? `⚠ ${missing} missing` : null,
		attributes.length > 0 ? `${attributes.length} portable attributes` : null,
		unsupported > 0 ? `⚠ ${unsupported} unsupported values` : null,
	].filter((detail): detail is string => Boolean(detail));
}

function useGroupClickSelection({
	stageSelect,
	consumeHold,
	select,
	dereference,
}: {
	stageSelect?: () => StagedSelectionWrite | null;
	consumeHold(): boolean;
	select(): void;
	dereference(): void;
}) {
	const clickTimer = useRef<number | null>(null);
	const staged = useRef<StagedSelectionWrite | null>(null);
	useEffect(
		() => () => {
			if (clickTimer.current !== null) window.clearTimeout(clickTimer.current);
			staged.current?.cancel();
		},
		[],
	);
	const scheduleLiveSelection = () => {
		if (consumeHold()) return;
		if (clickTimer.current !== null) window.clearTimeout(clickTimer.current);
		staged.current?.cancel();
		const pending = stageSelect?.() ?? null;
		staged.current = pending;
		clickTimer.current = window.setTimeout(() => {
			clickTimer.current = null;
			staged.current = null;
			if (pending) pending.commit();
			else select();
		}, 240);
	};
	const selectFrozen = () => {
		if (consumeHold()) return;
		if (clickTimer.current !== null) window.clearTimeout(clickTimer.current);
		clickTimer.current = null;
		staged.current?.cancel();
		staged.current = null;
		dereference();
	};

	const cancelSelection = () => {
		if (clickTimer.current !== null) window.clearTimeout(clickTimer.current);
		clickTimer.current = null;
		staged.current?.cancel();
		staged.current = null;
	};
	return { scheduleLiveSelection, selectFrozen, cancelSelection };
}
