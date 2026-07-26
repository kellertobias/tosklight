import type { ReactNode } from "react";
import type { CommandLineMode } from "./CommandLine";

export interface CommandSectionProps {
	mode: CommandLineMode;
	hardware: boolean;
	commandLine: ReactNode;
	programmer: ReactNode;
	playbacks: ReactNode;
	programmerTools: ReactNode;
	playbackTools: ReactNode;
	hardwareTools: ReactNode;
	className?: string;
}

/**
 * Complete service-independent lower desk surface.
 *
 * Runtime adapters provide the stateful command line, programmer, playback, and tool
 * surfaces. This component owns the software/hardware and programmer/playback
 * layout switch so application and Storybook render the same structure.
 */
export function CommandSection({
	mode,
	hardware,
	commandLine,
	programmer,
	playbacks,
	programmerTools,
	playbackTools,
	hardwareTools,
	className = "",
}: CommandSectionProps) {
	const primary = mode === "programmer" ? programmer : playbacks;
	const secondary = hardware
		? hardwareTools
		: mode === "programmer"
			? programmerTools
			: playbackTools;
	return (
		<section
			className={`control-section ${mode} ${hardware ? "hardware-connected" : "touch-connected"} ${className}`.trim()}
			data-control-mode={mode}
			data-hardware-connected={hardware ? "true" : "false"}
		>
			{commandLine}
			<div className="control-left-pane">{primary}</div>
			<aside
				className={`control-right-pane ${hardware ? "hardware-right-pane" : ""}`}
			>
				{hardware ? (
					secondary
				) : (
					<div className="control-right-main">{secondary}</div>
				)}
			</aside>
		</section>
	);
}
