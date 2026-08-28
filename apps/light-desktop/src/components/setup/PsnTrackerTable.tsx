import { SelectField } from "@tosklight/ui";
import type { PsnBinding, PsnPoint, PsnTracker } from "../../api/client/psn";

/**
 * Every tracker the desk has heard of, and what it is doing.
 *
 * A tracker with no name has simply not been named yet — names arrive once a second in their own
 * packet — so the number is shown either way rather than a blank.
 */
export function PsnTrackerTable({
	trackers,
	bindings,
	points,
	busy,
	onBind,
}: {
	trackers: PsnTracker[];
	bindings: PsnBinding[];
	points: PsnPoint[];
	busy: boolean;
	onBind: (trackerId: number, pointFixtureId: string) => void;
}) {
	if (trackers.length === 0) {
		return <p>No tracker has been heard from yet.</p>;
	}
	return (
		<table className="psn-trackers">
			<thead>
				<tr>
					<th>Tracker</th>
					<th>Position</th>
					<th>Last seen</th>
					<th>Source</th>
					<th>3D Point</th>
				</tr>
			</thead>
			<tbody>
				{trackers.map((tracker) => {
					const bound = bindings.find(
						(binding) => binding.trackerId === tracker.trackerId,
					);
					return (
						<tr
							key={`${tracker.source}-${tracker.trackerId}`}
							className={tracker.stale ? "is-stale" : undefined}
						>
							<td>
								{tracker.name
									? `${tracker.trackerId} · ${tracker.name}`
									: tracker.trackerId}
							</td>
							<td>
								{tracker.positionMetres
									? tracker.positionMetres
											.map((axis) => `${axis.toFixed(2)}m`)
											.join(", ")
									: "not reported"}
							</td>
							<td>
								{tracker.stale
									? `${Math.round(tracker.ageMillis / 1000)}s ago — stale`
									: `${tracker.ageMillis}ms ago`}
							</td>
							<td>{tracker.source}</td>
							<td>
								<SelectField
									ariaLabel={`3D Point for tracker ${tracker.trackerId}`}
									value={bound?.pointFixtureId ?? ""}
									disabled={busy || points.length === 0}
									options={[
										{ value: "", label: "Not bound" },
										...points.map((point) => ({
											value: point.fixtureId,
											label: point.fixtureNumber
												? `${point.fixtureNumber} · ${point.name}`
												: point.name,
										})),
									]}
									onChange={(value) => {
										if (value) onBind(tracker.trackerId, value);
									}}
								/>
							</td>
						</tr>
					);
				})}
			</tbody>
		</table>
	);
}
