import {
	draggedDmxStart,
	type PatchFixtureProjection,
	type PatchFixtureWrite,
	type PatchProfileRevision,
	type PatchSplitAssignment,
} from "@tosklight/patch";
import { Button, SwitchField, type TitleActionGroup } from "@tosklight/ui";
import {
	WindowHeader,
	WindowScrollArea,
	WindowSettings,
} from "@tosklight/ui/window-kit";
import { useEffect, useMemo, useRef, useState } from "react";
import {
	DMX_SLOTS,
	type DmxOccupancy,
	type DmxOccupant,
	dmxChannelsPerRow,
	dmxOccupancy,
	fixtureTitle,
	patchedUniverses,
} from "./dmxChannels";
import {
	type Channel,
	ChannelInfo,
	classes,
	gridStyle,
	occupantLabel,
	PatchSummary,
	rows,
	sameFixture,
	useWidth,
} from "./DmxWorkspace";
import { beginTitleBarDrag } from "./WindowChrome";

/** One dragged patch: where one split of a fixture, or of one of its multi-patch copies, now starts. */
interface PatchMove {
	fixtureId: string;
	instance: number | null;
	split: number;
	universe: number;
	address: number;
}

type PatchMoves = ReadonlyMap<string, PatchMove>;

/** A patch cell carries its address, so it is sized to read one. */
const PATCH_CELL = 34;
const SIDEBAR_KEY = "tosklight.architect.dmx-patch-sidebar";

/**
 * The Patch screen's DMX tab: every channel of every patched universe, lit where the rig occupies
 * it and dark where not. Its only window setting is whether the side column is shown.
 *
 * Dragging a fixture's block to other addresses repatches it. The move is a proposal until
 * **Apply Patch** in the side column writes it, so dragging needs the side column.
 */
export function DmxPatchScreen({
	pages,
	fixtures,
	profileRevisions,
	onApplyPatch,
}: {
	/** The Patch screen's Sheet and DMX tabs, directly left of the settings. */
	pages: TitleActionGroup;
	fixtures: readonly PatchFixtureProjection[];
	profileRevisions: readonly PatchProfileRevision[];
	/** Writes the moved fixtures as one patch change, resolving once the rig reads them back. */
	onApplyPatch: (fixtures: readonly PatchFixtureWrite[]) => Promise<void>;
}) {
	const [sidebar, setSidebar] = useStoredFlag(SIDEBAR_KEY, true);
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
	const [moves, setMoves] = useState<PatchMoves>(new Map());
	const shown = useMemo(
		() =>
			fixtures.map((fixture) =>
				withMoves(fixture, movesOf(moves, fixture.fixtureId)),
			),
		[fixtures, moves],
	);
	const occupancy = useMemo(
		() => dmxOccupancy(shown, profileRevisions),
		[shown, profileRevisions],
	);
	return (
		<section className="viz-dmx-workspace viz-dmx-patch-screen">
			<WindowHeader
				title="Patch"
				dragHandleProps={{
					"data-tauri-drag-region": true,
					onPointerDown: beginTitleBarDrag,
				}}
				groups={[pages]}
				settings
				onSettings={(anchor) =>
					setSettingsAnchor((open) =>
						open ? null : anchor.getBoundingClientRect(),
					)
				}
			/>
			{settingsAnchor ? (
				<WindowSettings
					modal={false}
					anchor={settingsAnchor}
					title="DMX"
					onClose={() => setSettingsAnchor(null)}
					tabs={[
						{
							id: "display",
							label: "Display",
							content: (
								<SwitchField
									label="Show sidebar"
									offLabel={null}
									onLabel={null}
									checked={sidebar}
									onChange={(event) => setSidebar(event.target.checked)}
								/>
							),
						},
					]}
				/>
			) : null}
			<DmxPatchView
				occupancy={occupancy}
				fixtures={fixtures}
				sidebar={sidebar}
				moves={moves}
				onMoves={setMoves}
				onApplyPatch={onApplyPatch}
			/>
		</section>
	);
}

/** Every channel of every patched universe: lit where the rig occupies it, dark where not. */
function DmxPatchView({
	occupancy,
	fixtures,
	sidebar,
	moves,
	onMoves,
	onApplyPatch,
}: {
	occupancy: DmxOccupancy;
	/** The rig as stored, before any move proposed here. */
	fixtures: readonly PatchFixtureProjection[];
	sidebar: boolean;
	moves: PatchMoves;
	onMoves: (moves: PatchMoves) => void;
	onApplyPatch: (fixtures: readonly PatchFixtureWrite[]) => Promise<void>;
}) {
	const [selected, setSelected] = useState<Channel | null>(null);
	const [applying, setApplying] = useState(false);
	const [applyError, setApplyError] = useState("");
	// The block being dragged, and which of its channels the pointer took it by.
	const drag = useRef<{ occupant: DmxOccupant; offset: number } | null>(null);
	useDragRelease(drag);
	const host = useRef<HTMLElement>(null);
	const columns = dmxChannelsPerRow(useWidth(host), PATCH_CELL);
	const patched = patchedUniverses(occupancy);
	const universes = patched.length ? patched : [1];

	function dragTo(universe: number, address: number) {
		const held = drag.current;
		if (!held) return;
		const { occupant } = held;
		const start = draggedDmxStart(address, held.offset, occupant.footprint);
		onMoves(movedTo(moves, fixtures, occupant, universe, start));
		setSelected({ universe, address: start });
		setApplyError("");
	}

	async function apply() {
		setApplying(true);
		setApplyError("");
		try {
			await onApplyPatch(movedWrites(fixtures, moves));
			onMoves(new Map());
		} catch (reason) {
			setApplyError(String(reason));
		} finally {
			setApplying(false);
		}
	}

	return (
		<div className={`viz-dmx-content${sidebar ? "" : " is-without-sidebar"}`}>
			<WindowScrollArea>
				<main ref={host} style={gridStyle(columns, PATCH_CELL)}>
					{universes.map((universe) => {
						const addresses = occupancy.get(universe);
						return (
							<section
								className="viz-dmx-universe is-patch"
								key={universe}
								aria-label={`Universe ${universe} patch`}
							>
								<header>
									<b>Universe {universe}</b>
									<small>
										{addresses?.size ?? 0} of {DMX_SLOTS} channels patched
									</small>
								</header>
								{rows(columns, (address) => (
									<PatchCell
										key={address}
										universe={universe}
										address={address}
										addresses={addresses}
										columns={columns}
										draggable={sidebar}
										moves={moves}
										selected={
											selected?.universe === universe &&
											selected.address === address
										}
										onGrab={(occupant) => {
											drag.current = { occupant, offset: occupant.channel - 1 };
										}}
										onEnter={() => dragTo(universe, address)}
										onSelect={() => setSelected({ universe, address })}
									/>
								))}
							</section>
						);
					})}
					{patched.length ? null : (
						<p className="viz-dmx-empty">
							No fixture is patched to a DMX address yet.
						</p>
					)}
				</main>
			</WindowScrollArea>
			{sidebar ? (
				<aside className="viz-dmx-info">
					{moves.size ? (
						<PendingPatch
							moves={moves}
							fixtures={fixtures}
							applying={applying}
							error={applyError}
							onApply={() => void apply()}
							onDiscard={() => {
								onMoves(new Map());
								setApplyError("");
							}}
						/>
					) : null}
					{selected ? (
						<ChannelInfo
							channel={selected}
							occupants={
								occupancy.get(selected.universe)?.get(selected.address) ?? []
							}
							onDeselect={() => setSelected(null)}
						/>
					) : (
						<PatchSummary occupancy={occupancy} />
					)}
				</aside>
			) : null}
		</div>
	);
}

/** One address of one universe: lit where the rig occupies it, and a handle on its fixture's block. */
function PatchCell({
	universe,
	address,
	addresses,
	columns,
	draggable,
	moves,
	selected,
	onGrab,
	onEnter,
	onSelect,
}: {
	universe: number;
	address: number;
	addresses: ReadonlyMap<number, DmxOccupant[]> | undefined;
	columns: number;
	/** Whether a patched cell can be taken to move its block; only with the sidebar shown. */
	draggable: boolean;
	moves: PatchMoves;
	selected: boolean;
	onGrab: (occupant: DmxOccupant) => void;
	onEnter: () => void;
	onSelect: () => void;
}) {
	const occupants = addresses?.get(address) ?? [];
	const grabbable = draggable && occupants.length > 0;
	return (
		<button
			type="button"
			onPointerDown={
				grabbable
					? (event) => {
							onGrab(occupants[0]);
							// A touch captures the pointer to the cell it began on, which would keep every
							// other cell from hearing the drag arrive.
							const cell = event.currentTarget;
							if (cell.hasPointerCapture?.(event.pointerId))
								cell.releasePointerCapture(event.pointerId);
						}
					: undefined
			}
			onPointerEnter={onEnter}
			className={classes({
				"is-draggable": grabbable,
				"is-moved": occupants.some((occupant) =>
					moves.has(
						moveKey(occupant.fixture.fixtureId, occupant.instance, occupant.split),
					),
				),
				"is-patched": occupants.length > 0,
				"is-start": occupants.some((occupant) => occupant.channel === 1),
				"is-conflict": occupants.length > 1,
				"joins-prev": sameFixture(occupants, addresses?.get(address - 1)),
				"joins-next": sameFixture(occupants, addresses?.get(address + 1)),
				// A row's last cell continues on the next row: no gap to bridge.
				"bridges-next":
					address % columns !== 0 &&
					sameFixture(occupants, addresses?.get(address + 1)),
				"is-selected": selected,
			})}
			aria-pressed={selected}
			aria-label={`Universe ${universe}, address ${address}, ${
				occupants.length
					? occupants.map(occupantLabel).join("; ")
					: "not patched"
			}`}
			title={
				occupants.length ? occupants.map(occupantLabel).join("\n") : undefined
			}
			onClick={onSelect}
		>
			{address}
		</button>
	);
}

/** The moves dragged but not yet written, and the action that writes them. */
function PendingPatch({
	moves,
	fixtures,
	applying,
	error,
	onApply,
	onDiscard,
}: {
	moves: PatchMoves;
	fixtures: readonly PatchFixtureProjection[];
	applying: boolean;
	error: string;
	onApply: () => void;
	onDiscard: () => void;
}) {
	return (
		<section className="viz-dmx-pending" aria-label="Pending patch">
			<b>Pending patch</b>
			<ul className="viz-dmx-list">
				{[...moves.entries()].map(([key, move]) => {
					const fixture = fixtures.find(
						(candidate) => candidate.fixtureId === move.fixtureId,
					);
					const from = fixture
						? storedPatch(fixture, move.instance, move.split)
						: undefined;
					const copy =
						move.instance == null
							? ""
							: ` · ${fixture?.multipatch[move.instance]?.name.trim() || `Multi-patch ${move.instance + 1}`}`;
					return (
						<li key={key}>
							<span>
								{fixture ? fixtureTitle(fixture) : "Fixture"}
								{copy}
								{move.split === 1 ? "" : ` · split ${move.split}`}
							</span>
							<small>
								{from?.universe != null && from.address != null
									? `${from.universe}.${from.address}`
									: "Unpatched"}{" "}
								→ {move.universe}.{move.address}
							</small>
						</li>
					);
				})}
			</ul>
			{error ? (
				<output className="viz-dmx-error" role="alert">
					{error}
				</output>
			) : null}
			<div className="viz-dmx-pending-actions">
				<Button size="compact" disabled={applying} onClick={onDiscard}>
					Discard
				</Button>
				<Button size="compact" disabled={applying} onClick={onApply}>
					{applying ? "Applying…" : "Apply Patch"}
				</Button>
			</div>
		</section>
	);
}

/** A drag ends wherever the pointer is let go, on a cell or not. */
function useDragRelease(drag: { current: unknown }) {
	useEffect(() => {
		const release = () => {
			drag.current = null;
		};
		window.addEventListener("pointerup", release);
		window.addEventListener("pointercancel", release);
		return () => {
			window.removeEventListener("pointerup", release);
			window.removeEventListener("pointercancel", release);
		};
	}, [drag]);
}

function moveKey(fixtureId: string, instance: number | null, split: number) {
	return `${fixtureId}:${instance ?? "fixture"}:${split}`;
}

function movesOf(moves: PatchMoves, fixtureId: string) {
	return [...moves.values()].filter((move) => move.fixtureId === fixtureId);
}

/** The moves with the occupant's block starting at `address`; back where it is stored, it is no move. */
function movedTo(
	moves: PatchMoves,
	fixtures: readonly PatchFixtureProjection[],
	occupant: DmxOccupant,
	universe: number,
	address: number,
): PatchMoves {
	const { fixtureId } = occupant.fixture;
	const key = moveKey(fixtureId, occupant.instance, occupant.split);
	const stored = fixtures.find((fixture) => fixture.fixtureId === fixtureId);
	const original = stored
		? storedPatch(stored, occupant.instance, occupant.split)
		: undefined;
	const next = new Map(moves);
	if (original?.universe === universe && original.address === address)
		next.delete(key);
	else
		next.set(key, {
			fixtureId,
			instance: occupant.instance,
			split: occupant.split,
			universe,
			address,
		});
	return next;
}

/** Every fixture a move touches, as the patch change that writes it. */
function movedWrites(
	fixtures: readonly PatchFixtureProjection[],
	moves: PatchMoves,
): PatchFixtureWrite[] {
	return fixtures
		.filter((fixture) => movesOf(moves, fixture.fixtureId).length)
		.map((fixture) =>
			patchWrite(withMoves(fixture, movesOf(moves, fixture.fixtureId))),
		);
}

/** Where one split of the fixture, or of one of its copies, is patched in the stored rig. */
function storedPatch(
	fixture: PatchFixtureProjection,
	instance: number | null,
	split: number,
) {
	const splits =
		instance == null
			? fixture.splitPatches
			: (fixture.multipatch[instance]?.splitPatches ?? []);
	return splits.find((patch) => patch.split === split);
}

/** The fixture with each move's split patched where the move put it. */
function withMoves<T extends PatchFixtureWrite>(
	fixture: T,
	moves: readonly PatchMove[],
): T {
	if (!moves.length) return fixture;
	const place = (
		splits: readonly PatchSplitAssignment[],
		instance: number | null,
	) => {
		const next = [...splits];
		for (const move of moves) {
			if (move.instance !== instance) continue;
			const patch = {
				split: move.split,
				universe: move.universe,
				address: move.address,
			};
			const index = next.findIndex((candidate) => candidate.split === move.split);
			if (index >= 0) next[index] = patch;
			else next.push(patch);
		}
		return next;
	};
	return {
		...fixture,
		splitPatches: place(fixture.splitPatches, null),
		multipatch: fixture.multipatch.map((copy, index) => ({
			...copy,
			splitPatches: place(copy.splitPatches, index),
		})),
	};
}

/** What a patch change writes: the fixture as the rig projects it, less what only the rig derives. */
function patchWrite(fixture: PatchFixtureProjection): PatchFixtureWrite {
	const {
		fixtureRevision: _revision,
		logicalHeads: _logicalHeads,
		...write
	} = fixture;
	return write;
}

/** A window preference kept on this machine, not in the show. */
function useStoredFlag(
	key: string,
	fallback: boolean,
): [boolean, (value: boolean) => void] {
	const [value, setValue] = useState(() => {
		try {
			const stored = localStorage.getItem(key);
			return stored == null ? fallback : stored === "true";
		} catch {
			return fallback;
		}
	});
	return [
		value,
		(next) => {
			setValue(next);
			try {
				localStorage.setItem(key, String(next));
			} catch {
				// A window that cannot store it still shows the choice made now.
			}
		},
	];
}
