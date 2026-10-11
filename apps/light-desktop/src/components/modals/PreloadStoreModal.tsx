import { useApp } from "../../state/AppContext";
import { PendingPreloadInspection } from "./PendingPreloadInspection";

export function PreloadStoreModal() {
	const { state, dispatch } = useApp();
	if (!state.preloadStoreOpen) return null;
	return (
		<PendingPreloadInspection
			onClose={() =>
				dispatch({ type: "SET_MODAL", modal: "preloadStoreOpen", value: false })
			}
		/>
	);
}
