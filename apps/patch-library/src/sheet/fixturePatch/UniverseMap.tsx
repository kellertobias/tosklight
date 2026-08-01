import { Button, SelectField } from "@tosklight/ui";
import { WindowScrollArea } from "@tosklight/ui/window-kit";
import {
	type CSSProperties,
	useEffect,
	useLayoutEffect,
	useRef,
	useState,
} from "react";
import type { PatchedFixture } from "../../wire";
import { conflicts, fixtureRanges } from "../patchUtils";

const DMX_GRID_COLUMNS = 16;
const DMX_GRID_MIN_CELL = 46;
const DMX_GRID_GAP = 2;
const DMX_GRID_HORIZONTAL_PADDING = 16;

export function dmxGridColumnCount(
	width: number,
	minimumCell = DMX_GRID_MIN_CELL,
	gap = DMX_GRID_GAP,
	horizontalPadding = DMX_GRID_HORIZONTAL_PADDING,
) {
	const available = Math.max(0, width - horizontalPadding);
	return Math.max(
		1,
		Math.min(512, Math.floor((available + gap) / (minimumCell + gap))),
	);
}

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
	selectedProposal?: string;
	onSelectedProposal?: (key: string) => void;
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
			<WindowScrollArea className="dmx-address-grid-scroll">
				<UniverseGrid {...props} {...model} />
			</WindowScrollArea>
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
	const [internalSelectedProposal, setInternalSelectedProposal] = useState(
		displayedProposals[0]?.key ?? "primary",
	);
	const selectedProposal = props.selectedProposal ?? internalSelectedProposal;
	const setSelectedProposal = (key: string) => {
		setInternalSelectedProposal(key);
		props.onSelectedProposal?.(key);
	};
	const drag = useRef<{ key: string; offset: number; moved: boolean } | null>(
		null,
	);
	const suppressNextClick = useRef(false);
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
		suppressNextClick,
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
			<SelectField
				ariaLabel="Universe"
				value={String(universe)}
				options={Array.from({ length: 32 }, (_, index) => ({
					value: String(index + 1),
					label: String(index + 1),
				}))}
				onChange={(value) => onUniverse(Number(value))}
			/>
		</header>
	);
}

type UniverseGridProps = UniverseMapProps &
	ReturnType<typeof useUniverseMapModel>;

function UniverseGrid(props: UniverseGridProps) {
	const grid = useRef<HTMLDivElement>(null);
	const [columns, setColumns] = useState(DMX_GRID_COLUMNS);
	useLayoutEffect(() => {
		const node = grid.current;
		if (!node) return;
		const measure = () => {
			if (node.clientWidth > 0)
				setColumns(dmxGridColumnCount(node.clientWidth));
		};
		measure();
		if (typeof ResizeObserver === "undefined") return;
		const observer = new ResizeObserver(measure);
		observer.observe(node);
		return () => observer.disconnect();
	}, []);
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
			ref={grid}
			className="dmx-address-grid"
			role="grid"
			aria-label={`DMX universe ${props.universe}`}
			data-dmx-columns={columns}
			style={
				{
					"--dmx-grid-columns": columns,
				} as CSSProperties
			}
			onClickCapture={(event) => {
				if (!props.suppressNextClick.current) return;
				props.suppressNextClick.current = false;
				event.preventDefault();
				event.stopPropagation();
			}}
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
				if (next !== candidate.start) props.drag.current.moved = true;
				if (props.onProposalAddress)
					props.onProposalAddress(candidate.key, next);
				else props.onAddress(next);
			}}
			onPointerUp={() => {
				if (props.drag.current?.moved) {
					props.suppressNextClick.current = true;
					window.setTimeout(() => {
						props.suppressNextClick.current = false;
					}, 0);
				}
				props.drag.current = null;
			}}
			onPointerCancel={() => {
				props.drag.current = null;
			}}
		>
			<DmxAddressCells
				{...props}
				columns={columns}
				moveProposal={moveProposal}
			/>
			<DmxRangeOverlays ranges={props.ranges} columns={columns} />
			<DmxProposalOverlays {...props} columns={columns} />
		</div>
	);
}

function DmxAddressCells(
	props: UniverseGridProps & {
		columns: number;
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
					gridRow: Math.floor(index / props.columns) + 1,
					gridColumn: (index % props.columns) + 1,
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
						moved: false,
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

function DmxRangeOverlays({
	ranges,
	columns,
}: {
	ranges: UniverseRange[];
	columns: number;
}) {
	return ranges.flatMap(({ fixture, range, index }) =>
		dmxGridSegments(range.start, range.end, columns).map(
			(segment, segmentIndex) => (
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
			),
		),
	);
}

function DmxProposalOverlays(
	props: UniverseGridProps & {
		columns: number;
	},
) {
	return props.displayedProposals.flatMap((candidate) =>
		dmxGridSegments(
			candidate.start,
			Math.min(512, candidate.start + candidate.footprint - 1),
			props.columns,
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
