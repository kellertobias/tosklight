import { ErrorAlert } from "@tosklight/ui";
import { Button } from "@tosklight/ui";
import { useEffect, useState } from "react";
import { useActiveShowError } from "../../features/deskSnapshot/DeskSnapshotState";
import { useDeskStateDiagnostics } from "../../features/deskState/DeskStateDiagnosticsState";
import { criticalDeskFailure } from "../../features/shellStatus/criticalDeskFailure";
import { useShellStatusActions } from "../../features/shellStatus/ShellStatusActionsProvider";
import {
	useConnectionStatus,
	useServerError,
} from "../../features/shellStatus/ShellStatusState";

/** Red is reserved for authoritative capability loss; request failures remain non-blocking. */
export function ServerErrorToast() {
	const connection = useConnectionStatus();
	const error = useServerError();
	const actions = useShellStatusActions();
	const critical = criticalDeskFailure(
		useActiveShowError(),
		useDeskStateDiagnostics(),
	);
	const [displayedError, setDisplayedError] = useState<string | null>(null);
	useEffect(() => {
		if (connection !== "connected") {
			setDisplayedError(null);
			return;
		}
		setDisplayedError(error);
	}, [connection, error]);
	if (connection !== "connected") return null;
	if (critical)
		return (
			<ErrorAlert
				as="aside"
				className="server-error-toast"
				copyText={critical.message}
				role="alert"
				aria-label="Desk failure"
			>
				<div>
					<strong>Desk needs attention</strong>
					<span>{critical.message}</span>
					<small>{critical.action}</small>
				</div>
			</ErrorAlert>
		);
	if (!displayedError) return null;
	return (
		<aside
			className="server-action-notice"
			role="status"
			aria-label="Action feedback"
		>
			<div>
				<strong>Action could not be completed</strong>
				<details>
					<summary>{displayedError.split("\n")[0]}</summary>
					<pre>{displayedError}</pre>
				</details>
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
