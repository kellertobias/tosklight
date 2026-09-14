import { Button } from "@tosklight/ui";
import {
	type CSSProperties,
	Fragment,
	type KeyboardEvent,
	type MouseEvent as ReactMouseEvent,
	type ReactNode,
	useEffect,
	useLayoutEffect,
	useRef,
} from "react";
import type { MultiPatchInstance, PatchedFixture } from "../../wire";
import { isDmxPatchable } from "../patchUtils";
import { usePatchController } from "./controller";
import { saveEdit } from "./editSave";
import {
	armEdit,
	cancelEdit,
	requestFixtureEditClose,
	selectSplitAddress,
} from "./editSession";
import {
	selectContextFixture,
	selectFixtureRange,
	selectPatchFixture,
} from "./fixtureActions";
import { FixtureTypeIcon, MultiPatchBranch } from "./fixtureDisplay";
import { fixtureDisplayId } from "./fixtureIds";
import { beginMultipatchEdit } from "./multipatchActions";
import { PATCH_SHEET_COLUMNS, type PatchSheetColumn } from "./patchColumns";
import { placedSceneryMetres, SCENERY_AXES, sceneryOf } from "./scenerySize";
import {
	chainModeLabel,
	chainModeOf,
	isChain,
	SCENERY_OPTION_COLUMNS,
	sceneryOptionsOf,
} from "./sceneryOptions";
import { formatMib, mastersValue } from "./policyValues";
import { revealPatchRow } from "./revealRow";
import { isPatchSortColumn, nextPatchSort } from "./tableSort";
import {
	definitionSplits,
	effectiveSplitPatches,
	fixturePolicyApplicability,
	formatFixturePatch,
	formatInstancePatch,
} from "./patchModel";

/** Draws a column's cell only while the operator shows that column. */
function Shown({
	column,
	children,
}: {
	column: PatchSheetColumn;
	children: ReactNode;
}) {
	return usePatchController().columns.hiddenColumns.has(column) ? null : children;
}

export function PatchTable() {
	const controller = usePatchController();
	const wrap = useRef<HTMLElement>(null);
	const { revealRequest, setRevealRequest } = controller.ui;
	const bottomInset = controller.props.stagePreviewOpen
		? controller.props.stagePreviewClearance
		: 0;
	useLayoutEffect(() => {
		if (!revealRequest) return;
		setRevealRequest(null);
		const row = [
			...(wrap.current?.querySelectorAll<HTMLElement>("tr[data-fixture-id]") ??
				[]),
		].find((candidate) => candidate.dataset.fixtureId === revealRequest.fixtureId);
		if (row) revealPatchRow(row, bottomInset);
	}, [revealRequest, setRevealRequest, bottomInset]);
	return (
		<section
			ref={wrap}
			className="patch-table-wrap"
			style={bottomInset ? { scrollPaddingBottom: bottomInset } : undefined}
		>
			<table className="patch-table">
				<thead>
					<tr>
						{PATCH_SHEET_COLUMNS.filter(
							({ id }) => !controller.columns.hiddenColumns.has(id),
						).map(({ id, label }) => (
							<PatchColumnHeader key={id} column={label} />
						))}
					</tr>
				</thead>
				<tbody>
					{controller.data.visible.map((fixture) => (
						<FixtureRows key={fixture.fixture_id} fixture={fixture} />
					))}
				</tbody>
			</table>
			{!controller.data.visible.length && (
				<div className="patch-empty">No fixtures in this layer.</div>
			)}
			{controller.props.stagePreviewOpen && (
				<div
					className="patch-stage-scroll-clearance"
					style={{ height: controller.props.stagePreviewClearance }}
					aria-hidden="true"
				/>
			)}
		</section>
	);
}

/**
 * A column header. A sortable one orders the table by its column: the first click ascending, the
 * next descending. The header keeps the column's name; its button is named for what it does.
 */
function PatchColumnHeader({ column }: { column: string }) {
	const controller = usePatchController();
	if (!isPatchSortColumn(column)) return <th>{column}</th>;
	const { sort, setSort } = controller.ui;
	const active = sort.column === column;
	return (
		<th aria-label={column} aria-sort={active ? sort.direction : "none"}>
			<button
				type="button"
				className={`patch-sort${active ? " is-active" : ""}`}
				aria-label={`Sort by ${column}`}
				onClick={() => setSort(nextPatchSort(sort, column))}
			>
				{column}
				<span className="patch-sort-mark" aria-hidden="true">
					{active ? (sort.direction === "ascending" ? "▲" : "▼") : ""}
				</span>
			</button>
		</th>
	);
}

/**
 * Scrolls a fixture's row into view unless it is already fully visible between the sticky header
 * and any stage preview covering the bottom of the table.
 */
function FixtureRows({ fixture }: { fixture: PatchedFixture }) {
	return (
		<Fragment>
			<FixtureRow fixture={fixture} />
			{(fixture.multipatch ?? []).map((instance, index) => (
				<MultiPatchRow
					key={instance.id}
					fixture={fixture}
					instance={instance}
					last={index === (fixture.multipatch?.length ?? 0) - 1}
				/>
			))}
		</Fragment>
	);
}

function FixtureRow({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	const layerLocked = Boolean(
		controller.data.layers.find(
			(layer) => layer.id === (fixture.layer_id || "default"),
		)?.locked,
	);
	const selectedFixtureIds = controller.selection.fixtureIds;
	const selected =
		selectedFixtureIds?.has(fixture.fixture_id) ||
		fixture.logical_heads.some((head) =>
			selectedFixtureIds?.has(head.fixture_id),
		) ||
		controller.ui.selectedFixture === fixture.fixture_id;
	const pending = controller.patch.pendingFixtureIds.has(fixture.fixture_id);
	return (
		<tr
			data-fixture-id={fixture.fixture_id}
			className={`${selected ? "selected" : ""} ${pending ? "pending" : ""} ${layerLocked ? "is-layer-locked" : ""}`.trim()}
			aria-disabled={layerLocked || undefined}
			aria-busy={pending || undefined}
			onMouseDown={(event) => {
				if (!controller.host.desktopEditing || event.button !== 0) return;
				controller.ui.dragSelection.current = fixture.fixture_id;
				selectPatchFixture(controller, fixture, event);
			}}
			onMouseEnter={(event) => {
				if (
					!controller.host.desktopEditing ||
					(event.buttons & 1) === 0 ||
					!controller.ui.dragSelection.current
				)
					return;
				selectFixtureRange(
					controller,
					controller.data.visible,
					fixture.fixture_id,
				);
			}}
			onMouseUp={() => {
				controller.ui.dragSelection.current = null;
			}}
			onClick={(event) => {
				if (!controller.host.desktopEditing)
					selectPatchFixture(controller, fixture, event);
			}}
		>
			<FixtureIdentityCells fixture={fixture} />
			<FixturePatchCell fixture={fixture} />
			<FixturePolicyCells fixture={fixture} />
			<FixtureBehaviorCells fixture={fixture} />
			<FixtureTransformCells fixture={fixture} />
			<FixtureLayerCell fixture={fixture} />
			<FixtureVisibilityCells fixture={fixture} />
			<FixtureNoteCell fixture={fixture} />
		</tr>
	);
}

function FixtureNoteCell({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	const note =
		controller.library?.fixtureNotes?.get(fixture.fixture_id)?.note ?? "";
	return (
		<Shown column="note">
		<td className="patch-note-cell">
			<Button
				className="patch-value"
				aria-label={`Note ${fixtureDisplayId(fixture)}`}
				onClick={() => armEdit(controller, fixture, "note")}
			>
				{note || "—"}
			</Button>
		</td>
		</Shown>
	);
}

function EyeIcon({ visible }: { visible: boolean }) {
	return visible ? (
		<svg viewBox="0 0 24 24" aria-hidden="true">
			<path d="M2.5 12s3.5-6 9.5-6 9.5 6 9.5 6-3.5 6-9.5 6-9.5-6-9.5-6Z" />
			<circle cx="12" cy="12" r="2.8" />
		</svg>
	) : (
		<svg viewBox="0 0 24 24" aria-hidden="true">
			<path d="m3 3 18 18M10.6 6.1A10 10 0 0 1 12 6c6 0 9.5 6 9.5 6a15 15 0 0 1-2.3 3M6.2 6.2C3.8 8 2.5 12 2.5 12s3.5 6 9.5 6c1.1 0 2.1-.2 3-.5M9.8 9.8a3 3 0 0 0 4.4 4.4" />
		</svg>
	);
}

function FixtureVisibilityCells({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	const stored = controller.library?.fixtureVisibility?.get(fixture.fixture_id);
	const visibility = stored ?? {
		fixtureId: fixture.fixture_id,
		visible2d: true,
		visible3d: true,
	};
	return (
		<>
			{(["2d", "3d"] as const).map((surface) => {
				const key = surface === "2d" ? "visible2d" : "visible3d";
				const visible = visibility[key];
				return (
					<Shown column={`visible_${surface}`} key={surface}>
					<td className="patch-visibility-cell">
						<Button
							className="patch-visibility-toggle"
							aria-label={`${visible ? "Hide" : "Show"} fixture ${fixtureDisplayId(fixture)} in ${surface.toUpperCase()}`}
							onClick={(event) => {
								event.stopPropagation();
								void controller.library?.saveFixtureVisibility?.({
									...visibility,
									[key]: !visible,
								});
							}}
						>
							<EyeIcon visible={visible} />
						</Button>
					</td>
					</Shown>
				);
			})}
		</>
	);
}

function FixturePolicyCells({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	const applicable = fixturePolicyApplicability(fixture.definition);
	const policyCell = (
		available: boolean,
		label: string,
		value: boolean,
		kind: "invert_pan" | "invert_tilt",
		trueLabel: string,
		falseLabel: string,
	) => (
		<Shown column={kind}>
		<td>
			{available ? (
				<Button
					className="patch-value"
					aria-label={`${label} ${fixtureDisplayId(fixture)}`}
					onClick={() => armEdit(controller, fixture, kind)}
					onContextMenu={(event) =>
						openModalOnContext(event, controller, fixture, kind)
					}
				>
					{value ? trueLabel : falseLabel}
				</Button>
			) : (
				<span role="img" aria-label={`${label} unavailable`}>
					—
				</span>
			)}
		</td>
		</Shown>
	);
	return (
		<>
			<MastersCell fixture={fixture} />
			{policyCell(
				applicable.pan,
				"Invert Pan",
				fixture.invert_pan ?? false,
				"invert_pan",
				"Inverted",
				"Normal",
			)}
			{policyCell(
				applicable.tilt,
				"Invert Tilt",
				fixture.invert_tilt ?? false,
				"invert_tilt",
				"Inverted",
				"Normal",
			)}
		</>
	);
}

function FixtureIdentityCells({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	return (
		<>
			<Shown column="type">
			<td className="patch-type-cell">
				<FixtureTypeIcon type={fixture.definition.device_type} />
			</td>
			</Shown>
			<Shown column="fixture_id">
			<td>
				<DesktopEditableValue
					fixture={fixture}
					kind="number"
					label={`Fixture ID ${fixtureDisplayId(fixture)}`}
					value={String(fixtureDisplayId(fixture))}
				/>
			</td>
			</Shown>
			<Shown column="name">
			<td>
				<DesktopEditableValue
					fixture={fixture}
					kind="name"
					label={`Name ${fixtureDisplayId(fixture)}`}
					value={fixture.name || fixture.definition.name}
				/>
			</td>
			</Shown>
			<Shown column="manufacturer">
			<td>{fixture.definition.manufacturer}</td>
			</Shown>
			<Shown column="mode">
			<td>
				<Button
					className="patch-value"
					onClick={() => armEdit(controller, fixture, "mode")}
					onContextMenu={(event) =>
						openModalOnContext(event, controller, fixture, "mode")
					}
				>
					{fixture.definition.model} · {fixture.definition.mode}
				</Button>
			</td>
			</Shown>
		</>
	);
}

function FixturePatchCell({ fixture }: { fixture: PatchedFixture }) {
	return (
		<Shown column="patch">
			<FixturePatchValue fixture={fixture} />
		</Shown>
	);
}

function FixturePatchValue({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	if (!isDmxPatchable(fixture.definition))
		return (
			<td>
				<span>Not patchable</span>
			</td>
		);
	if (definitionSplits(fixture.definition).length === 1)
		return (
			<td>
				<DesktopEditableValue
					fixture={fixture}
					kind="address"
					label={`Patch ${fixtureDisplayId(fixture)}`}
					value={formatFixturePatch(fixture)}
					onConfigure={() =>
						armEdit(controller, fixture, "address", undefined, "modal")
					}
				/>
			</td>
		);
	return (
		<td>
			{/* biome-ignore lint/a11y/useSemanticElements: Keeping the existing div preserves the compact table-cell geometry. */}
			<div
				className="split-patch-targets"
				role="group"
				aria-label={`Fixture ${fixtureDisplayId(fixture)} split patches`}
			>
				{effectiveSplitPatches(
					fixture.definition,
					fixture.split_patches,
					fixture.universe,
					fixture.address,
				).map((patch) => (
					<Button
						key={patch.split}
						className="patch-address"
						active={
							controller.ui.selectedFixture === fixture.fixture_id &&
							controller.ui.editingSplit === patch.split
						}
						aria-label={`Split ${patch.split} patch ${patch.universe && patch.address ? `${patch.universe}.${patch.address}` : "unpatched"}`}
						onClick={(event) => {
							event.stopPropagation();
							selectSplitAddress(controller, fixture, patch.split);
						}}
					>
						S{patch.split}{" "}
						{patch.universe && patch.address
							? `${patch.universe}.${patch.address}`
							: "—"}
					</Button>
				))}
				{controller.host.desktopEditing ? (
					<Button
						className="patch-universe-action"
						aria-label={`Open patch settings for fixture ${fixtureDisplayId(fixture)}`}
						onClick={(event) => {
							event.stopPropagation();
							armEdit(controller, fixture, "address", undefined, "modal");
						}}
					>
						<SettingsSlidersIcon />
					</Button>
				) : null}
			</div>
		</td>
	);
}

function SettingsSlidersIcon() {
	return (
		<svg aria-hidden="true" viewBox="0 0 24 24">
			<path d="M4 7h6m4 0h6M10 4v6M4 17h10m4 0h2m-6-3v6" />
		</svg>
	);
}

function DesktopEditableValue({
	fixture,
	kind,
	label,
	value,
	onConfigure,
}: {
	fixture: PatchedFixture;
	kind: "number" | "name" | "address";
	label: string;
	value: string;
	onConfigure?: () => void;
}) {
	const controller = usePatchController();
	const editing =
		controller.host.desktopEditing &&
		controller.ui.editPresentation === "inline" &&
		controller.ui.edit === kind &&
		controller.ui.selectedFixture === fixture.fixture_id;
	const editor = useRef<HTMLSpanElement>(null);
	const input = useRef<HTMLInputElement>(null);
	useEffect(() => {
		if (!editing) return;
		// Focusing must not scroll the table sideways towards the field.
		input.current?.focus({ preventScroll: true });
		const outside = (event: PointerEvent) => {
			if (editor.current?.contains(event.target as Node)) return;
			requestFixtureEditClose(controller);
		};
		document.addEventListener("pointerdown", outside, true);
		return () => document.removeEventListener("pointerdown", outside, true);
	}, [controller, editing]);
	if (!controller.host.desktopEditing)
		return (
			<Button
				className={
					kind === "address"
						? "patch-address split-patch-summary"
						: "patch-value"
				}
				onClick={() => armEdit(controller, fixture, kind)}
			>
				{value}
			</Button>
		);
	const cancel = () => cancelEdit(controller);
	const keyboard = (event: KeyboardEvent<HTMLInputElement>) => {
		if (event.key === "Enter") saveEdit(controller);
		if (event.key === "Escape") cancel();
	};
	const inputWidth = Math.max(5, Math.min(24, value.length + 1));
	return (
		// biome-ignore lint/a11y/noStaticElementInteractions: The wrapper owns the contextual edit gesture for both child actions.
		<span
			ref={editor}
			className={editing ? "patch-inline-editor" : "patch-inline-value"}
			style={
				{ "--patch-inline-width": `${inputWidth}ch` } as CSSProperties
			}
			onContextMenu={(event) => {
				event.preventDefault();
				event.stopPropagation();
				if (isControlClick(event)) return;
				selectContextFixture(controller, fixture);
				armEdit(
					controller,
					fixture,
					kind,
					undefined,
					kind === "number" || kind === "address" ? "value_entry" : "modal",
				);
			}}
		>
			{onConfigure ? (
				<Button
					className="patch-inline-configure"
					aria-label={`Open patch settings for fixture ${fixtureDisplayId(fixture)}`}
					onClick={(event) => {
						event.stopPropagation();
						onConfigure();
					}}
				>
					<SettingsSlidersIcon />
				</Button>
			) : null}
			<Button
				className="patch-inline-pencil"
				aria-label={`Edit ${label}`}
				onClick={(event) => {
					event.stopPropagation();
					armEdit(controller, fixture, kind, undefined, "inline");
				}}
			>
				<span aria-hidden="true">✎</span>
			</Button>
			<input
				ref={input}
				className={
					kind === "address"
						? "patch-address split-patch-summary"
						: "patch-value"
				}
				aria-label={editing ? `Edit ${label}` : label}
				readOnly={!editing}
				size={inputWidth}
				value={editing ? controller.ui.editText : value}
				onDoubleClick={(event) => {
					if (editing) return;
					event.stopPropagation();
					armEdit(controller, fixture, kind, undefined, "inline");
				}}
				onChange={(event) => controller.ui.setEditText(event.target.value)}
				onKeyDown={editing ? keyboard : undefined}
			/>
		</span>
	);
}

function FixtureBehaviorCells({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	return (
		<Shown column="mib">
			<td>
				{isDmxPatchable(fixture.definition) ? (
					<Button
						className="patch-value"
						aria-label={`MIB ${fixtureDisplayId(fixture)}`}
						onClick={() => armEdit(controller, fixture, "mib")}
						onContextMenu={(event) =>
							openModalOnContext(event, controller, fixture, "mib")
						}
					>
						{formatMib(fixture)}
					</Button>
				) : (
					"—"
				)}
			</td>
		</Shown>
	);
}

/** Which masters reduce the fixture: none, group, grand or both. */
function MastersCell({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	const value = mastersValue(fixture);
	return (
		<Shown column="masters">
			<td>
				{value ? (
					<Button
						className="patch-value"
						aria-label={`Masters ${fixtureDisplayId(fixture)}`}
						onClick={() => armEdit(controller, fixture, "masters")}
						onContextMenu={(event) =>
							openModalOnContext(event, controller, fixture, "masters")
						}
					>
						{value}
					</Button>
				) : (
					<span role="img" aria-label="Masters unavailable">
						—
					</span>
				)}
			</td>
		</Shown>
	);
}

function FixtureTransformCells({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	return (
		<>
			{(["x", "y", "z"] as const).map((axis) => (
				<Shown column={`location_${axis}`} key={`location-${axis}`}>
				<td className="patch-secondary">
					<Button
						className="patch-value"
						onClick={() => armEdit(controller, fixture, "location", axis)}
						onContextMenu={(event) =>
							openModalOnContext(event, controller, fixture, "location", axis)
						}
					>
						{((fixture.location?.[axis] ?? 0) / 1000).toFixed(3)} m
					</Button>
				</td>
				</Shown>
			))}
			{(["x", "y", "z"] as const).map((axis) => (
				<Shown column={`rotation_${axis}`} key={`rotation-${axis}`}>
				<td className="patch-secondary">
					<Button
						className="patch-value"
						onClick={() => armEdit(controller, fixture, "rotation", axis)}
						onContextMenu={(event) =>
							openModalOnContext(event, controller, fixture, "rotation", axis)
						}
					>
						{Number((fixture.rotation?.[axis] ?? 0).toFixed(3))}°
					</Button>
				</td>
				</Shown>
			))}
			<Shown column="bracket">
			<td className="patch-secondary">
				<Button
					className="patch-value"
					onClick={() => armEdit(controller, fixture, "bracket_angle")}
					onContextMenu={(event) =>
						openModalOnContext(event, controller, fixture, "bracket_angle")
					}
				>
					{Number((fixture.bracket_angle ?? 0).toFixed(1))}°
				</Button>
			</td>
			</Shown>
			<Shown column="shaper">
			<td className="patch-secondary">
				<Button
					className="patch-value"
					onClick={() => armEdit(controller, fixture, "shaper_angle")}
					onContextMenu={(event) =>
						openModalOnContext(event, controller, fixture, "shaper_angle")
					}
				>
					{fixture.shaper_angle === undefined || fixture.shaper_angle === null
						? "\u2014"
						: `${Number(fixture.shaper_angle.toFixed(1))}°`}
				</Button>
			</td>
			</Shown>
			<SceneryCells fixture={fixture} />
			<SceneryOptionCells fixture={fixture} />
		</>
	);
}

/**
 * A generated Venue object's colour, and how a chain is rigged. Anything that is not generated has
 * no colour to choose, and anything that is not a chain has no rigging.
 */
function SceneryOptionCells({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	const generated = Boolean(sceneryOf(fixture));
	const chain = isChain(fixture);
	const colour = sceneryOptionsOf(fixture).colour_srgb;
	const edit = (kind: "scenery_colour" | "chain") => ({
		onClick: () => armEdit(controller, fixture, kind),
		onContextMenu: (event: ReactMouseEvent<HTMLElement>) =>
			openModalOnContext(event, controller, fixture, kind),
	});
	return (
		<>
			<Shown column="scenery_colour">
				<td className="patch-secondary">
					{generated ? (
						<Button
							className="patch-value"
							aria-label={`Colour ${fixtureDisplayId(fixture)}`}
							{...edit("scenery_colour")}
						>
							{colour ? (
								<>
									<span
										aria-hidden="true"
										style={{
											display: "inline-block",
											width: "0.8em",
											height: "0.8em",
											marginRight: "0.35em",
											borderRadius: 2,
											verticalAlign: "middle",
											background: colour,
										}}
									/>
									{colour}
								</>
							) : (
								"Default"
							)}
						</Button>
					) : (
						"—"
					)}
				</td>
			</Shown>
			<Shown column="chain">
				<td className="patch-secondary">
					{chain ? (
						<Button
							className="patch-value"
							aria-label={`Chain ${fixtureDisplayId(fixture)}`}
							{...edit("chain")}
						>
							{chainModeLabel(chainModeOf(fixture))}
						</Button>
					) : (
						"—"
					)}
				</td>
			</Shown>
		</>
	);
}

/**
 * Width, height and depth of a generated Venue object — a truss's length, a curtain's width and
 * drop, a stage element's height. Only the measurements its profile makes to measure are offered;
 * anything else in the rig is the size it is and shows a dash.
 */
function SceneryCells({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	const scenery = sceneryOf(fixture);
	const placed = scenery ? placedSceneryMetres(fixture, scenery) : null;
	return (
		<>
			{SCENERY_AXES.map((axis) => (
				<Shown column={`footprint_${axis.axis}`} key={axis.axis}>
					<td className="patch-secondary">
						{scenery && placed && scenery.adjustable[axis.axis] ? (
							<Button
								className="patch-value"
								aria-label={`${axis.label} ${fixtureDisplayId(fixture)}`}
								onClick={() =>
									armEdit(controller, fixture, axis.edit, undefined, "value_entry")
								}
								onContextMenu={(event) =>
									openModalOnContext(event, controller, fixture, axis.edit)
								}
							>
								{placed[axis.key].toFixed(2)} m
							</Button>
						) : (
							"—"
						)}
					</td>
				</Shown>
			))}
		</>
	);
}

function openModalOnContext(
	event: ReactMouseEvent<HTMLElement>,
	controller: ReturnType<typeof usePatchController>,
	fixture: PatchedFixture,
	kind: Parameters<typeof armEdit>[2],
	axis?: Parameters<typeof armEdit>[3],
) {
	if (!controller.host.desktopEditing) return;
	event.preventDefault();
	event.stopPropagation();
	if (isControlClick(event)) return;
	selectContextFixture(controller, fixture);
	armEdit(
		controller,
		fixture,
		kind,
		axis,
		isContextualNumericEdit(kind, axis) ? "value_entry" : "modal",
	);
}

/**
 * macOS reports Ctrl-click as a context-menu gesture as well. In the sheet Ctrl-click is an
 * additive selection, which the row's mouse-down already applied, so it must not open an editor.
 */
function isControlClick(event: ReactMouseEvent<HTMLElement>) {
	return event.ctrlKey && event.button === 0;
}

function isContextualNumericEdit(
	kind: Parameters<typeof armEdit>[2],
	axis?: Parameters<typeof armEdit>[3],
) {
	return (
		kind === "number" ||
		kind === "address" ||
		kind === "bracket_angle" ||
		kind === "shaper_angle" ||
		kind === "scenery_width" ||
		kind === "scenery_height" ||
		kind === "scenery_depth" ||
		((kind === "location" || kind === "rotation") && Boolean(axis))
	);
}

function FixtureLayerCell({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	return (
		<Shown column="layer">
		<td className="patch-secondary">
			<Button
				className="patch-value"
				onClick={() => {
					if (controller.editArmed) {
						controller.ui.setSelectedFixture(fixture.fixture_id);
						controller.ui.setLayerModal("select");
					}
				}}
				onContextMenu={(event) => {
					if (!controller.host.desktopEditing) return;
					event.preventDefault();
					event.stopPropagation();
					if (isControlClick(event) || !controller.editArmed) return;
					selectContextFixture(controller, fixture);
					controller.ui.setSelectedFixture(fixture.fixture_id);
					controller.ui.setLayerModal("select");
				}}
			>
				{controller.data.layers.find(
					(layer) => layer.id === (fixture.layer_id || "default"),
				)?.name ?? "Default"}
			</Button>
		</td>
		</Shown>
	);
}

function MultiPatchRow({
	fixture,
	instance,
	last,
}: {
	fixture: PatchedFixture;
	instance: MultiPatchInstance;
	last: boolean;
}) {
	const controller = usePatchController();
	const applicable = fixturePolicyApplicability(fixture.definition);
	return (
		<tr
			className="multipatch-row"
			onClick={(event) => selectPatchFixture(controller, fixture, event)}
		>
			<Shown column="type">
				<td className="patch-tree-cell">
					<MultiPatchBranch last={last} />
				</td>
			</Shown>
			<Shown column="fixture_id">
				<td />
			</Shown>
			<Shown column="name">
				<td className="multipatch-name">
					<strong>{instance.name || "Multi-patch"}</strong>
					<span>multi-patch</span>
				</td>
			</Shown>
			<Shown column="manufacturer">
				<td />
			</Shown>
			<Shown column="mode">
				<td />
			</Shown>
			<Shown column="patch">
				<td>
					{isDmxPatchable(fixture.definition) ? (
						<Button
							className="patch-address split-patch-summary"
							onClick={() =>
								beginMultipatchEdit(controller, fixture, instance, "address")
							}
						>
							{formatInstancePatch(fixture.definition, instance)}
						</Button>
					) : (
						<span>Not patchable</span>
					)}
				</td>
			</Shown>
			<Shown column="masters">
				<td>
					{mastersValue(fixture) ? `Shared · ${mastersValue(fixture)}` : "—"}
				</td>
			</Shown>
			<MultipatchAxisCell
				fixture={fixture}
				instance={instance}
				axis="pan"
				available={applicable.pan}
			/>
			<MultipatchAxisCell
				fixture={fixture}
				instance={instance}
				axis="tilt"
				available={applicable.tilt}
			/>
			<Shown column="mib">
				<td />
			</Shown>
			<MultipatchTransformCells fixture={fixture} instance={instance} />
			{(["layer", "visible_2d", "visible_3d", "note"] as const).map((column) => (
				<Shown column={column} key={column}>
					<td />
				</Shown>
			))}
		</tr>
	);
}

/** A copy's own position in the rig: location, rotation, bracket and shaper. */
function MultipatchTransformCells({
	fixture,
	instance,
}: {
	fixture: PatchedFixture;
	instance: MultiPatchInstance;
}) {
	const controller = usePatchController();
	return (
		<>
			{(["x", "y", "z"] as const).map((axis) => (
				<Shown column={`location_${axis}`} key={`location-${axis}`}>
					<td className="patch-secondary">
						<Button
							className="patch-value"
							onClick={() =>
								beginMultipatchEdit(
									controller,
									fixture,
									instance,
									"location",
									axis,
								)
							}
						>
							{(instance.location[axis] / 1000).toFixed(3)} m
						</Button>
					</td>
				</Shown>
			))}
			{(["x", "y", "z"] as const).map((axis) => (
				<Shown column={`rotation_${axis}`} key={`rotation-${axis}`}>
					<td className="patch-secondary">
						<Button
							className="patch-value"
							onClick={() =>
								beginMultipatchEdit(
									controller,
									fixture,
									instance,
									"rotation",
									axis,
								)
							}
						>
							{Number(instance.rotation[axis].toFixed(3))}°
						</Button>
					</td>
				</Shown>
			))}
			<Shown column="bracket">
				<td className="patch-secondary">
					<Button
						className="patch-value"
						onClick={() =>
							beginMultipatchEdit(controller, fixture, instance, "bracket_angle")
						}
					>
						{Number((instance.bracket_angle ?? 0).toFixed(1))}°
					</Button>
				</td>
			</Shown>
			<Shown column="shaper">
				<td className="patch-secondary">
					<Button
						className="patch-value"
						onClick={() =>
							beginMultipatchEdit(controller, fixture, instance, "shaper_angle")
						}
					>
						{instance.shaper_angle === undefined || instance.shaper_angle === null
							? "\u2014"
							: `${Number(instance.shaper_angle.toFixed(1))}°`}
					</Button>
				</td>
			</Shown>
			{/* A Venue object has no copies, so a copy's row has no size of its own to show. */}
			{[
				...SCENERY_AXES.map((axis) => `footprint_${axis.axis}` as const),
				...SCENERY_OPTION_COLUMNS,
			].map((column) => (
				<Shown column={column} key={column}>
					<td className="patch-secondary">—</td>
				</Shown>
			))}
		</>
	);
}

function MultipatchAxisCell({
	fixture,
	instance,
	axis,
	available,
}: {
	fixture: PatchedFixture;
	instance: MultiPatchInstance;
	axis: "pan" | "tilt";
	available: boolean;
}) {
	const controller = usePatchController();
	const column = axis === "pan" ? "invert_pan" : "invert_tilt";
	if (!available)
		return (
			<Shown column={column}>
				<td>
					<span role="img" aria-label={`Invert ${axis} unavailable`}>
						—
					</span>
				</td>
			</Shown>
		);
	const inverted =
		axis === "pan"
			? (instance.invert_pan ?? false)
			: (instance.invert_tilt ?? false);
	return (
		<Shown column={column}>
		<td>
			<Button
				className="patch-value"
				aria-label={`Invert ${axis} ${instance.name || "Multi-patch"}`}
				onClick={() =>
					beginMultipatchEdit(
						controller,
						fixture,
						instance,
						axis === "pan" ? "invert_pan" : "invert_tilt",
					)
				}
			>
				{inverted ? "Inverted" : "Normal"}
			</Button>
		</td>
		</Shown>
	);
}
