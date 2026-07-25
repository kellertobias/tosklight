import { memo } from "react";
import type { ConnectionStatus } from "../../../api/types";
import { useOutputRuntimeBlackout } from "../../../features/outputRuntime/OutputRuntimeView";
import { Button } from "../../common";

export const CommandLineStatus = memo(function CommandLineStatus({
	status,
	frequency,
	timecode,
	onOpen,
}: {
	status: ConnectionStatus;
	frequency: number | "—";
	timecode: string | null;
	onOpen: () => void;
}) {
	const blackout = useOutputRuntimeBlackout() === true;
	return (
		<Button
			aria-label={`DMX ${frequency}Hz; ${timecode ?? "No Timecode"}. Open running and output controls`}
			className={`command-status ${status}`}
			title="Open running and output controls"
			onClick={onOpen}
		>
			<span className={blackout ? "blackout-status" : ""}>
				{blackout ? (
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
});

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
