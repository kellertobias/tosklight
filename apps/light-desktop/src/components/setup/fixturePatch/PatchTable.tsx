import {
	SCENERY_AXES,
	placedSceneryMetres,
	sceneryOf,
} from "./scenerySize";
import {
	chainModeLabel,
	chainModeOf,
	isChain,
	sceneryOptionsOf,
} from "./sceneryOptions";
import { formatModelScale, hasModelScale, modelScaleOf } from "./modelScale";
import { Button } from "@tosklight/ui";
import { Fragment, type ReactNode } from "react";
import { PATCH_COLUMNS, type PatchColumn } from "../../../types";
import type { MultiPatchInstance, PatchedFixture } from "../../../api/types";
import { isDmxPatchable, isInternal } from "../patchUtils";
import { usePatchController } from "./controller";
import {
	armEdit,
	beginFixtureEditFromContextMenu,
	beginSplitAddressEditFromContextMenu,
	selectSplitAddress,
} from "./editSession";
import { selectPatchFixture } from "./fixtureActions";
import { FixtureIcon, MultiPatchBranch } from "./fixtureDisplay";
import { fixtureDisplayId } from "./fixtureIds";
import { LightSourceCell } from "./LightSourceAppearance";
import { isPatchSortColumn, nextPatchSort } from "./tableSort";
import {
	beginMultipatchEdit,
	beginMultipatchEditFromContextMenu,
	beginMultipatchVectorEditFromContextMenu,
	PRIMARY_PHYSICAL_PATCH,
	selectPhysicalPatchRow,
} from "./multipatchActions";
import {
	FixtureModeCell,
	MastersCell,
	MibCell,
	PanTiltCell,
} from "./PatchTableStackedCells";
import {
	definitionSplits,
	effectiveSplitPatches,
	formatFixturePatch,
	formatInstancePatch,
} from "./patchModel";

/** Draws its cells only while their column is shown. */
function Shown({ column, children }: { column: PatchColumn; children: ReactNode }) {
	return usePatchController().props.hiddenColumns.has(column) ? null : children;
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
			{/* The arrow is drawn by CSS from data-sort, so the header's text stays the column name. */}
			<Button
				className="patch-sort"
				active={active}
				data-sort={active ? sort.direction : undefined}
				aria-label={`Sort by ${column}`}
				onClick={() => setSort(nextPatchSort(sort, column))}
			>
				{column}
			</Button>
		</th>
	);
}

export function PatchTable() {
	const controller = usePatchController();
	return (
		<section
			className={`patch-table-wrap ${
				controller.appState.patchSetArmed ? "patch-set-targets" : ""
			}`}
		>
			<table className="patch-table">
				<thead>
					<tr>
						{PATCH_COLUMNS.filter(
							({ id }) => !controller.props.hiddenColumns.has(id),
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
	const selectedFixtureIds = controller.selection.fixtureIds;
	const selected =
		selectedFixtureIds?.has(fixture.fixture_id) ||
		fixture.logical_heads.some((head) =>
			selectedFixtureIds?.has(head.fixture_id),
		) ||
		controller.ui.selectedFixture === fixture.fixture_id ||
		(controller.patch.selectedPatchInstance?.fixtureId === fixture.fixture_id &&
			controller.patch.selectedPatchInstance.multipatchInstanceId === null);
	const pending = controller.patch.pendingFixtureIds.has(fixture.fixture_id);
	return (
		<tr
			data-fixture-id={fixture.fixture_id}
			className={`${selected ? "selected" : ""} ${pending ? "pending" : ""}`.trim()}
			aria-busy={pending || undefined}
			onClick={(event) => {
				controller.patch.selectPatchInstance({
					fixtureId: fixture.fixture_id,
					multipatchInstanceId: null,
				});
				selectPhysicalPatchRow(
					controller,
					fixture,
					PRIMARY_PHYSICAL_PATCH,
					event,
				);
				selectPatchFixture(controller, fixture, event);
			}}
		>
			<FixtureIdentityCells fixture={fixture} />
			<Shown column="patch">
				<FixturePatchCell fixture={fixture} />
			</Shown>
			<Shown column="masters">
				<MastersCell fixture={fixture} />
			</Shown>
			<Shown column="pan_tilt">
				<PanTiltCell fixture={fixture} />
			</Shown>
			<Shown column="mib">
				<MibCell fixture={fixture} />
			</Shown>
			<Shown column="light_source">
				<LightSourceCell fixture={fixture} />
			</Shown>
			<FixtureTransformCells fixture={fixture} />
			<Shown column="layer">
				<FixtureLayerCell fixture={fixture} />
			</Shown>
		</tr>
	);
}

function FixtureIdentityCells({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	return (
		<>
			<Shown column="type">
				<td className="patch-type-cell">
					<FixtureIcon definition={fixture.definition} />
				</td>
			</Shown>
			<Shown column="fixture_id">
				<td>{fixtureDisplayId(fixture)}</td>
			</Shown>
			<Shown column="name">
			<td>
				<Button
					className="patch-value"
					onClick={() => armEdit(controller, fixture, "name")}
					onContextMenu={(event) => {
						event.preventDefault();
						event.stopPropagation();
						beginFixtureEditFromContextMenu(controller, fixture, "name");
					}}
				>
					{fixture.name || fixture.definition.name}
				</Button>
			</td>
			</Shown>
			<Shown column="fixture_mode">
				<FixtureModeCell fixture={fixture} />
			</Shown>
		</>
	);
}

function FixturePatchCell({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	if (isInternal(fixture.definition)) {
		const library = fixture.internal_bindings?.library ?? "No library";
		const output = fixture.internal_bindings?.output ?? "No output";
		return (
			<td>
				<Button
					className="patch-address split-patch-summary"
					onClick={() => armEdit(controller, fixture, "internal_bindings")}
					onContextMenu={(event) => {
						event.preventDefault();
						event.stopPropagation();
						beginFixtureEditFromContextMenu(
							controller,
							fixture,
							"internal_bindings",
						);
					}}
				>
					{library} → {output}
				</Button>
			</td>
		);
	}
	if (!isDmxPatchable(fixture.definition))
		return (
			<td>
				<span>Not patchable</span>
			</td>
		);
	if (definitionSplits(fixture.definition).length === 1)
		return (
			<td>
				<Button
					className="patch-address split-patch-summary"
					onClick={() => armEdit(controller, fixture, "address")}
					onContextMenu={(event) => {
						event.preventDefault();
						event.stopPropagation();
						beginFixtureEditFromContextMenu(controller, fixture, "address");
					}}
				>
					{formatFixturePatch(fixture)}
				</Button>
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
						onContextMenu={(event) => {
							event.preventDefault();
							event.stopPropagation();
							beginSplitAddressEditFromContextMenu(
								controller,
								fixture,
								patch.split,
							);
						}}
					>
						S{patch.split}{" "}
						{patch.universe && patch.address
							? `${patch.universe}.${patch.address}`
							: "—"}
					</Button>
				))}
			</div>
		</td>
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
						aria-label={`Location ${axis.toUpperCase()} ${fixtureDisplayId(fixture)}`}
						onClick={() => armEdit(controller, fixture, "location", axis)}
						onContextMenu={(event) => {
							event.preventDefault();
							event.stopPropagation();
							beginFixtureEditFromContextMenu(
								controller,
								fixture,
								"location",
								axis,
							);
						}}
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
						aria-label={`Rotation ${axis.toUpperCase()} ${fixtureDisplayId(fixture)}`}
						onClick={() => armEdit(controller, fixture, "rotation", axis)}
						onContextMenu={(event) => {
							event.preventDefault();
							event.stopPropagation();
							beginFixtureEditFromContextMenu(
								controller,
								fixture,
								"rotation",
								axis,
							);
						}}
					>
						{Number((fixture.rotation?.[axis] ?? 0).toFixed(3))}°
					</Button>
				</td>
				</Shown>
			))}
			<FootprintCells fixture={fixture} />
			<SceneryOptionCells fixture={fixture} />
			<ModelScaleCell fixture={fixture} />
		</>
	);
}

/**
 * How many times its built size a Venue object is drawn — an imported hall modelled in the wrong
 * unit, a set piece at half size. Anything that is not a Venue object shows a dash.
 */
function ModelScaleCell({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	return (
		<Shown column="model_scale">
			{hasModelScale(fixture) ? (
				<td className="patch-secondary">
					<Button
						className="patch-value"
						aria-label={`Scale ${fixtureDisplayId(fixture)}`}
						onClick={() => armEdit(controller, fixture, "model_scale")}
						onContextMenu={(event) => {
							event.preventDefault();
							event.stopPropagation();
							beginFixtureEditFromContextMenu(controller, fixture, "model_scale");
						}}
					>
						{formatModelScale(modelScaleOf(fixture))}
					</Button>
				</td>
			) : (
				NO_MEASUREMENT
			)}
		</Shown>
	);
}

/** One editable measurement in the footprint columns, in metres. */
function MeasurementCell({
	fixture,
	edit,
	label,
	metres,
}: {
	fixture: PatchedFixture;
	edit: Parameters<typeof armEdit>[2];
	label: string;
	metres: number;
}) {
	const controller = usePatchController();
	return (
		<td className="patch-secondary">
			<Button
				className="patch-value"
				aria-label={`${label} ${fixtureDisplayId(fixture)}`}
				onClick={() => armEdit(controller, fixture, edit)}
				onContextMenu={(event) => {
					event.preventDefault();
					event.stopPropagation();
					beginFixtureEditFromContextMenu(controller, fixture, edit);
				}}
			>
				{metres.toFixed(2)} m
			</Button>
		</td>
	);
}

const NO_MEASUREMENT = <td className="patch-secondary">—</td>;

/**
 * Width, height and depth, for the two things in a rig that are made to measure: a crowd area's
 * footprint and a generated Venue object's size. Anything else is the size it is.
 *
 * Only the dimensions the object really is made to measure are offered — a curtain's width and
 * drop, a truss's length. A truss's cross-section is what the truss is, not a number to type.
 */
function FootprintCells({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	const scenery = sceneryOf(fixture);
	if (scenery) {
		const placed = placedSceneryMetres(fixture, scenery);
		return (
			<>
				{SCENERY_AXES.map((axis) =>
					<Shown key={axis.axis} column={`footprint_${axis.axis}`}>
						{scenery.adjustable[axis.axis] ? (
							<MeasurementCell
								fixture={fixture}
								edit={axis.edit}
								label={axis.label}
								metres={placed[axis.key]}
							/>
						) : (
							NO_MEASUREMENT
						)}
					</Shown>
				)}
			</>
		);
	}
	const crowd = fixture.definition.profile_snapshot?.crowd;
	if (!crowd)
		return (
			<>
				<Shown column="footprint_width">{NO_MEASUREMENT}</Shown>
				<Shown column="footprint_height">{NO_MEASUREMENT}</Shown>
				<Shown column="footprint_depth">{NO_MEASUREMENT}</Shown>
			</>
		);
	const stored = controller.stagePositions3d[fixture.fixture_id];
	return (
		<>
			<Shown column="footprint_width">
				<MeasurementCell
					fixture={fixture}
					edit="crowd_width"
					label="Crowd width"
					metres={stored?.crowdWidthMetres ?? crowd.default_width_metres}
				/>
			</Shown>
			<Shown column="footprint_height">{NO_MEASUREMENT}</Shown>
			<Shown column="footprint_depth">
				<MeasurementCell
					fixture={fixture}
					edit="crowd_depth"
					label="Crowd depth"
					metres={stored?.crowdDepthMetres ?? crowd.default_depth_metres}
				/>
			</Shown>
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
	const option = (
		kind: "scenery_colour" | "chain",
		label: string,
		value: ReactNode,
	) => (
		<td className="patch-secondary">
			<Button
				className="patch-value"
				aria-label={`${label} ${fixtureDisplayId(fixture)}`}
				onClick={() => armEdit(controller, fixture, kind)}
				onContextMenu={(event) => {
					event.preventDefault();
					event.stopPropagation();
					beginFixtureEditFromContextMenu(controller, fixture, kind);
				}}
			>
				{value}
			</Button>
		</td>
	);
	return (
		<>
			<Shown column="scenery_colour">
				{generated
					? option(
							"scenery_colour",
							"Colour",
							colour ? (
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
							),
						)
					: NO_MEASUREMENT}
			</Shown>
			<Shown column="chain">
				{chain
					? option("chain", "Chain", chainModeLabel(chainModeOf(fixture)))
					: NO_MEASUREMENT}
			</Shown>
		</>
	);
}

function FixtureLayerCell({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	return (
		<td className="patch-secondary">
			<Button
				className="patch-value"
				onClick={() => {
					if (controller.appState.patchSetArmed) {
						controller.ui.setSelectedFixture(fixture.fixture_id);
						controller.ui.setLayerModal("select");
					}
				}}
				onContextMenu={(event) => {
					event.preventDefault();
					event.stopPropagation();
					controller.ui.setSelectedFixture(fixture.fixture_id);
					controller.ui.setLayerModal("select");
				}}
			>
				{controller.data.layers.find(
					(layer) => layer.id === (fixture.layer_id || "default"),
				)?.name ?? "Default"}
			</Button>
		</td>
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
	const selected =
		controller.patch.selectedPatchInstance?.fixtureId === fixture.fixture_id &&
		controller.patch.selectedPatchInstance.multipatchInstanceId === instance.id;
	return (
		<tr
			className={`multipatch-row${selected ? " selected" : ""}`}
			aria-label={`Multi-patch ${instance.name || instance.id}`}
			onClick={(event) => {
				controller.patch.selectPatchInstance({
					fixtureId: fixture.fixture_id,
					multipatchInstanceId: instance.id,
				});
				selectPhysicalPatchRow(controller, fixture, instance.id, event);
				selectPatchFixture(controller, fixture, event);
			}}
		>
			<Shown column="type">
				<td className="patch-tree-cell">
					<MultiPatchBranch last={last} />
				</td>
			</Shown>
			<Shown column="fixture_id">
				<td>—</td>
			</Shown>
			<Shown column="name">
				<td>—</td>
			</Shown>
			<Shown column="fixture_mode">
				<FixtureModeCell fixture={fixture} shared />
			</Shown>
			<Shown column="patch">
			<td>
				{isDmxPatchable(fixture.definition) ? (
					<Button
						className="patch-address split-patch-summary"
						onClick={() =>
							beginMultipatchEdit(controller, fixture, instance, "address")
						}
						onContextMenu={(event) => {
							event.preventDefault();
							event.stopPropagation();
							beginMultipatchEditFromContextMenu(
								controller,
								fixture,
								instance,
								"address",
							);
						}}
					>
						{formatInstancePatch(fixture.definition, instance)}
					</Button>
				) : (
					<span>Not patchable</span>
				)}
			</td>
			</Shown>
			<Shown column="masters">
				<MastersCell fixture={fixture} shared />
			</Shown>
			<Shown column="pan_tilt">
				<PanTiltCell fixture={fixture} instance={instance} />
			</Shown>
			<Shown column="mib">
				<MibCell fixture={fixture} shared />
			</Shown>
			<Shown column="light_source">
				<LightSourceCell fixture={fixture} instance={instance} />
			</Shown>
			<MultiPatchVectorCells fixture={fixture} instance={instance} />
			<Shown column="footprint_width">{NO_MEASUREMENT}</Shown>
			<Shown column="footprint_height">{NO_MEASUREMENT}</Shown>
			<Shown column="footprint_depth">{NO_MEASUREMENT}</Shown>
			{/* A Venue object has no copies, so a copy's row has no colour or chain rigging of its own. */}
			<Shown column="scenery_colour">{NO_MEASUREMENT}</Shown>
			<Shown column="chain">{NO_MEASUREMENT}</Shown>
			<Shown column="model_scale">{NO_MEASUREMENT}</Shown>
			<Shown column="layer">
				<td className="patch-secondary">
					<span>—</span>
				</td>
			</Shown>
		</tr>
	);
}

/** A multi-patch copy's own location and rotation, one cell per axis. */
function MultiPatchVectorCells({
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
						onContextMenu={(event) => {
							event.preventDefault();
							event.stopPropagation();
							beginMultipatchVectorEditFromContextMenu(
								controller,
								fixture,
								instance,
								"location",
								axis,
							);
						}}
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
						onContextMenu={(event) => {
							event.preventDefault();
							event.stopPropagation();
							beginMultipatchVectorEditFromContextMenu(
								controller,
								fixture,
								instance,
								"rotation",
								axis,
							);
						}}
					>
						{Number(instance.rotation[axis].toFixed(3))}°
					</Button>
				</td>
				</Shown>
			))}
		</>
	);
}
