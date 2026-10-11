import type { PropsWithChildren, ReactNode } from "react";

/** One authored grid column; only an active Solo editor reserves a top row. */
export function ScreenPlaybackDraftLayout({
	editor,
	children,
}: PropsWithChildren<{
	editor: ReactNode;
}>) {
	return (
		<div className="screen-playback-draft-layout">
			<div className="screen-playback-draft-editor">{editor}</div>
			{children}
		</div>
	);
}
