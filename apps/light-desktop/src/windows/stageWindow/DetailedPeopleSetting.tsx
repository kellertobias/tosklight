import { SwitchField } from "@tosklight/ui";
import { useApp } from "../../state/AppContext";

/** A local picture preference, also frozen by captures made from lightweight views. */
export function DetailedPeopleSetting() {
	const { state, dispatch } = useApp();
	return <SwitchField
		label="Detailed people"
		description="Rounded, show-lit audience bodies in Ultra and 4K images. Draft and Lines stay lightweight."
		offLabel="Off"
		onLabel="On"
		checked={state.stageVizDetailedPeople ?? false}
		onChange={(event) => dispatch({ type: "SET_STAGE_OPTIONS", vizDetailedPeople: event.target.checked })}
	/>;
}
