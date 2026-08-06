import { Button } from "@tosklight/ui";
import { Fragment } from "react";
import type { MultiPatchInstance, PatchedFixture } from "../../../api/types";
import { isDmxPatchable } from "../patchUtils";
import { usePatchController } from "./controller";
import {
	armEdit,
	armEditFromContextMenu,
	selectSplitAddress,
} from "./editSession";
import { selectPatchFixture } from "./fixtureActions";
import { FixtureTypeIcon, MultiPatchBranch } from "./fixtureDisplay";
import { fixtureDisplayId } from "./fixtureIds";
import { LightSourceCell } from "./LightSourceAppearance";
import {
	beginMultipatchEdit,
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

const columns = [
	"Type",
	"Fixture ID",
	"Name",
	"Fixture / mode",
	"Patch",
	"Masters",
	"Pan / Tilt",
	"MIB",
	"Light source",
	"Location X",
	"Location Y",
	"Location Z",
	"Rotation X",
	"Rotation Y",
	"Rotation Z",
	"Layer",
];

export function PatchTable() {
	const controller = usePatchController();
	return (
		<section className="patch-table-wrap">
			<table className="patch-table">
				<thead>
					<tr>
						{columns.map((column) => (
							<th key={column}>{column}</th>
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
			<FixturePatchCell fixture={fixture} />
			<MastersCell fixture={fixture} />
			<PanTiltCell fixture={fixture} />
			<MibCell fixture={fixture} />
			<LightSourceCell fixture={fixture} />
			<FixtureTransformCells fixture={fixture} />
			<FixtureLayerCell fixture={fixture} />
		</tr>
	);
}

function FixtureIdentityCells({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	return (
		<>
			<td className="patch-type-cell">
				<FixtureTypeIcon type={fixture.definition.device_type} />
			</td>
			<td>{fixtureDisplayId(fixture)}</td>
			<td>
				<Button
					className="patch-value"
					onClick={() => armEdit(controller, fixture, "name")}
				>
					{fixture.name || fixture.definition.name}
				</Button>
			</td>
			<FixtureModeCell fixture={fixture} />
		</>
	);
}

function FixturePatchCell({ fixture }: { fixture: PatchedFixture }) {
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
				<Button
					className="patch-address split-patch-summary"
					onClick={() => armEdit(controller, fixture, "address")}
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
				<td className="patch-secondary" key={`location-${axis}`}>
					<Button
						className="patch-value"
						aria-label={`Location ${axis.toUpperCase()} ${fixtureDisplayId(fixture)}`}
						onClick={() => armEdit(controller, fixture, "location", axis)}
						onContextMenu={(event) => {
							event.preventDefault();
							armEditFromContextMenu(controller, fixture, "location", axis);
						}}
					>
						{((fixture.location?.[axis] ?? 0) / 1000).toFixed(3)} m
					</Button>
				</td>
			))}
			{(["x", "y", "z"] as const).map((axis) => (
				<td className="patch-secondary" key={`rotation-${axis}`}>
					<Button
						className="patch-value"
						aria-label={`Rotation ${axis.toUpperCase()} ${fixtureDisplayId(fixture)}`}
						onClick={() => armEdit(controller, fixture, "rotation", axis)}
						onContextMenu={(event) => {
							event.preventDefault();
							armEditFromContextMenu(controller, fixture, "rotation", axis);
						}}
					>
						{Number((fixture.rotation?.[axis] ?? 0).toFixed(3))}°
					</Button>
				</td>
			))}
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
			<td className="patch-tree-cell">
				<MultiPatchBranch last={last} />
			</td>
			<td>—</td>
			<td>—</td>
			<FixtureModeCell fixture={fixture} shared />
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
			<MastersCell fixture={fixture} shared />
			<PanTiltCell fixture={fixture} instance={instance} />
			<MibCell fixture={fixture} shared />
			<LightSourceCell fixture={fixture} instance={instance} />
			{(["x", "y", "z"] as const).map((axis) => (
				<td className="patch-secondary" key={`location-${axis}`}>
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
			))}
			{(["x", "y", "z"] as const).map((axis) => (
				<td className="patch-secondary" key={`rotation-${axis}`}>
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
			))}
			<td className="patch-secondary">
				<span>—</span>
			</td>
		</tr>
	);
}
