import { type RefObject, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { Button, Input } from "../controls";

export type CommandLineMode = "programmer" | "playbacks";
export type CommandConnectionStatus =
	| "connecting"
	| "connected"
	| "offline"
	| "error";
export type CommandRecordState =
	| "empty"
	| "ready"
	| "record-armed"
	| "update-armed";

export interface CommandHistoryItem {
	id: string;
	command: string;
	status: "accepted" | "rejected";
	feedback: string;
	source: "software" | "osc" | "window";
	at: string;
}

export interface CommandStatus {
	connection: CommandConnectionStatus;
	frequency: number | "—";
	timecode: string | null;
	blackout: boolean;
}

export interface CommandLineProps {
	mode: CommandLineMode;
	hardware: boolean;
	ready: boolean;
	completed: boolean;
	commandError: string | null;
	persistentError: string | null;
	persistentErrorOpen: boolean;
	commandLine: string;
	commandTarget: string;
	preloadArmed: boolean;
	preloadActive: boolean;
	preloadReady: boolean;
	preloadLabel: string;
	pendingSummary: string;
	recordState: CommandRecordState;
	recordShiftArmed: boolean;
	history: readonly CommandHistoryItem[];
	historyOpen: boolean;
	status: CommandStatus;
	onReplace: (value: string, pristine?: boolean) => void;
	onExecute: () => void | Promise<void>;
	onToggleMode: () => void;
	onHistoryOpenChange: (open: boolean) => void;
	onReuseHistory: (command: string) => void;
	onOpenStatus: () => void;
	onAcknowledgeCommandError: () => void;
	onPersistentErrorOpenChange: (open: boolean) => void;
	onAcknowledgePersistentError: () => void;
	onRecordStart: (shifted: boolean) => void;
	onRecordEnd: () => void;
	onRecordCancel: () => void;
	onRecordComplete: (shifted: boolean) => void;
	onAdvancePreload: () => void | Promise<void>;
	onReleasePreload: () => void | Promise<void>;
}

export function CommandLine(props: CommandLineProps) {
	const historyPanel = useRef<HTMLElement | null>(null);
	useHistoryDismissal(
		props.historyOpen,
		historyPanel,
		props.onHistoryOpenChange,
	);
	return (
		<header
			className={`command-line-bar command-line-left ${props.hardware ? "hardware-mode" : ""} ${props.mode === "playbacks" ? "playback-mode" : ""} ${props.commandError ? "has-command-error" : ""}`}
			aria-busy={!props.ready}
			data-command-authority={props.ready ? "ready" : "loading"}
		>
			<CommandErrorBanner
				message={props.commandError}
				onAcknowledge={props.onAcknowledgeCommandError}
			/>
			<CommandInputSurface {...props} />
			<CommandHistoryPanel
				history={props.history}
				open={props.historyOpen}
				panel={historyPanel}
				onClose={() => props.onHistoryOpenChange(false)}
				onReuse={props.onReuseHistory}
			/>
			<PersistentErrorPopover
				message={props.persistentError}
				open={props.persistentErrorOpen}
				onClose={() => props.onPersistentErrorOpenChange(false)}
				onAcknowledge={props.onAcknowledgePersistentError}
			/>
			<CommandRecordPreload {...props} />
		</header>
	);
}

function CommandInputSurface(props: CommandLineProps) {
	return (
		<>
			<Button
				className={`mode-toggle ${props.mode === "playbacks" ? "playbacks-active" : ""}`}
				onClick={props.onToggleMode}
			>
				<span className="mode-icon">
					{props.mode === "playbacks" ? "▶" : "⌨"}
				</span>
				<span>
					<b>PROG.</b>
					<small>PLAYBK</small>
				</span>
			</Button>
			<div className="command-field">
				<Input
					className={`command-input ${props.preloadArmed ? "blind" : ""} ${props.recordState === "update-armed" ? "update-armed" : ""} ${props.completed ? "completed" : ""} ${props.commandError ? "error" : ""}`}
					aria-label="Command line"
					value={props.commandLine}
					placeholder=""
					onClick={() => props.onHistoryOpenChange(true)}
					onChange={(event) =>
						props.onReplace(
							props.completed
								? `${props.commandTarget} ${event.target.value.slice(-1)}`
								: event.target.value,
						)
					}
					onKeyDown={(event) => {
						if (event.key !== "Enter") return;
						event.stopPropagation();
						void props.onExecute();
					}}
				/>
				{!props.hardware && (
					<Button
						className="command-escape"
						onClick={() => props.onReplace("", true)}
					>
						ESC
					</Button>
				)}
				<CommandStatusButton
					status={props.status}
					onOpen={props.onOpenStatus}
				/>
			</div>
			{props.completed && (
				<span
					className="command-complete"
					role="img"
					aria-label="Command applied"
				>
					✓
				</span>
			)}
		</>
	);
}

function CommandStatusButton({
	status,
	onOpen,
}: {
	status: CommandStatus;
	onOpen: () => void;
}) {
	return (
		<Button
			aria-label={`DMX ${status.frequency}Hz; ${status.timecode ?? "No Timecode"}. Open running and output controls`}
			className={`command-status ${status.connection}`}
			title="Open running and output controls"
			onClick={onOpen}
		>
			<span className={status.blackout ? "blackout-status" : ""}>
				{status.blackout ? (
					<>
						<i>
							<span className="status-label-full">DMX </span>
							{status.frequency}Hz
						</i>
						<b>BLACKOUT</b>
					</>
				) : (
					<>
						<span className="status-label-full">DMX {status.frequency}Hz</span>
						<span className="status-label-compact">{status.frequency}Hz</span>
					</>
				)}
			</span>
			<span
				className={`timecode-status ${status.timecode ? "timecode-active" : "timecode-idle"}`}
			>
				{status.timecode ?? (
					<>
						<span className="status-label-full">No Timecode</span>
						<span className="status-label-compact">No TC</span>
					</>
				)}
			</span>
		</Button>
	);
}

function CommandErrorBanner({
	message,
	onAcknowledge,
}: {
	message: string | null;
	onAcknowledge: () => void;
}) {
	if (!message) return null;
	return createPortal(
		<div className="command-error-message" role="alert">
			<span>{message}</span>
			<Button onClick={onAcknowledge}>Acknowledge</Button>
		</div>,
		document.body,
	);
}

function PersistentErrorPopover({
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

function CommandRecordPreload(props: CommandLineProps) {
	const preloadHold = useRef<number | null>(null);
	const preloadHeld = useRef(false);
	const cancelPreloadHold = () => {
		if (preloadHold.current !== null) window.clearTimeout(preloadHold.current);
		preloadHold.current = null;
	};
	const recordClass =
		props.recordState === "update-armed"
			? "update-armed"
			: props.recordState === "record-armed"
				? "armed"
				: props.recordState === "ready"
					? "record-ready"
					: "record-empty";
	const recordLabel =
		props.recordState === "update-armed"
			? "UPDATE ARMED"
			: props.recordState === "record-armed"
				? "REC ARMED"
				: "REC";
	return (
		<div className="command-record-preload">
			<Button
				className={`global-store-button ${recordClass}`}
				aria-pressed={
					props.recordState === "update-armed" ||
					props.recordState === "record-armed"
				}
				title="REC · Shift+REC: Update · hold Shift+REC: Update Settings"
				onPointerDown={(event) =>
					props.onRecordStart(props.recordShiftArmed || event.shiftKey)
				}
				onPointerUp={props.onRecordEnd}
				onPointerCancel={props.onRecordCancel}
				onClick={(event) =>
					props.onRecordComplete(props.recordShiftArmed || event.shiftKey)
				}
			>
				{recordLabel}
			</Button>
			<Button
				className={`preload-button ${props.preloadArmed ? "preload-go" : "preload-enter"}`}
				disabled={!props.preloadReady}
				aria-busy={!props.preloadReady}
				title={
					props.preloadArmed && props.pendingSummary
						? `Pending Preload: ${props.pendingSummary}`
						: props.preloadActive
							? "Hold to release the active preload scene"
							: undefined
				}
				onPointerDown={() => {
					preloadHeld.current = false;
					if (!props.preloadActive) return;
					preloadHold.current = window.setTimeout(() => {
						preloadHeld.current = true;
						void props.onReleasePreload();
					}, 650);
				}}
				onPointerUp={cancelPreloadHold}
				onPointerCancel={cancelPreloadHold}
				onContextMenu={(event) => event.preventDefault()}
				onClick={() => {
					if (!preloadHeld.current) void props.onAdvancePreload();
					preloadHeld.current = false;
				}}
			>
				<b>{props.preloadLabel}</b>
				{!props.preloadArmed && props.preloadActive && (
					<small>(Hold: release)</small>
				)}
			</Button>
		</div>
	);
}

function CommandHistoryPanel({
	history,
	open,
	panel,
	onClose,
	onReuse,
}: {
	history: readonly CommandHistoryItem[];
	open: boolean;
	panel: RefObject<HTMLElement | null>;
	onClose: () => void;
	onReuse: (command: string) => void;
}) {
	if (!open) return null;
	return createPortal(
		<section
			className="command-history-panel"
			role="dialog"
			aria-modal="false"
			aria-label="Command line history"
			ref={panel}
		>
			<header>
				<div>
					<h2>Command Line History</h2>
					<small>Newest first · this desk · last 50 results</small>
				</div>
				<Button aria-label="Close command line history" onClick={onClose}>
					×
				</Button>
			</header>
			<div className="command-history-list">
				{history.length === 0 ? (
					<p className="command-history-empty">
						No accepted or rejected commands yet.
					</p>
				) : (
					history.map((entry) => (
						<article
							className={`command-history-entry ${entry.status}`}
							key={entry.id}
						>
							<div className="command-history-entry-main">
								<span className="command-history-status">
									{entry.status === "accepted" ? "Accepted" : "Rejected"}
								</span>
								<code>{entry.command}</code>
								<small>
									{new Date(entry.at).toLocaleTimeString([], {
										hour: "2-digit",
										minute: "2-digit",
										second: "2-digit",
									})}{" "}
									·{" "}
									{entry.source === "osc"
										? "attached hardware"
										: entry.source === "window"
											? "content window"
											: "desk"}
								</small>
							</div>
							<p>{entry.feedback}</p>
							{entry.source !== "window" && (
								<Button onClick={() => onReuse(entry.command)}>Reuse</Button>
							)}
						</article>
					))
				)}
			</div>
		</section>,
		document.body,
	);
}

function useHistoryDismissal(
	open: boolean,
	panel: RefObject<HTMLElement | null>,
	setOpen: (open: boolean) => void,
) {
	useEffect(() => {
		if (!open) return;
		const closeOnEscape = (event: KeyboardEvent) => {
			if (event.key !== "Escape") return;
			event.preventDefault();
			setOpen(false);
		};
		const closeOutside = (event: PointerEvent) => {
			if (panel.current?.contains(event.target as Node)) return;
			if ((event.target as Element | null)?.closest(".command-input")) return;
			setOpen(false);
		};
		window.addEventListener("keydown", closeOnEscape, true);
		window.addEventListener("pointerdown", closeOutside, true);
		return () => {
			window.removeEventListener("keydown", closeOnEscape, true);
			window.removeEventListener("pointerdown", closeOutside, true);
		};
	}, [open, panel, setOpen]);
}
