import { useEffect, useRef } from "react";
import { Button } from "../controls";
import type { CommandLineMode } from "./CommandLine";

export function CommandModeToggle({ mode, onToggle, onSettings }: {
	mode: CommandLineMode;
	onToggle?(): void;
	onSettings?(): void;
}) {
	const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
	const held = useRef(false);
	const cancel = () => {
		if (timer.current !== null) clearTimeout(timer.current);
		timer.current = null;
	};
	useEffect(() => cancel, []);
	return (
		<Button
			className={`mode-toggle ${mode === "playbacks" ? "playbacks-active" : ""}`}
			title={onSettings ? "Hold: Playback section settings" : undefined}
			onPointerDown={() => {
				cancel();
				held.current = false;
				if (onSettings) {
					timer.current = setTimeout(() => {
						held.current = true;
						onSettings();
					}, 650);
				}
			}}
			onPointerUp={cancel}
			onPointerLeave={cancel}
			onPointerCancel={() => {
				cancel();
				held.current = true;
			}}
			onContextMenu={(event) => event.preventDefault()}
			onClick={() => {
				if (!held.current) onToggle?.();
				held.current = false;
			}}
		>
			<span className="mode-icon">{mode === "playbacks" ? "▶" : "⌨"}</span>
			<span>
				<b>PROG.</b>
				<small>PLAYBK</small>
			</span>
		</Button>
	);
}
