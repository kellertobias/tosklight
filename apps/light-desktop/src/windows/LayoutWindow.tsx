import { useMemo, useRef, useState } from "react";
import { useShowObjectView } from "../features/showObjects/ShowObjectsView";
import { usePortableGroups } from "../features/showObjects/ShowObjectsState";
import { useBootstrapReady } from "../features/deskSnapshot/DeskSnapshotState";
import { useApp } from "../state/AppContext";
import type { SelectionGridPositions } from "./selectionGrid";
import { rowsFirst, selectionGridCells } from "./selectionGrid";
import { useStageLayout } from "./stageWindow/useStageLayout";
import { useStageSelection } from "./stageWindow/useStageSelection";
import { useLayoutVisualization } from "./layoutWindow/useLayoutVisualization";
import type { WindowProps } from "./windowTypes";

export function LayoutWindow({
	active = true,
	compact,
	paneId,
	layoutGroupId,
	viewOnly,
}: WindowProps) {
	const { state, dispatch } = useApp();
	const selectedGroupId = paneId ? layoutGroupId : state.layoutGroupId;
	useShowObjectView("group", active);
	const groups = usePortableGroups(active);
	const bootstrapReady = useBootstrapReady();
	const group = groups.find((candidate) => candidate.id === selectedGroupId);
	const layout = useStageLayout();
	const selection = useStageSelection(active && !viewOnly);
	const visualization = useLayoutVisualization(
		active,
		group?.body.fixtures ?? [],
	);
	const anchor = useRef<string | null>(null);
	const drag = useRef<{
		x: number;
		y: number;
		additive: boolean;
		pointerId: number;
	} | null>(null);
	const [marquee, setMarquee] = useState<{
		left: number;
		top: number;
		width: number;
		height: number;
	} | null>(null);
	const projection = useMemo(() => {
		const positions: SelectionGridPositions = {
			positions2d: { ...layout.positions },
			positions3d: { ...layout.positions3d },
		};
		for (const fixture of visualization.fixtures) {
			for (const head of fixture.logical_heads) {
				if (!positions.positions2d[head.fixture_id]) {
					const position = layout.positions[fixture.fixture_id];
					if (position)
						(positions.positions2d as Record<string, typeof position>)[
							head.fixture_id
						] = position;
				}
				if (!positions.positions3d[head.fixture_id]) {
					const position = layout.positions3d[fixture.fixture_id];
					if (position)
						(positions.positions3d as Record<string, typeof position>)[
							head.fixture_id
						] = position;
				}
			}
		}
		const cells = group
			? selectionGridCells(group.body.fixtures, group.body.grid, positions)
			: [];
		return {
			cells,
			order: rowsFirst(cells, "top_left"),
			columns: Math.max(1, ...cells.map((cell) => cell.column + 1)),
			rows: Math.max(1, ...cells.map((cell) => cell.row + 1)),
		};
	}, [
		group,
		layout.positions,
		layout.positions3d,
		visualization.fixtures,
	]);
	const presentations = useMemo(
		() =>
			new Map(
				visualization.presentations.map((fixture) => [
					fixture.fixtureId,
					fixture,
				]),
			),
		[visualization.presentations],
	);
	const inheritedPresentation = (fixtureId: string) => {
		const direct = presentations.get(fixtureId);
		if (direct) return direct;
		const owner = visualization.fixtures.find((fixture) =>
			fixture.logical_heads.some((head) => head.fixture_id === fixtureId),
		);
		return owner ? presentations.get(owner.fixture_id) : undefined;
	};
	const selectFixture = (
		fixtureId: string,
		event: React.MouseEvent<HTMLButtonElement>,
	) => {
		if (viewOnly) return;
		if (event.shiftKey && anchor.current) {
			const first = projection.order.indexOf(anchor.current);
			const last = projection.order.indexOf(fixtureId);
			if (first >= 0 && last >= 0) {
				const range = projection.order.slice(
					Math.min(first, last),
					Math.max(first, last) + 1,
				);
				void selection.replaceFixtureIds(
					event.metaKey || event.ctrlKey
						? orderedUnion(selection.fixtureIds, range)
						: range,
				);
				return;
			}
		}
		anchor.current = fixtureId;
		if (event.metaKey || event.ctrlKey)
			void selection.applyFixtureGesture(
				fixtureId,
				selection.fixtureIdSet.has(fixtureId) ? "remove" : "add",
			);
		else void selection.replaceFixtureIds([fixtureId]);
	};
	const beginMarquee = (event: React.PointerEvent<HTMLDivElement>) => {
		if (viewOnly || event.target !== event.currentTarget) return;
		const bounds = event.currentTarget.getBoundingClientRect();
		drag.current = {
			x: event.clientX,
			y: event.clientY,
			additive: event.metaKey || event.ctrlKey,
			pointerId: event.pointerId,
		};
		event.currentTarget.setPointerCapture(event.pointerId);
		setMarquee({
			left: event.clientX - bounds.left,
			top: event.clientY - bounds.top,
			width: 0,
			height: 0,
		});
	};
	const moveMarquee = (event: React.PointerEvent<HTMLDivElement>) => {
		const start = drag.current;
		if (!start || start.pointerId !== event.pointerId) return;
		const bounds = event.currentTarget.getBoundingClientRect();
		setMarquee({
			left: Math.min(start.x, event.clientX) - bounds.left,
			top: Math.min(start.y, event.clientY) - bounds.top,
			width: Math.abs(event.clientX - start.x),
			height: Math.abs(event.clientY - start.y),
		});
	};
	const finishMarquee = (event: React.PointerEvent<HTMLDivElement>) => {
		const start = drag.current;
		if (!start || start.pointerId !== event.pointerId) return;
		drag.current = null;
		setMarquee(null);
		const left = Math.min(start.x, event.clientX);
		const right = Math.max(start.x, event.clientX);
		const top = Math.min(start.y, event.clientY);
		const bottom = Math.max(start.y, event.clientY);
		const hitIds = new Set(
			[...event.currentTarget.querySelectorAll<HTMLElement>("[data-layout-fixture-id]")]
				.filter((element) => {
					const bounds = element.getBoundingClientRect();
					return (
						bounds.right >= left &&
						bounds.left <= right &&
						bounds.bottom >= top &&
						bounds.top <= bottom
					);
				})
				.map((element) => element.dataset.layoutFixtureId ?? ""),
		);
		const hits = projection.order.filter((fixtureId) => hitIds.has(fixtureId));
		if (hits.length === 0) return;
		anchor.current = hits.at(-1) ?? null;
		void selection.replaceFixtureIds(
			start.additive ? orderedUnion(selection.fixtureIds, hits) : hits,
		);
	};

	return (
		<section
			className={`layout-window ${compact ? "compact" : ""}`}
			aria-label="Layout"
		>
			{!compact && (
				<header className="layout-window-header">
					<label>
						Group
						<select
							value={selectedGroupId ?? ""}
							onChange={(event) =>
								dispatch({
									type: "SET_LAYOUT_GROUP",
									groupId: event.target.value,
								})
							}
						>
							<option value="">Choose a Group</option>
							{selectedGroupId &&
								!groups.some((candidate) => candidate.id === selectedGroupId) && (
									<option value={selectedGroupId}>
										Unavailable · {selectedGroupId}
									</option>
								)}
							{groups.map((candidate) => (
								<option key={candidate.id} value={candidate.id}>
									{candidate.id} ·{" "}
									{candidate.body.name || `Group ${candidate.id}`}
								</option>
							))}
						</select>
					</label>
				</header>
			)}
			{!selectedGroupId ? (
				<div className="layout-window-state" role="status">
					Choose a Group in Layout settings.
				</div>
			) : !bootstrapReady ? (
				<div className="layout-window-state" role="status">
					Loading Layout…
				</div>
			) : !group ? (
				<div className="layout-window-state missing" role="status">
					Group {selectedGroupId} is unavailable. Choose another Group in Layout
					settings.
				</div>
			) : projection.cells.length === 0 ? (
				<div className="layout-window-state" role="status">
					{group.body.name || `Group ${group.id}`} is empty.
				</div>
			) : (
				<div
					className="layout-grid"
					aria-label={`${group.body.name || `Group ${group.id}`} fixture layout`}
					style={{
						gridTemplateColumns: `repeat(${projection.columns}, minmax(5rem, 1fr))`,
						gridTemplateRows: `repeat(${projection.rows}, minmax(4rem, 1fr))`,
					}}
					onPointerDown={beginMarquee}
					onPointerMove={moveMarquee}
					onPointerUp={finishMarquee}
					onPointerCancel={() => {
						drag.current = null;
						setMarquee(null);
					}}
				>
					{projection.cells.map((cell) => {
						const fixture = inheritedPresentation(cell.fixtureId);
						const selected = selection.fixtureIdSet.has(cell.fixtureId);
						return (
							<button
								key={cell.fixtureId}
								type="button"
								className={`layout-cell ${selected ? "selected" : ""} ${fixture ? "" : "missing"}`}
								aria-pressed={selected}
								data-layout-fixture-id={cell.fixtureId}
								aria-label={
									fixture
										? `Fixture ${fixture.fixtureNumber}, ${fixture.dimmer}%`
										: `Unavailable fixture ${cell.fixtureId}`
								}
								style={{
									gridColumn: cell.column + 1,
									gridRow: cell.row + 1,
									"--layout-fixture-color": fixture?.color ?? "transparent",
								} as React.CSSProperties}
								onClick={(event) => selectFixture(cell.fixtureId, event)}
							>
								<strong>{fixture?.fixtureNumber ?? cell.fixtureId}</strong>
								<span className="layout-cell-level">
									{fixture ? `${fixture.dimmer}%` : "Unavailable"}
								</span>
								<span className="layout-cell-color" aria-hidden="true" />
							</button>
						);
					})}
					{marquee && (
						<span
							className="layout-marquee"
							aria-hidden="true"
							style={marquee}
						/>
					)}
				</div>
			)}
		</section>
	);
}

function orderedUnion(first: readonly string[], second: readonly string[]) {
	const seen = new Set(first);
	return [
		...first,
		...second.filter((fixtureId) => {
			if (seen.has(fixtureId)) return false;
			seen.add(fixtureId);
			return true;
		}),
	];
}
