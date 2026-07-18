import { useServer } from "../../../api/ServerContext";
import { useApp } from "../../../state/AppContext";
import { Button } from "../../common";

export function CommandLineStatus() {
	const server = useServer();
	const { state, dispatch } = useApp();
	const frequency = server.bootstrap?.frame_rate_hz ?? "—";
	const timecode = server.bootstrap?.active_timecode;
	return (
		<Button
			aria-label={`DMX ${frequency}Hz; ${timecode ?? "No Timecode"}. Open running and output controls`}
			className={`command-status ${server.status}`}
			title="Open running and output controls"
			onClick={() =>
				dispatch({
					type: "SET_MODAL",
					modal: "systemControlsOpen",
					value: true,
				})
			}
		>
			<span className={state.blackout ? "blackout-status" : ""}>
				{state.blackout ? (
					<>
						<i>
							<span className="status-label-full">DMX </span>
							{frequency}Hz
						</i>
						<b>BLACKOUT</b>
					</>
				) : (
					<>
						<span className="status-label-full">DMX {frequency}Hz</span>
						<span className="status-label-compact">{frequency}Hz</span>
					</>
				)}
			</span>
			<span
				className={`timecode-status ${timecode ? "timecode-active" : "timecode-idle"}`}
			>
				{timecode ?? (
					<>
						<span className="status-label-full">No Timecode</span>
						<span className="status-label-compact">No TC</span>
					</>
				)}
			</span>
		</Button>
	);
}

export function CommandErrorBanner({
	message,
	onAcknowledge,
}: {
	message: string | null;
	onAcknowledge: () => void;
}) {
	if (!message) return null;
	return (
		<div className="command-error-message" role="alert">
			<span>{message}</span>
			<Button onClick={onAcknowledge}>Acknowledge</Button>
		</div>
	);
}

export function PersistentErrorPopover({
	message,
	open,
	onClose,
	onAcknowledge,
}: {
	message: string | null;
	open: boolean;
	onClose: () => void;
	onAcknowledge: () => void;
}) {
	if (!open || !message) return null;
	return (
		<div className="persistent-error-popover" role="alertdialog">
			<header>
				<b>
					<span>▲</span> Desk error
				</b>
				<Button onClick={onClose}>×</Button>
			</header>
			<pre>{message}</pre>
			<Button onClick={onAcknowledge}>Acknowledge</Button>
		</div>
	);
}
