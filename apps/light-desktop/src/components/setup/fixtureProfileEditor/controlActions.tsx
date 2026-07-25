import type { FixtureMode } from "../../../api/types";
import { Button } from "../../common";
import { uuid } from "../fixtureProfileModel";
import { ControlActionCard } from "./controlActionCard";

export function ControlActionsEditor({
	mode,
	onChange,
}: {
	mode: FixtureMode;
	onChange: (mode: FixtureMode) => void;
}) {
	const setAction = (next: FixtureMode["control_actions"][number]) =>
		onChange({
			...mode,
			control_actions: mode.control_actions.map((action) =>
				action.id === next.id ? next : action,
			),
		});
	return (
		<section className="fixture-control-actions">
			<header>
				<h3>Typed control actions</h3>
				<Button
					onClick={() =>
						onChange({
							...mode,
							control_actions: [
								...mode.control_actions,
								{
									id: uuid(),
									name: `Action ${mode.control_actions.length + 1}`,
									semantic: "custom",
									kind: "momentary",
									duration_millis: null,
									assignments: [],
								},
							],
						})
					}
				>
					Add control action
				</Button>
			</header>
			{mode.control_actions.map((action) => (
				<ControlActionCard
					key={action.id}
					action={action}
					mode={mode}
					onChange={setAction}
					onRemove={() =>
						onChange({
							...mode,
							control_actions: mode.control_actions.filter(
								(candidate) => candidate.id !== action.id,
							),
						})
					}
				/>
			))}
		</section>
	);
}
