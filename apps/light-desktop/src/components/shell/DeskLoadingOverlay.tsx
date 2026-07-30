import { useDeskLoadingState } from "../../features/deskLoading/DeskLoadingState";
import { LoadingSurface } from "../common/LoadingSurface";

export function DeskLoadingOverlay() {
	const loading = useDeskLoadingState();
	if (!loading) return null;
	return (
		<LoadingSurface
			className="show-loading-cover"
			title={loading.title}
			detail={loading.detail}
			note="Ready desk controls remain available while show capabilities reconnect."
		/>
	);
}
