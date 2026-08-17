import { Button } from "@tosklight/ui";
import { useEffect, useState } from "react";
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
	const [displayedError, setDisplayedError] = useState<string | null>(null);
	useEffect(() => {
		if (connection !== "connected") {
			setDisplayedError(null);
			return;
		}
		if (error) setDisplayedError(error);
	}, [connection, error]);
	if (connection !== "connected" || !displayedError) return null;
	return (
		<aside
			className="server-error-toast"
			role="alert"
			aria-label="Desk failure"
		>
			<div>
				<strong>Desk needs attention</strong>
				<span>{displayedError}</span>
				<small>Correct the named condition, then retry the action.</small>
			</div>
			<Button
				onClick={() => {
					setDisplayedError(null);
					actions?.dismissError();
				}}
			>
				Dismiss
			</Button>
		</aside>
	);
}
