import type { PatchedFixture } from "../../api/types";
import { usePatch, usePatchView } from "../../features/patch/PatchContext";
import { useProgrammingSelectionView } from "../../features/programmingInteraction/ProgrammingInteractionView";
import { Button } from "../common";

const slots = (["x", "y", "z"] as const)
	.flatMap((axis) => [
		{ kind: "location" as const, axis },
		{ kind: "rotation" as const, axis },
	])
	.sort((left, right) => left.kind.localeCompare(right.kind));

export function PatchParameterControls() {
	const patch = usePatch();
	usePatchView();
	const selection = useProgrammingSelectionView();
	const fixture = selection
		? selectedPatchedFixture(patch.fixtures, selection.selected)
		: null;
	const updateVector = (
		kind: "location" | "rotation",
		axis: "x" | "y" | "z",
		delta: number,
	) => {
		if (!fixture) return;
		const current = fixture[kind] ?? { x: 0, y: 0, z: 0 };
		void patch.updateFixture(fixture.fixture_id, {
			[kind]: { ...current, [axis]: current[axis] + delta },
		});
	};
	const label =
		patch.status !== "ready"
			? "Patch loading…"
			: selection
				? fixture
					? fixture.name || fixture.definition.name
					: "Select a patched fixture"
				: "Programmer selection loading…";
	const disabled = patch.status !== "ready" || !fixture;
	return (
		<div className="parameter-controls patch-parameter-controls">
			<div className="family-tabs">
				<b>Fixture position</b>
				<span className="family-spacer" />
				<small>{label}</small>
			</div>
			<div className="parameter-surfaces">
				{slots.map(({ kind, axis }) => (
					<PatchVectorControl
						key={`${kind}-${axis}`}
						kind={kind}
						axis={axis}
						stored={fixture?.[kind]?.[axis] ?? 0}
						disabled={disabled}
						onChange={updateVector}
					/>
				))}
			</div>
		</div>
	);
}

function selectedPatchedFixture(
	fixtures: readonly PatchedFixture[],
	selectedIds: readonly string[],
) {
	for (const selectedId of selectedIds) {
		const fixture = fixtures.find(
			(item) =>
				item.fixture_id === selectedId ||
				item.logical_heads.some((head) => head.fixture_id === selectedId),
		);
		if (fixture) return fixture;
	}
	return null;
}

function PatchVectorControl({
	kind,
	axis,
	stored,
	disabled,
	onChange,
}: {
	kind: "location" | "rotation";
	axis: "x" | "y" | "z";
	stored: number;
	disabled: boolean;
	onChange: (
		kind: "location" | "rotation",
		axis: "x" | "y" | "z",
		delta: number,
	) => void;
}) {
	const label = kind === "location" ? "Location" : "Rotation";
	const display =
		kind === "location"
			? `${(stored / 1000).toFixed(3)} m`
			: `${stored.toFixed(0)}°`;
	const step = kind === "location" ? 10 : 1;
	return (
		<div className="patch-vector-control">
			<span>
				{label} {axis.toUpperCase()}
			</span>
			<strong>{display}</strong>
			<div>
				<Button disabled={disabled} onClick={() => onChange(kind, axis, -step)}>
					−
				</Button>
				<Button disabled={disabled} onClick={() => onChange(kind, axis, step)}>
					+
				</Button>
			</div>
		</div>
	);
}
