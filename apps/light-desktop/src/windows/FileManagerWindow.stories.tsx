import type { Meta, StoryObj } from "@storybook/react-vite";
import type { FileDirectory, FileEntry } from "../api/types";
import { FilesProvider } from "../features/files/FilesContext";
import type { FilesContextValue } from "../features/files/types";
import { FileManager } from "./FileManagerWindow";

const entries: FileEntry[] = [
	{
		name: "Tour 2026",
		path: "Tour 2026",
		kind: "folder",
		size: 0,
		modified_millis: Date.UTC(2026, 6, 26, 18, 30),
		created_millis: Date.UTC(2026, 5, 1),
		hidden: false,
		writable: true,
	},
	{
		name: "Opening Night.show",
		path: "Opening Night.show",
		kind: "file",
		size: 1_284_550,
		modified_millis: Date.UTC(2026, 6, 26, 20, 15),
		created_millis: Date.UTC(2026, 6, 20),
		hidden: false,
		writable: true,
	},
	{
		name: "Cue notes.md",
		path: "Cue notes.md",
		kind: "file",
		size: 4_812,
		modified_millis: Date.UTC(2026, 6, 25, 11, 45),
		created_millis: Date.UTC(2026, 6, 25),
		hidden: false,
		writable: true,
	},
];

const directory: FileDirectory = {
	root_id: "shows",
	path: "",
	entries,
};

const source: FilesContextValue = {
	status: "connected",
	systemPickerFallback: false,
	fileRoots: async () => [{
		id: "shows",
		label: "Shows",
		icon: "shows",
		removable: false,
		writable: true,
	}],
	fileEntries: async () => directory,
	fileMetadata: async (_root, path) => ({
		...(entries.find((entry) => entry.path === path) ?? entries[0]),
		root_id: "shows",
		capabilities: {
			created_time: true,
			hidden_attributes: true,
			native_notes: true,
			trash: true,
			range_streaming: true,
			thumbnails: true,
		},
	}),
	readFileNote: async (_root, path) => ({
		root_id: "shows",
		path,
		supported: true,
		note: "Prepared for the evening performance.",
	}),
	saveFileNote: async (_root, path, note) => ({
		root_id: "shows",
		path,
		supported: true,
		note,
	}),
	readTextFile: async (_root, path) => ({
		root_id: "shows",
		path,
		text: "# Cue notes\n\nCheck the house preset before doors.",
		revision: "storybook",
		read_only: false,
	}),
	saveTextFile: async (_root, path, text) => ({
		root_id: "shows",
		path,
		text,
		revision: "storybook-next",
		read_only: false,
	}),
	fileOperation: async () => ({ paths: [], complete: true, items: [] }),
	fileContent: async () => new Blob(["deterministic"]),
	fileStreamUrl: async () => "data:audio/wav;base64,",
	fileThumbnail: async () => new Blob(),
	claimFileInput: async (instanceId, action) => ({
		instance_id: instanceId,
		action,
		session_id: "storybook-session",
		desk_id: "storybook-desk",
		expires_in_millis: 60_000,
	}),
	releaseFileInput: async () => undefined,
};

const meta = {
	title: "Application/Windows/File Manager",
	component: FileManager,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	decorators: [
		(Story) => (
			<FilesProvider source={source}>
				<div style={{ width: "100vw", height: "100vh" }}>
					<Story />
				</div>
			</FilesProvider>
		),
	],
	args: { active: true, instanceId: "storybook-file-manager" },
} satisfies Meta<typeof FileManager>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Populated: Story = {};

export const Narrow: Story = {
	parameters: { viewport: { defaultViewport: "mobile1" } },
};
