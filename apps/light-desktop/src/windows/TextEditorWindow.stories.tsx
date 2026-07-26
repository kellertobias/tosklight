import type { Meta, StoryObj } from "@storybook/react-vite";
import { useRef, useState } from "react";
import type { TextEditorController } from "./textEditorWindow/controller";
import { TextEditorWindowView } from "./TextEditorWindow";

function TextEditorStory() {
	const initial = "# Cue notes\n\n- Confirm the house preset\n- Check follow-spot channel\n\n`GO` starts the next cue.";
	const [text, setText] = useState(initial);
	const textarea = useRef<HTMLTextAreaElement>(null);
	const controller = {
		availability: "ready",
		chooserFiles: [{
			name: "Cue notes.md",
			path: "Cue notes.md",
			kind: "file",
			size: initial.length,
			modified_millis: Date.UTC(2026, 6, 26, 20, 15),
			created_millis: Date.UTC(2026, 6, 20),
			hidden: false,
			writable: true,
		}],
		dirty: text !== initial,
		document: {
			root_id: "shows",
			path: "Cue notes.md",
			text: initial,
			revision: "storybook",
			read_only: false,
		},
		editorMode: "split",
		externalDocument: null,
		filesLoading: false,
		label: "Cue notes.md",
		messageId: "storybook-text-editor-message",
		notice: null,
		paneChrome: null,
		paneReadOnly: false,
		roots: [{
			id: "shows",
			label: "Shows",
			icon: "shows",
			removable: false,
			writable: true,
		}],
		saving: false,
		selectedPath: "Cue notes.md",
		selectedRoot: "shows",
		status: text === initial ? "Saved" : "Unsaved",
		text,
		textarea,
		associateFile: () => undefined,
		changeText: setText,
		openFile: async () => undefined,
		persistViewState: () => undefined,
		recreate: () => undefined,
		reloadExternal: () => undefined,
		reloadFiles: async () => undefined,
		save: () => setText(text),
		saveAs: () => undefined,
	} as unknown as TextEditorController;
	return <TextEditorWindowView controller={controller} />;
}

const meta = {
	title: "Application/Windows/Text Editor",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	render: () => <div style={{ height: 713 }}><TextEditorStory /></div>,
} satisfies Meta;

export default meta;
type Story = StoryObj;

export const SplitMarkdown: Story = {};

export const Narrow: Story = {
	parameters: { viewport: { defaultViewport: "mobile1" } },
};
