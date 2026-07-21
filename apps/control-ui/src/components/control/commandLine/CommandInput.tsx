import { useCallback } from "react";
import type { CommandTargetMode } from "../../../controlSurface/commandTarget";
import { useApp } from "../../../state/AppContext";
import { Button, Input } from "../../common";
import { CommandLineStatusBoundary } from "./CommandLineStatusBoundary";

export function CommandInput({
	playback,
	hardware,
	completed,
	commandError,
	commandLine,
	commandTarget,
	preloadArmed,
	onReplace,
	onExecute,
	onOpenHistory,
}: {
	playback: boolean;
	hardware: boolean;
	completed: boolean;
	commandError: string | null;
	commandLine: string;
	commandTarget: CommandTargetMode;
	preloadArmed: boolean;
	onReplace: (value: string, pristine?: boolean) => void;
	onExecute: () => Promise<void>;
	onOpenHistory: () => void;
}) {
	const { state, dispatch } = useApp();
	const openSystemControls = useCallback(
		() =>
			dispatch({
				type: "SET_MODAL",
				modal: "systemControlsOpen",
				value: true,
			}),
		[dispatch],
	);
	return (
		<>
			<Button
				className={`mode-toggle ${playback ? "playbacks-active" : ""}`}
				onClick={() => dispatch({ type: "TOGGLE_CONTROL_MODE" })}
			>
				<span className="mode-icon">{playback ? "▶" : "⌨"}</span>
				<span>
					<b>PROG.</b>
					<small>PLAYBK</small>
				</span>
			</Button>
			<div className="command-field">
				<Input
					className={`command-input ${preloadArmed ? "blind" : ""} ${state.updateArmed ? "update-armed" : ""} ${completed ? "completed" : ""} ${commandError ? "error" : ""}`}
					aria-label="Command line"
					value={commandLine}
					placeholder=""
					onClick={onOpenHistory}
					onChange={(event) =>
						onReplace(
							completed
								? `${commandTarget} ${event.target.value.slice(-1)}`
								: event.target.value,
						)
					}
					onKeyDown={(event) => {
						if (event.key !== "Enter") return;
						event.stopPropagation();
						void onExecute();
					}}
				/>
				{!hardware && (
					<Button
						className="command-escape"
						onClick={() => onReplace("", true)}
					>
						ESC
					</Button>
				)}
				<CommandLineStatusBoundary onOpen={openSystemControls} />
			</div>
			{completed && (
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
