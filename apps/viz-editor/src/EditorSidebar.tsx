import { Button } from "@tosklight/ui";
import { OperatorDestinationList } from "@tosklight/ui/application";
import { useState } from "react";
import appIcon from "../src-tauri/icons/icon.svg";
import { beginWindowDrag } from "./WindowChrome";

export type EditorWorkspace =
	| "show"
	| "cad"
	| "patch"
	| "venue"
	| "effects"
	| "media"
	| "settings";

/// The screens the Architect offers, in the order the operator reads them.
///
/// Everything but Show needs a document, so the entries stay visible and go disabled instead of
/// disappearing: the operator sees what opening a show would give them.
export function EditorSidebar({
	filename,
	workspace,
	hasDocument,
	onSelectWorkspace,
	onSelectSettings,
	openWindow,
	openVisualizer,
}: {
	filename: string;
	workspace: EditorWorkspace;
	hasDocument: boolean;
	onSelectWorkspace: (workspace: EditorWorkspace) => void;
	onSelectSettings: () => void;
	openWindow: () => Promise<void>;
	openVisualizer: () => Promise<void>;
}) {
	const [openingWindow, setOpeningWindow] = useState(false);
	const [openingViz, setOpeningViz] = useState(false);

	return (
		<aside className="viz-editor-sidebar">
			<div
				className="viz-editor-identity"
				data-tauri-drag-region
				title={filename}
				onPointerDown={beginWindowDrag}
			>
				<img
					src={appIcon}
					alt="ToskLight Architect"
					className="viz-editor-app-icon"
				/>
				<span>{filename}</span>
			</div>
			<OperatorDestinationList
				ariaLabel="Visualizer screens"
				activeId={workspace}
				onSelect={(id) => onSelectWorkspace(id as EditorWorkspace)}
				entries={[
					{ id: "show", label: "Show", icon: <span>◫</span> },
					{
						id: "cad",
						label: "CAD",
						icon: <span>⊞</span>,
						disabled: !hasDocument,
					},
					{
						id: "patch",
						label: "Patch",
						icon: <span>⌘</span>,
						disabled: !hasDocument,
					},
					{
						id: "venue",
						label: "Venue",
						icon: <span>◇</span>,
						disabled: !hasDocument,
					},
					{
						id: "effects",
						label: "Effects",
						icon: <span>✦</span>,
						disabled: !hasDocument,
					},
					{
						id: "media",
						label: "Media",
						icon: <span>▣</span>,
						disabled: !hasDocument,
					},
				]}
			/>
			<Button
				className="viz-editor-settings-nav"
				active={workspace === "settings"}
				onClick={onSelectSettings}
			>
				Settings
			</Button>
			<Button
				className="viz-editor-open-window"
				title="Open another window on this show"
				disabled={openingWindow}
				onClick={() => {
					setOpeningWindow(true);
					openWindow().finally(() => setOpeningWindow(false));
				}}
			>
				{openingWindow ? "Opening…" : "Open Window"}
			</Button>
			<Button
				className="viz-editor-open-viz"
				disabled={!hasDocument || openingViz}
				onClick={() => {
					setOpeningViz(true);
					openVisualizer().finally(() => setOpeningViz(false));
				}}
			>
				{openingViz ? "Opening Viz…" : "Open Viz"}
			</Button>
		</aside>
	);
}
