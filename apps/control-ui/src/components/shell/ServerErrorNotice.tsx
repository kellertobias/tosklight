import { useServerError } from "../../features/shellStatus/ShellStatusState";

/**
 * The shared operator error line shown inside a modal or setup surface.
 *
 * It reads the error itself so a surface that only displays errors does not have to subscribe to
 * shell status, and it renders nothing when there is nothing to report.
 */
export function ServerErrorNotice({ alert = false }: { alert?: boolean }) {
	const error = useServerError();
	if (!error) return null;
	return (
		<p className="modal-error" role={alert ? "alert" : undefined}>
			{error}
		</p>
	);
}
