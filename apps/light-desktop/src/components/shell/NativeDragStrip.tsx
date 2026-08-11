import { Button } from "@tosklight/ui";
import { useEffect, useState } from "react";
import { useDesktopBridge } from "../../platform/desktop";

/**
 * `closes` says what the strip's X owns: the main window is the desk itself, so closing it
 * quits the application, while every screen window only closes itself.
 */
export function NativeDragStrip({
	closes = "window",
}: {
	closes?: "window" | "application";
} = {}) {
	const desktop = useDesktopBridge();
	const [fullscreen, setFullscreen] = useState(false);
	const closeLabel =
		closes === "application" ? "Quit ToskLight" : "Close window";

	useEffect(() => {
		if (!desktop.available) return;
		let active = true;
		void desktop.currentWindowFullscreen().then((next) => {
			if (active) setFullscreen(next);
		});
		return () => {
			active = false;
		};
	}, [desktop]);

	const closeWindow = () => {
		if (!desktop.available) return;
		void (closes === "application"
			? desktop.exitApplication()
			: desktop.closeCurrentWindow());
	};
	const enterFullscreen = () => {
		if (!desktop.available) return;
		void desktop.setCurrentWindowFullscreen(true).then(() => {
			setFullscreen(true);
		});
	};
	const exitFullscreen = () => {
		if (!desktop.available) return;
		void desktop.setCurrentWindowFullscreen(false).then(() => {
			setFullscreen(false);
		});
	};
	const startDragging = (event: React.PointerEvent<HTMLButtonElement>) => {
		if (event.button !== 0 || !desktop.available) return;
		event.preventDefault();
		void desktop.startCurrentWindowDrag();
	};
	if (!desktop.available) return null;
	if (fullscreen)
		return (
			<div className="native-drag-strip native-drag-strip-fullscreen">
				<Button
					className="native-window-exit-fullscreen"
					aria-label="Exit fullscreen"
					title="Exit fullscreen"
					onClick={exitFullscreen}
				>
					Exit fullscreen
				</Button>
			</div>
		);
	return (
		<div className="native-drag-strip">
			<Button
				className="native-window-close"
				aria-label={closeLabel}
				title={closeLabel}
				onClick={closeWindow}
			>
				<svg viewBox="0 0 16 16" aria-hidden="true">
					<path d="M4 4l8 8M12 4l-8 8" />
				</svg>
			</Button>
			<Button
				aria-label="Enter fullscreen"
				aria-pressed="false"
				title="Enter fullscreen"
				onClick={enterFullscreen}
			>
				<svg viewBox="0 0 16 16" aria-hidden="true">
					<path d="M6 2H2v4M10 2h4v4M6 14H2v-4M10 14h4v-4" />
				</svg>
			</Button>
			<Button
				className="native-window-drag"
				data-tauri-drag-region
				aria-label="Move window"
				title="Drag to move window"
				onPointerDown={startDragging}
			>
				<svg viewBox="0 0 16 16" aria-hidden="true">
					<path d="M8 1v14M1 8h14M8 1L6 3m2-2 2 2M8 15l-2-2m2 2 2-2M1 8l2-2M1 8l2 2m12-2-2-2m2 2-2 2" />
				</svg>
			</Button>
		</div>
	);
}
