import { Button } from "@tosklight/ui";
import { useApp } from "../../state/AppContext";
import {
	isRegisteredWindow,
	windowRegistry,
} from "../../windows/WindowRegistry";
import { DeskGrid } from "./DeskGrid";

export function WorkspaceView() {
	const { state, dispatch } = useApp();
	const migrationNotice = state.layoutMigrationNotice ? (
		<div className="migration-notice" role="status">
			<span>
				Layout was removed. Spatial ordering now lives in Group settings and
				 Dynamics Projection.
			</span>
			<Button onClick={() => dispatch({ type: "DISMISS_LAYOUT_MIGRATION_NOTICE" })}>
				Dismiss
			</Button>
		</div>
	) : null;
	if (state.builtIn && isRegisteredWindow(state.builtIn)) {
		const Window = windowRegistry[state.builtIn];
		return (
			<>
				{migrationNotice}
				<main
					className="workspace-view built-in-view"
					data-light-surface="built-in"
					data-pane-type={state.builtIn}
					aria-label={`${state.builtIn} built-in`}
				>
					<Window builtIn />
				</main>
			</>
		);
	}
	const desk =
		state.desks.find((item) => item.id === state.activeDeskId) ?? state.desks[0];
	return (
		<>
			{migrationNotice}
			<main
				className="workspace-view"
				data-light-surface="desktop"
				data-desktop-id={desk.id}
				aria-label={`Desktop ${desk.name}`}
			>
				<DeskGrid desk={desk} />
			</main>
		</>
	);
}
