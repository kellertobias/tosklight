import { Button } from "@tosklight/ui";
import type { MultiPatchInstance, PatchedFixture } from "../../api/types";
import { usePatch, usePatchView } from "../../features/patch/PatchContext";

const slots = (["x", "y", "z"] as const)
	.flatMap((axis) => [
		{ kind: "location" as const, axis },
		{ kind: "rotation" as const, axis },
	])
	.sort((left, right) => left.kind.localeCompare(right.kind));

export function PatchParameterControls() {
	const patch = usePatch();
	usePatchView();
	const target = selectedPhysicalTarget(
		patch.fixtures,
		patch.selectedPatchInstance,
	);
	const fixture = target?.fixture ?? null;
	const updateVector = (
		kind: "location" | "rotation",
		axis: "x" | "y" | "z",
		delta: number,
	) => {
		if (!target) return;
		const current = target.instance[kind] ?? { x: 0, y: 0, z: 0 };
		const updated = { ...current, [axis]: current[axis] + delta };
		if (!target.multipatch) {
			void patch.updateFixture(target.fixture.fixture_id, { [kind]: updated });
			return;
		}
		void patch.updateFixture(target.fixture.fixture_id, {
			multipatch: (target.fixture.multipatch ?? []).map((instance) =>
				instance.id === target.multipatch?.id
					? { ...instance, [kind]: updated }
					: instance,
			),
		});
	};
	const label =
		patch.status !== "ready"
			? "Patch loading…"
			: target
				? target.multipatch?.name ||
					target.fixture.name ||
					target.fixture.definition.name
				: "Select a physical patch row";
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
						stored={target?.instance[kind]?.[axis] ?? 0}
						disabled={disabled}
						onChange={updateVector}
					/>
				))}
			</div>
		</div>
	);
}

function selectedPhysicalTarget(
	fixtures: readonly PatchedFixture[],
	selection: {
		fixtureId: string;
		multipatchInstanceId: string | null;
	} | null,
) {
	if (!selection) return null;
	const fixture = fixtures.find(
		(candidate) => candidate.fixture_id === selection.fixtureId,
	);
	if (!fixture) return null;
	if (!selection.multipatchInstanceId)
		return {
			fixture,
			instance: fixture,
			multipatch: null as MultiPatchInstance | null,
		};
	const multipatch = fixture.multipatch?.find(
		(instance) => instance.id === selection.multipatchInstanceId,
	);
	return multipatch ? { fixture, instance: multipatch, multipatch } : null;
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
