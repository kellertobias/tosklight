import { Button } from "@tosklight/ui";
import { useShellStatusActions } from "../../features/shellStatus/ShellStatusActionsProvider";
import {
	useConnectionStatus,
	useServerError,
} from "../../features/shellStatus/ShellStatusState";

/** One global lane for connected-desk failures that do not belong to an object surface. */
export function ServerErrorToast() {
	const connection = useConnectionStatus();
	const error = useServerError();
	const actions = useShellStatusActions();
	if (connection !== "connected" || !error) return null;
	return (
		<aside className="server-error-toast" role="alert" aria-label="Desk failure">
			<div>
				<strong>Desk needs attention</strong>
				<span>{error}</span>
				<small>Correct the named condition, then retry the action.</small>
			</div>
			<Button onClick={() => actions?.dismissError()}>Dismiss</Button>
		</aside>
	);
}
