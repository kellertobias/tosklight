import { Fragment } from "react";
import type { MultiPatchInstance, PatchedFixture } from "../../../api/types";
import { Button } from "../../common";
import { isDmxPatchable } from "../patchUtils";
import { usePatchController } from "./controller";
import { armEdit, selectSplitAddress } from "./editSession";
import { selectPatchFixture } from "./fixtureActions";
import {
	FixtureTypeIcon,
	formatRotation,
	MultiPatchBranch,
} from "./fixtureDisplay";
import { fixtureDisplayId } from "./fixtureIds";
import { beginMultipatchEdit } from "./multipatchActions";
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
	"Manufacturer",
	"Product / mode",
	"Patch",
	"MIB",
	"MIB Delay",
	"Highlight Look",
	"Location X/Y/Z",
	"Rotation X/Y/Z",
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
	const selected =
		controller.server.selectedFixtures.includes(fixture.fixture_id) ||
		fixture.logical_heads.some((head) =>
			controller.server.selectedFixtures.includes(head.fixture_id),
		) ||
		controller.ui.selectedFixture === fixture.fixture_id;
	const pending = controller.patch.pendingFixtureIds.has(fixture.fixture_id);
	return (
		<tr
			className={`${selected ? "selected" : ""} ${pending ? "pending" : ""}`.trim()}
			aria-busy={pending || undefined}
			onClick={(event) => selectPatchFixture(controller, fixture, event)}
		>
			<FixtureIdentityCells fixture={fixture} />
			<FixturePatchCell fixture={fixture} />
			<FixtureBehaviorCells fixture={fixture} />
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
			<td>{fixture.definition.manufacturer}</td>
			<td>
				<Button
					className="patch-value"
					onClick={() => armEdit(controller, fixture, "mode")}
				>
					{fixture.definition.model} · {fixture.definition.mode}
				</Button>
			</td>
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

function FixtureBehaviorCells({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	if (!isDmxPatchable(fixture.definition))
		return (
			<>
				<td>—</td>
				<td>—</td>
				<td>—</td>
			</>
		);
	const overrides = Object.keys(fixture.highlight_overrides ?? {}).length;
	return (
		<>
			<td>
				<Button
					className="patch-value"
					aria-label={`Move in Black ${fixtureDisplayId(fixture)}`}
					onClick={() => armEdit(controller, fixture, "mib")}
				>
					{(fixture.move_in_black_enabled ?? true) ? "On" : "Off"}
				</Button>
			</td>
			<td>
				<Button
					className="patch-value"
					aria-label={`MIB Delay ${fixtureDisplayId(fixture)}`}
					onClick={() => armEdit(controller, fixture, "mib_delay")}
				>
					{(fixture.move_in_black_delay_millis ?? 0) / 1000} s
				</Button>
			</td>
			<td>
				<Button
					className="patch-value"
					aria-label={`Highlight Look ${fixtureDisplayId(fixture)}`}
					onClick={() => armEdit(controller, fixture, "highlight")}
				>
					{overrides
						? `${overrides} override${overrides === 1 ? "" : "s"}`
						: "Profile default"}
				</Button>
			</td>
		</>
	);
}

function FixtureTransformCells({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	return (
		<>
			<td className="patch-secondary">
				<Button
					className="patch-value"
					onClick={() => armEdit(controller, fixture, "location")}
				>
					{(["x", "y", "z"] as const)
						.map((axis) => ((fixture.location?.[axis] ?? 0) / 1000).toFixed(3))
						.join(" / ")}{" "}
					m
				</Button>
			</td>
			<td className="patch-secondary">
				<Button
					className="patch-value"
					onClick={() => armEdit(controller, fixture, "rotation")}
				>
					{formatRotation(fixture.rotation)}
				</Button>
			</td>
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
	return (
		<tr className="multipatch-row">
			<td className="patch-tree-cell">
				<MultiPatchBranch last={last} />
			</td>
			<td />
			<td className="multipatch-name">
				<span>multi-patch</span>
			</td>
			<td />
			<td />
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
			<td />
			<td />
			<td />
			<td className="patch-secondary">
				<Button
					className="patch-value"
					onClick={() =>
						beginMultipatchEdit(controller, fixture, instance, "location")
					}
				>
					{(["x", "y", "z"] as const)
						.map((axis) => (instance.location[axis] / 1000).toFixed(3))
						.join(" / ")}{" "}
					m
				</Button>
			</td>
			<td className="patch-secondary">
				<Button
					className="patch-value"
					onClick={() =>
						beginMultipatchEdit(controller, fixture, instance, "rotation")
					}
				>
					{formatRotation(instance.rotation)}
				</Button>
			</td>
			<td />
		</tr>
	);
}
