import type { FixtureMode } from "../wire";
import { ControlActionCard } from "./controlActionCard";

/** The mode's typed control actions. New ones are added from the mode editor's title bar. */
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
		<section className="fixture-mode-control-actions">
			{!mode.control_actions.length && (
				<p className="empty-editor-message">
					This mode has no control actions. Add one — lamp on, reset, a fan setting — from
					the title bar.
				</p>
			)}
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
