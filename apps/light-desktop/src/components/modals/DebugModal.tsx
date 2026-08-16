import { ModalPortal, ModalTitleBar } from "@tosklight/ui";
import { useEffect, useRef, useState } from "react";
import { useOutputHealth } from "../../features/deskSnapshot/DeskSnapshotState";
import { useShellStatusActions } from "../../features/shellStatus/ShellStatusActionsProvider";
import { useApp } from "../../state/AppContext";

type LogEntry = { revision: number; kind: string; payload: unknown };

const MAJOR_DESK_EVENT_KINDS = new Set([
	"session_started",
	"session_disconnected",
	"hardware_connection_changed",
	"media_server_offline",
	"preload_persistence_failed",
]);

export function isMajorDeskEvent(entry: LogEntry) {
	const kind = entry.kind.toLowerCase();
	if (MAJOR_DESK_EVENT_KINDS.has(kind)) return true;
	return /(?:^|_)(?:error|failed|failure|rejected|disconnected|offline)(?:_|$)/.test(
		kind,
	);
}

function useMajorDeskEvents(
	open: boolean,
	shellStatus: ReturnType<typeof useShellStatusActions>,
) {
	const [logs, setLogs] = useState<LogEntry[]>([]);
	const lastRevision = useRef(0);
	const reading = useRef(false);
	useEffect(() => {
		if (!open || !shellStatus) return;
		let cancelled = false;
		lastRevision.current = 0;
		setLogs([]);
		const refresh = async () => {
			if (reading.current) return;
			reading.current = true;
			try {
				const entries = await shellStatus.readServerLogs(lastRevision.current);
				if (cancelled) return;
				if (entries.length)
					lastRevision.current = Math.max(
						lastRevision.current,
						...entries.map((entry) => entry.revision),
					);
				const major = entries.filter(isMajorDeskEvent);
				if (major.length)
					setLogs((current) => {
						const byRevision = new Map(
							[...current, ...major].map((entry) => [entry.revision, entry]),
						);
						return [...byRevision.values()]
							.sort((left, right) => left.revision - right.revision)
							.slice(-50);
					});
			} catch {
				// Desk Status remains useful even if one diagnostic refresh fails.
			} finally {
				reading.current = false;
			}
		};
		void refresh();
		const timer = window.setInterval(() => void refresh(), 5_000);
		return () => {
			cancelled = true;
			window.clearInterval(timer);
		};
	}, [open, shellStatus]);
	return logs;
}

export function DebugModal() {
	const { state, dispatch } = useApp();
	const shellStatus = useShellStatusActions();
	const outputHealth = useOutputHealth();
	const logs = useMajorDeskEvents(state.debugOpen, shellStatus);
	if (!state.debugOpen) return null;
	const close = () =>
		dispatch({ type: "SET_MODAL", modal: "debugOpen", value: false });
	return (
		<ModalPortal onClose={close}>
			<div
				className="stacked-modal-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && close()
				}
			>
				<section
					className="nested-modal debug-modal"
					role="dialog"
					aria-modal="true"
					aria-label="Desk Status"
				>
					<ModalTitleBar
						title="Desk Status"
						groups={[
							{
								id: "debug",
								actions: [
									{
										id: "debug",
										kind: "dropdown",
										label: "Debug",
										dropdown: {
											kind: "items",
											ariaLabel: "Debug",
											items: [
												{
													kind: "action",
													id: "sections",
													label: `${state.showSectionNames ? "✓ " : ""}Show section names`,
													onPress: () => {
														dispatch({ type: "TOGGLE_SECTION_NAMES" });
														dispatch({
															type: "SET_MODAL",
															modal: "debugOpen",
															value: false,
														});
														dispatch({
															type: "SET_MODAL",
															modal: "setupOpen",
															value: false,
														});
													},
												},
												{
													kind: "toggle",
													id: "hardware",
													label: "Simulate Hardware",
													checked: state.midiProfile,
													onChange: () =>
														dispatch({ type: "TOGGLE_MIDI_PROFILE" }),
												},
												{
													kind: "toggle",
													id: "scrollbars",
													label: "Simulate Touch Scroll Bars",
													checked: state.touchScrollbars,
													onChange: () =>
														dispatch({ type: "TOGGLE_TOUCH_SCROLLBARS" }),
												},
												{ kind: "divider", id: "errors" },
												{
													kind: "action",
													id: "simulate-error",
													label: "Simulate DMX Error",
													onPress: () =>
														shellStatus?.simulateError(
															"Simulated DMX output failure",
														),
												},
												{
													kind: "action",
													id: "clear-errors",
													label: "Clear Simulated Errors",
													onPress: () => shellStatus?.simulateError(null),
												},
											],
										},
									},
								],
							},
						]}
						closeLabel="Close Desk Status"
						onClose={close}
					/>
					<div className="debug-diagnostics">
						<section>
							<b>{outputHealth?.frame_hz.toFixed(1) ?? "—"} Hz</b>
							<small>Current frame rate</small>
						</section>
						<section>
							<b>{outputHealth?.deadline_misses ?? 0}</b>
							<small>Scheduler deadline misses</small>
						</section>
						<section>
							<b>{outputHealth?.send_errors ?? 0}</b>
							<small>Network output errors</small>
						</section>
					</div>
					<h4>Major desk events</h4>
					<pre className="server-log">
						{logs.length
							? logs
									.map(
										(entry) =>
											`${entry.revision.toString().padStart(6, "0")}  ${entry.kind}  ${JSON.stringify(entry.payload)}`,
									)
									.join("\n")
							: "No major desk events logged."}
					</pre>
				</section>
			</div>
		</ModalPortal>
	);
}
