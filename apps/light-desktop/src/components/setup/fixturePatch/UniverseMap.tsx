import { useEffect, useRef, useState } from "react";
import type { PatchedFixture } from "../../../api/types";
import { Button, Select } from "../../common";
import { conflicts, fixtureRanges } from "../patchUtils";

const DMX_GRID_COLUMNS = 16;

export function dmxGridSegments(
	start: number,
	end: number,
	columns = DMX_GRID_COLUMNS,
) {
	const segments: Array<{ row: number; column: number; length: number }> = [];
	let address = start;
	while (address <= end) {
		const row = Math.floor((address - 1) / columns) + 1;
		const column = ((address - 1) % columns) + 1;
		const length = Math.min(end - address + 1, columns - column + 1);
		segments.push({ row, column, length });
		address += length;
	}
	return segments;
}

export function draggedDmxStart(
	address: number,
	offset: number,
	footprint: number,
) {
	return Math.max(
		1,
		Math.min(512 - Math.max(1, footprint) + 1, address - offset),
	);
}

export type UniverseMapProposal = {
	key: string;
	start: number;
	footprint: number;
	label: string;
};

type UniverseMapProps = {
	fixtures: PatchedFixture[];
	universe: number;
	proposed: number;
	footprint: number;
	proposedLabel: string;
	proposals?: UniverseMapProposal[];
	onAddress: (address: number) => void;
	onProposalAddress?: (key: string, address: number) => void;
	onUniverse: (universe: number) => void;
};

type UniverseRange = {
	fixture: PatchedFixture;
	range: ReturnType<typeof fixtureRanges>[number];
	index: number;
};

export function UniverseMap(props: UniverseMapProps) {
	const model = useUniverseMapModel(props);
	return (
		<section className="universe-visual">
			<UniverseMapHeader
				universe={props.universe}
				onUniverse={props.onUniverse}
			/>
			<div className="dmx-address-grid-scroll">
				<UniverseGrid {...props} {...model} />
			</div>
		</section>
	);
}

function useUniverseMapModel(props: UniverseMapProps) {
	const displayedProposals = props.proposals?.length
		? props.proposals
		: props.proposed > 0
			? [
					{
						key: "primary",
						start: props.proposed,
						footprint: props.footprint,
						label: props.proposedLabel,
					},
				]
			: [];
	const [selectedProposal, setSelectedProposal] = useState(
		displayedProposals[0]?.key ?? "primary",
	);
	const drag = useRef<{ key: string; offset: number } | null>(null);
	useEffect(() => {
		if (
			displayedProposals.length &&
			!displayedProposals.some(
				(candidate) => candidate.key === selectedProposal,
			)
		)
			setSelectedProposal(displayedProposals[0].key);
	}, [displayedProposals, selectedProposal]);
	const ranges = universeRanges(props.fixtures, props.universe);
	return {
		displayedProposals,
		selectedProposal,
		setSelectedProposal,
		drag,
		ranges,
		ownersByAddress: ownersByAddress(ranges),
		proposalConflicts: proposalConflicts(
			displayedProposals,
			props.fixtures,
			props.universe,
		),
	};
}

function UniverseMapHeader({
	universe,
	onUniverse,
}: Pick<UniverseMapProps, "universe" | "onUniverse">) {
	return (
		<header>
			<div>
				<h3>Universe {universe}</h3>
				<small>
					Tap an address or drag each blue fixture patch individually.
				</small>
			</div>
			<Select
				value={universe}
				onChange={(event) => onUniverse(Number(event.target.value))}
			>
				{Array.from({ length: 32 }, (_, index) => (
					<option key={index + 1}>{index + 1}</option>
				))}
			</Select>
		</header>
	);
}

type UniverseGridProps = UniverseMapProps &
	ReturnType<typeof useUniverseMapModel>;

function UniverseGrid(props: UniverseGridProps) {
	const addressAtPointer = (event: React.PointerEvent) => {
		const target = document.elementFromPoint(
			event.clientX,
			event.clientY,
		) as HTMLElement | null;
		const cell = target?.closest("[data-dmx-address]") as HTMLElement | null;
		return cell ? Number(cell.dataset.dmxAddress) : null;
	};
	const moveProposal = (key: string, address: number) => {
		const candidate = props.displayedProposals.find((item) => item.key === key);
		if (!candidate) return;
		const next = draggedDmxStart(address, 0, candidate.footprint);
		if (props.onProposalAddress) props.onProposalAddress(key, next);
		else props.onAddress(next);
	};
	return (
		// biome-ignore lint/a11y/useSemanticElements: The interactive 512-slot control uses ARIA grid navigation semantics, not tabular data markup.
		<div
			className="dmx-address-grid"
			role="grid"
			aria-label={`DMX universe ${props.universe}`}
			onPointerMove={(event) => {
				if (!props.drag.current) return;
				const address = addressAtPointer(event);
				const candidate = props.displayedProposals.find(
					(item) => item.key === props.drag.current?.key,
				);
				if (address == null || !candidate) return;
				const next = draggedDmxStart(
					address,
					props.drag.current.offset,
					candidate.footprint,
				);
				if (props.onProposalAddress)
					props.onProposalAddress(candidate.key, next);
				else props.onAddress(next);
			}}
			onPointerUp={() => {
				props.drag.current = null;
			}}
			onPointerCancel={() => {
				props.drag.current = null;
			}}
		>
			<DmxAddressCells {...props} moveProposal={moveProposal} />
			<DmxRangeOverlays ranges={props.ranges} />
			<DmxProposalOverlays {...props} />
		</div>
	);
}

function DmxAddressCells(
	props: UniverseGridProps & {
		moveProposal: (key: string, address: number) => void;
	},
) {
	return Array.from({ length: 512 }, (_, index) => {
		const address = index + 1;
		const owners = props.ownersByAddress.get(address) ?? [];
		const proposedHere = props.displayedProposals.filter(
			(candidate) =>
				address >= candidate.start &&
				address <= candidate.start + candidate.footprint - 1,
		);
		const hasConflict = proposedHere.some((candidate) =>
			props.proposalConflicts.get(candidate.key),
		);
		const stateText = cellStateText(owners, proposedHere, hasConflict);
		return (
			<Button
				key={address}
				className={`dmx-address-cell${owners.length ? " used" : ""}${proposedHere.length ? (hasConflict ? " proposed conflict" : " proposed") : ""}`}
				style={{
					gridRow: Math.floor(index / DMX_GRID_COLUMNS) + 1,
					gridColumn: (index % DMX_GRID_COLUMNS) + 1,
				}}
				data-dmx-address={address}
				aria-label={`DMX address ${address}${stateText ? `, ${stateText}` : ""}`}
				role="gridcell"
				onClick={() => {
					if (!proposedHere.length)
						props.moveProposal(props.selectedProposal, address);
				}}
				onPointerDown={(event) => {
					const candidate = proposedHere[0];
					if (!candidate) return;
					props.setSelectedProposal(candidate.key);
					props.drag.current = {
						key: candidate.key,
						offset: address - candidate.start,
					};
					event.currentTarget.setPointerCapture?.(event.pointerId);
					event.preventDefault();
				}}
			>
				{address}
			</Button>
		);
	});
}

function DmxRangeOverlays({ ranges }: { ranges: UniverseRange[] }) {
	return ranges.flatMap(({ fixture, range, index }) =>
		dmxGridSegments(range.start, range.end).map((segment, segmentIndex) => (
			<div
				className="dmx-range-overlay used"
				key={`${fixture.fixture_id}-${index}-${segmentIndex}`}
				style={{
					gridRow: segment.row,
					gridColumn: `${segment.column} / span ${segment.length}`,
				}}
			>
				{segmentIndex === 0 && (
					<span>
						Fixture {fixture.fixture_number ?? "—"} ·{" "}
						{fixture.name || fixture.definition.name}
					</span>
				)}
			</div>
		)),
	);
}

function DmxProposalOverlays(props: UniverseGridProps) {
	return props.displayedProposals.flatMap((candidate) =>
		dmxGridSegments(
			candidate.start,
			Math.min(512, candidate.start + candidate.footprint - 1),
		).map((segment, segmentIndex) => (
			<div
				className={`dmx-range-overlay proposed${props.proposalConflicts.get(candidate.key) ? " conflict" : ""}${props.selectedProposal === candidate.key ? " selected" : ""}`}
				key={`${candidate.key}-${segmentIndex}`}
				style={{
					gridRow: segment.row,
					gridColumn: `${segment.column} / span ${segment.length}`,
				}}
			>
				{segmentIndex === 0 && <span>{candidate.label}</span>}
			</div>
		)),
	);
}

function universeRanges(fixtures: PatchedFixture[], universe: number) {
	return fixtures
		.flatMap((fixture) =>
			fixtureRanges(fixture).map((range, index) => ({ fixture, range, index })),
		)
		.filter((item) => item.range.universe === universe);
}

function ownersByAddress(ranges: UniverseRange[]) {
	const owners = new Map<number, UniverseRange[]>();
	for (const item of ranges)
		for (let address = item.range.start; address <= item.range.end; address++)
			owners.set(address, [...(owners.get(address) ?? []), item]);
	return owners;
}

function proposalConflicts(
	proposals: UniverseMapProposal[],
	fixtures: PatchedFixture[],
	universe: number,
) {
	return new Map(
		proposals.map((candidate) => {
			const end = candidate.start + candidate.footprint - 1;
			const overlapsBatch = proposals.some(
				(other) =>
					other.key !== candidate.key &&
					candidate.start <= other.start + other.footprint - 1 &&
					other.start <= end,
			);
			return [
				candidate.key,
				end > 512 ||
					conflicts(fixtures, universe, candidate.start, candidate.footprint)
						.length > 0 ||
					overlapsBatch,
			] as const;
		}),
	);
}

function cellStateText(
	owners: UniverseRange[],
	proposals: UniverseMapProposal[],
	hasConflict: boolean,
) {
	const ownerText = owners
		.map(
			({ fixture }) =>
				`Fixture ${fixture.fixture_number ?? "—"} ${fixture.name || fixture.definition.name}`,
		)
		.join(", ");
	const proposalText = proposals.map((candidate) => candidate.label).join(", ");
	return [
		ownerText && `used by ${ownerText}`,
		proposalText &&
			(hasConflict
				? `conflicting proposed patch for ${proposalText}`
				: `proposed patch for ${proposalText}`),
	]
		.filter(Boolean)
		.join(", ");
}
