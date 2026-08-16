import { Button } from "@tosklight/ui";
import {
	type CSSProperties,
	Fragment,
	type KeyboardEvent,
	type MouseEvent as ReactMouseEvent,
	useEffect,
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
import { selectFixtureRange, selectPatchFixture } from "./fixtureActions";
import { FixtureTypeIcon, MultiPatchBranch } from "./fixtureDisplay";
import { fixtureDisplayId } from "./fixtureIds";
import { beginMultipatchEdit } from "./multipatchActions";
import {
	definitionSplits,
	effectiveSplitPatches,
	fixturePolicyApplicability,
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
	"Group Masters",
	"Grand Master",
	"Invert Pan",
	"Invert Tilt",
	"MIB",
	"MIB Delay",
	"Location X",
	"Location Y",
	"Location Z",
	"Rotation X",
	"Rotation Y",
	"Rotation Z",
	"Bracket",
	"Shaper",
	"Layer",
	"2D",
	"3D",
	"Note",
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
		<td className="patch-note-cell">
			<Button
				className="patch-value"
				aria-label={`Note ${fixtureDisplayId(fixture)}`}
				onClick={() => armEdit(controller, fixture, "note")}
			>
				{note || "—"}
			</Button>
		</td>
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
					<td className="patch-visibility-cell" key={surface}>
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
		kind: "group_masters" | "grand_master" | "invert_pan" | "invert_tilt",
		trueLabel: string,
		falseLabel: string,
	) => (
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
	);
	return (
		<>
			{policyCell(
				applicable.groupMasters,
				"Group Masters",
				fixture.group_masters_enabled ?? true,
				"group_masters",
				"Controlled",
				"Ignored",
			)}
			{policyCell(
				applicable.grandMaster,
				"Grand Master",
				fixture.grand_master_enabled ?? true,
				"grand_master",
				"Controlled",
				"Ignored",
			)}
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
			<td className="patch-type-cell">
				<FixtureTypeIcon type={fixture.definition.device_type} />
			</td>
			<td>
				<DesktopEditableValue
					fixture={fixture}
					kind="number"
					label={`Fixture ID ${fixtureDisplayId(fixture)}`}
					value={String(fixtureDisplayId(fixture))}
				/>
			</td>
			<td>
				<DesktopEditableValue
					fixture={fixture}
					kind="name"
					label={`Name ${fixtureDisplayId(fixture)}`}
					value={fixture.name || fixture.definition.name}
				/>
			</td>
			<td>{fixture.definition.manufacturer}</td>
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
		input.current?.focus();
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
	if (!isDmxPatchable(fixture.definition))
		return (
			<>
				<td>—</td>
				<td>—</td>
			</>
		);
	return (
		<>
			<td>
				<Button
					className="patch-value"
					aria-label={`Move in Black ${fixtureDisplayId(fixture)}`}
					onClick={() => armEdit(controller, fixture, "mib")}
					onContextMenu={(event) =>
						openModalOnContext(event, controller, fixture, "mib")
					}
				>
					{(fixture.move_in_black_enabled ?? true) ? "On" : "Off"}
				</Button>
			</td>
			<td>
				<Button
					className="patch-value"
					aria-label={`MIB Delay ${fixtureDisplayId(fixture)}`}
					onClick={() => armEdit(controller, fixture, "mib_delay")}
					onContextMenu={(event) =>
						openModalOnContext(event, controller, fixture, "mib_delay")
					}
				>
					{(fixture.move_in_black_delay_millis ?? 0) / 1000} s
				</Button>
			</td>
		</>
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
						onClick={() => armEdit(controller, fixture, "location", axis)}
						onContextMenu={(event) =>
							openModalOnContext(event, controller, fixture, "location", axis)
						}
					>
						{((fixture.location?.[axis] ?? 0) / 1000).toFixed(3)} m
					</Button>
				</td>
			))}
			{(["x", "y", "z"] as const).map((axis) => (
				<td className="patch-secondary" key={`rotation-${axis}`}>
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
			))}
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
	armEdit(
		controller,
		fixture,
		kind,
		axis,
		isContextualNumericEdit(kind, axis) ? "value_entry" : "modal",
	);
}

function isContextualNumericEdit(
	kind: Parameters<typeof armEdit>[2],
	axis?: Parameters<typeof armEdit>[3],
) {
	return (
		kind === "number" ||
		kind === "address" ||
		kind === "mib_delay" ||
		kind === "bracket_angle" ||
		kind === "shaper_angle" ||
		((kind === "location" || kind === "rotation") && Boolean(axis))
	);
}

function FixtureLayerCell({ fixture }: { fixture: PatchedFixture }) {
	const controller = usePatchController();
	return (
		<td className="patch-secondary">
			<Button
				className="patch-value"
				onClick={() => {
					if (controller.editArmed) {
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
	const applicable = fixturePolicyApplicability(fixture.definition);
	return (
		<tr
			className="multipatch-row"
			onClick={(event) => selectPatchFixture(controller, fixture, event)}
		>
			<td className="patch-tree-cell">
				<MultiPatchBranch last={last} />
			</td>
			<td>
				{applicable.groupMasters
					? `Shared · ${(fixture.group_masters_enabled ?? true) ? "Controlled" : "Ignored"}`
					: "—"}
			</td>
			<td>
				{applicable.grandMaster
					? `Shared · ${(fixture.grand_master_enabled ?? true) ? "Controlled" : "Ignored"}`
					: "—"}
			</td>
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
			<td />
			<td className="multipatch-name">
				<strong>{instance.name || "Multi-patch"}</strong>
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
					>
						{Number(instance.rotation[axis].toFixed(3))}°
					</Button>
				</td>
			))}
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
			<td />
			<td />
			<td />
			<td />
		</tr>
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
	if (!available)
		return (
			<td>
				<span role="img" aria-label={`Invert ${axis} unavailable`}>
					—
				</span>
			</td>
		);
	const inverted =
		axis === "pan"
			? (instance.invert_pan ?? false)
			: (instance.invert_tilt ?? false);
	return (
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
	);
}
