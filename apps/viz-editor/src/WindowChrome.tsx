import { Button } from "@tosklight/ui";
import { type ReactNode, useEffect, useState } from "react";

async function currentWindow() {
	const { getCurrentWindow } = await import("@tauri-apps/api/window");
	return getCurrentWindow();
}

export function beginWindowDrag(event: React.PointerEvent<HTMLElement>) {
	if (event.button !== 0) return;
	event.preventDefault();
	void currentWindow()
		.then((window) => window.startDragging())
		.catch(() => undefined);
}

/// Starts a native resize from the corner grip.
///
/// The window is drawn without decorations, so macOS gives it no visible corner to pull and only
/// a hairline of edge to catch. This hands the gesture back to the window manager, which is what
/// makes the resize feel native rather than something reimplemented from pointer deltas.
export function beginWindowResize(event: React.PointerEvent<HTMLElement>) {
	if (event.button !== 0) return;
	event.preventDefault();
	event.stopPropagation();
	void currentWindow()
		.then((window) => window.startResizeDragging("SouthEast"))
		.catch(() => undefined);
}

/// The corner an operator pulls to resize the window.
///
/// Deliberately larger than the diagonal it draws: the mark is small enough to stay out of the
/// way of the content behind it, and the target around it is big enough to hit without aiming.
export function WindowResizeGrip() {
	return (
		<div
			className="viz-window-resize-grip"
			role="separator"
			aria-orientation="vertical"
			aria-label="Resize window"
			title="Drag to resize window"
			onPointerDown={beginWindowResize}
		>
			<svg viewBox="0 0 16 16" aria-hidden="true">
				<path d="M15 5L5 15M15 10l-5 5" />
			</svg>
		</div>
	);
}

export function WindowControls() {
	const [fullscreen, setFullscreen] = useState(false);

	useEffect(() => {
		let current = true;
		void currentWindow()
			.then((window) => window.isFullscreen())
			.then((next) => current && setFullscreen(next))
			.catch(() => undefined);
		return () => {
			current = false;
		};
	}, []);

	const setWindowFullscreen = (next: boolean) => {
		void currentWindow()
			.then((window) => window.setFullscreen(next))
			.then(() => setFullscreen(next))
			.catch(() => undefined);
	};

	return (
		<>
			<WindowResizeGrip />
			<div className="viz-native-window-controls">
			<Button
				className="viz-native-window-close"
				aria-label="Close window"
				title="Close window"
				onClick={() => {
					void currentWindow()
						.then((window) => window.close())
						.catch(() => undefined);
				}}
			>
				<svg viewBox="0 0 16 16" aria-hidden="true">
					<path d="M4 4l8 8M12 4l-8 8" />
				</svg>
			</Button>
			<Button
				aria-label={fullscreen ? "Exit fullscreen" : "Enter fullscreen"}
				aria-pressed={fullscreen}
				title={fullscreen ? "Exit fullscreen" : "Enter fullscreen"}
				onClick={() => setWindowFullscreen(!fullscreen)}
			>
				<svg viewBox="0 0 16 16" aria-hidden="true">
					<path d="M6 2H2v4M10 2h4v4M6 14H2v-4M10 14h4v-4" />
				</svg>
			</Button>
			<Button
				className="viz-native-window-drag"
				data-tauri-drag-region
				aria-label="Move window"
				title="Drag to move window"
				onPointerDown={beginWindowDrag}
			>
				<svg viewBox="0 0 16 16" aria-hidden="true">
					<path d="M8 1v14M1 8h14M8 1L6 3m2-2 2 2M8 15l-2-2m2 2 2-2M1 8l2-2M1 8l2 2m12-2-2-2m2 2-2 2" />
				</svg>
			</Button>
			</div>
		</>
	);
}

export function WindowTitle({
	title,
	children,
}: {
	title: string;
	children?: ReactNode;
}) {
	return (
		<header
			className="viz-native-window-title"
			data-tauri-drag-region
			onPointerDown={(event) => {
				if (
					(event.target as HTMLElement).closest(
						"button, input, select, textarea, [role='button']",
					)
				)
					return;
				beginWindowDrag(event);
			}}
		>
			<span className="viz-window-title-copy">{title}</span>
			{children}
		</header>
	);
}
