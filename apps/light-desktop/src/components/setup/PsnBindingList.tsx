import { Button, SwitchField } from "@tosklight/ui";
import type {
	PsnBinding,
	PsnEdit,
	PsnPlacement,
	PsnPoint,
} from "../../api/client/psn";

/**
 * What each tracker is currently holding, and the two ways to take it back.
 *
 * The switch releases the point to the show while keeping the binding; Unbind forgets it
 * altogether. Both are deliberate acts an operator can see, which is the whole reason a bound
 * point ignores cues and encoders in the first place.
 */
export function PsnBindingList({
	bindings,
	points,
	placements,
	busy,
	onEdit,
}: {
	bindings: PsnBinding[];
	points: PsnPoint[];
	placements: PsnPlacement[];
	busy: boolean;
	onEdit: (edit: PsnEdit) => void;
}) {
	const edit = onEdit;
	const configuration = { bindings };
	const unbind = (binding: PsnBinding) =>
		edit({
			bindings: bindings.filter((candidate) => candidate.id !== binding.id),
		});
	const status = { placements };
	return (
		<>
	{configuration.bindings.length === 0 ? (
		<p>
			Nothing is bound. A tracker moves a 3D Point only once it has been
			given one here.
		</p>
	) : (
		<ul className="psn-bindings">
			{configuration.bindings.map((binding) => {
				const point = points.find(
					(candidate) => candidate.fixtureId === binding.pointFixtureId,
				);
				const placement = status.placements.find(
					(candidate) => candidate.bindingId === binding.id,
				);
				return (
					<li key={binding.id}>
						<span>
							Tracker {binding.trackerId} → {point?.name ?? "a deleted 3D Point"}
						</span>
						{placement && (
							<span className="psn-placement">
								{formatPoint(placement.positionMetres)}
								{placement.outOfReach && " — out of reach"}
							</span>
						)}
						<SwitchField
							label={`Binding ${binding.trackerId}`}
							offLabel="Held by the show"
							onLabel="Held by the tracker"
							checked={binding.enabled}
							disabled={busy}
							onChange={(event) =>
								void edit({
									bindings: configuration.bindings.map((candidate) =>
										candidate.id === binding.id
											? { ...candidate, enabled: event.target.checked }
											: candidate,
									),
								})
							}
						/>
						<Button
							disabled={busy}
							onClick={() => void unbind(binding)}
							aria-label={`Unbind tracker ${binding.trackerId}`}
						>
							Unbind
						</Button>
					</li>
				);
			})}
		</ul>
	)}
		</>
	);
}

function formatPoint(position: [number, number, number]): string {
	return position.map((axis) => `${axis.toFixed(2)}m`).join(", ");
}
