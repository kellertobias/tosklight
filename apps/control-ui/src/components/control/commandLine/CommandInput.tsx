import { useServer } from "../../../api/ServerContext";
import { useApp } from "../../../state/AppContext";
import { Button, Input } from "../../common";
import { CommandLineStatus } from "./CommandLineStatus";

export function CommandInput({
	playback,
	hardware,
	completed,
	commandError,
	onReplace,
	onExecute,
	onOpenHistory,
}: {
	playback: boolean;
	hardware: boolean;
	completed: boolean;
	commandError: string | null;
	onReplace: (value: string, pristine?: boolean) => void;
	onExecute: () => Promise<void>;
	onOpenHistory: () => void;
}) {
	const { state, dispatch } = useApp();
	const server = useServer();
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
					className={`command-input ${state.preload === "blind" ? "blind" : ""} ${state.updateArmed ? "update-armed" : ""} ${completed ? "completed" : ""} ${commandError ? "error" : ""}`}
					aria-label="Command line"
					value={server.commandLine}
					placeholder=""
					onClick={onOpenHistory}
					onChange={(event) =>
						onReplace(
							completed
								? `${server.commandTargetMode} ${event.target.value.slice(-1)}`
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
				<CommandLineStatus />
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
