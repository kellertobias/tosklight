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
