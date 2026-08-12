import type { Meta, StoryObj } from "@storybook/react-vite";
import { SwitchField } from "@tosklight/ui/controls";
import { type ReactNode, useState } from "react";
import { StatefulMediaStory } from "../../../light-desktop/src/windows/MediaPaneWindow.stories";
import {
	allocateFreeAddresses,
	LibraryBrowserView,
} from "../features/media-library/LibraryPage";
import type { CatalogView } from "../shared/api/generated/media-wire";
import {
	DashboardScreen,
	LibrariesSettings,
	LogsSettings,
	NetworkInputsSettings,
	type OperatorTextSource,
	OutputsSettings,
	SettingsScreen,
	TextScreen,
	VisualizersScreen,
} from "./MediaServerScreens";
import {
	MediaListDetail,
	MediaMetric,
	MediaPanel,
	MediaPreview,
	type MediaServerSection,
	MediaServerShell,
} from "./MediaServerSurface";
import "../styles.css";
import "./mediaServerSurface.css";

const NOW = new Date("2026-08-12T21:31:00+02:00");

const MEDIA_STORY_BY_SECTION: Record<MediaServerSection, string> = {
	dashboard: "dashboard",
	media: "media",
	library: "library",
	visualizers: "visualizers",
	text: "text",
	settings: "settings-libraries",
};

const SETTINGS_STORY_BY_SECTION = {
	libraries: "settings-libraries",
	outputs: "settings-outputs",
	"network-inputs": "settings-network-and-inputs",
	logs: "settings-logs",
} as const;

function mediaServerStoryPath(section: MediaServerSection) {
	return `/?path=/story/tosklight-media-server--${MEDIA_STORY_BY_SECTION[section]}`;
}

function openStory(story: string) {
	(window.top ?? window).location.assign(
		`/?path=/story/tosklight-media-server--${story}`,
	);
}

const outputs = [
	{
		id: "main",
		name: "Main output",
		target: "Display 2",
		resolution: "1920 × 1080",
		status: "Live",
		layers: [
			{ id: "1", name: "Layer 1", source: "Storm Clouds", level: "100%" },
			{ id: "2", name: "Layer 2", source: "Prospero title", level: "82%" },
			{ id: "3", name: "Layer 3", source: "Aurora Field", level: "35%" },
		],
	},
	{
		id: "foh",
		name: "FOH confidence",
		target: "Off-screen surface",
		resolution: "1280 × 720",
		status: "Live",
		layers: [
			{ id: "1", name: "Layer 1", source: "Running order", level: "100%" },
			{ id: "2", name: "Layer 2", source: "Blank", level: "0%" },
		],
	},
];

const library = [
	{
		id: "storm",
		name: "Storm Clouds",
		address: "1 / 12",
		type: "HAP Alpha video",
	},
	{
		id: "island",
		name: "Island sunrise",
		address: "1 / 18",
		type: "HAP Alpha video",
	},
	{ id: "cloth", name: "Golden cloth", address: "2 / 4", type: "Still image" },
	{
		id: "mask",
		name: "Proscenium mask",
		address: "10 / 1",
		type: "Alpha mask",
	},
];

const visualizers = [
	{
		id: "aurora",
		name: "Aurora Field",
		address: "220 / 3",
		kind: "Reactive field",
		controls: ["Speed", "Amount", "Reactivity", "Smoothing"],
		variant: "aurora" as const,
	},
	{
		id: "particles",
		name: "Ember Particles",
		address: "220 / 4",
		kind: "Particle system",
		controls: ["Count", "Size", "Gravity", "Lifetime"],
		variant: "particles" as const,
	},
	{
		id: "equalizer",
		name: "Band Equalizer",
		address: "220 / 5",
		kind: "Audio bars",
		controls: ["Amount", "Decay", "Smoothing", "Mirror"],
		variant: "particles" as const,
	},
];

const initialText: OperatorTextSource[] = [
	{
		id: "prospero",
		name: "Prospero title",
		address: "200 / 3",
		kind: "Fixed words",
		text: "We are such stuff\nas dreams are made on",
		enabled: true,
	},
	{
		id: "interval",
		name: "Interval countdown",
		address: "200 / 4",
		kind: "Countdown",
		text: "Interval · 12:34",
		enabled: true,
	},
	{
		id: "doors",
		name: "Doors open",
		address: "200 / 5",
		kind: "Clock",
		text: "Doors 19:00",
		enabled: false,
	},
];

function Frame({
	active,
	children,
}: {
	active: MediaServerSection;
	children: ReactNode;
}) {
	return (
		<div className="marketing-screenshot-viewport">
			<MediaServerShell
				active={active}
				connected
				instance="Media Server · Stage Rack"
				now={NOW}
				onNavigate={(section) =>
					(window.top ?? window).location.assign(mediaServerStoryPath(section))
				}
			>
				{children}
			</MediaServerShell>
		</div>
	);
}

function StatefulLibrary() {
	const [catalog, setCatalog] = useState<CatalogView>(storyCatalog);
	return (
		<Frame active="library">
			<LibraryBrowserView
				catalog={catalog}
				thumbnailUrl={(_, file) => storyThumbnail(file)}
				onRenameFolder={(folder, name) =>
					setCatalog((current) => withStoryFolderName(current, folder, name))
				}
				onUpdateItem={(item, update) =>
					setCatalog((current) => withStoryItem(current, item.id, update))
				}
				onMoveItems={(items, folder) =>
					setCatalog((current) => moveStoryItems(current, items, folder))
				}
				onUpload={(files, folder) =>
					setCatalog((current) => uploadStoryFiles(current, files, folder))
				}
			/>
		</Frame>
	);
}

function withStoryFolderName(
	catalog: CatalogView,
	folder: number,
	name: string,
): CatalogView {
	const existing = catalog.folders.find((entry) => entry.folder === folder);
	return {
		...catalog,
		folders: existing
			? catalog.folders.map((entry) =>
					entry.folder === folder ? { ...entry, name } : entry,
				)
			: [...catalog.folders, { folder, name, items: [] }],
	};
}

function withStoryItem(
	catalog: CatalogView,
	id: string,
	update: { name?: string; intrinsicBpm?: number | null },
): CatalogView {
	return {
		...catalog,
		folders: catalog.folders.map((folder) => ({
			...folder,
			items: folder.items.map((item) =>
				item.id === id ? { ...item, ...update } : item,
			),
		})),
	};
}

function moveStoryItems(
	catalog: CatalogView,
	items: CatalogView["folders"][number]["items"],
	folder: number,
): CatalogView {
	if (
		items.every((item) =>
			catalog.folders
				.find((entry) => entry.folder === folder)
				?.items.some((candidate) => candidate.id === item.id),
		)
	)
		return catalog;
	const addresses = allocateFreeAddresses(catalog, folder, items);
	const moving = new Set(items.map((item) => item.id));
	const placements = new Map(
		items.flatMap((item, index) => {
			const address = addresses[index];
			return address ? [[item.id, address] as const] : [];
		}),
	);
	const folderNumbers = new Set([
		...catalog.folders.map((entry) => entry.folder),
		...addresses.map((address) => address.folder),
	]);
	return {
		...catalog,
		revision: catalog.revision + 1,
		folders: [...folderNumbers]
			.sort((left, right) => left - right)
			.map((number) => {
				const existing = catalog.folders.find(
					(entry) => entry.folder === number,
				);
				const arrivals = items.flatMap((item) => {
					const address = placements.get(item.id);
					return address?.folder === number
						? [{ ...item, file: address.file }]
						: [];
				});
				return {
					folder: number,
					name: existing?.name ?? "",
					items: [
						...(existing?.items ?? []).filter((item) => !moving.has(item.id)),
						...arrivals,
					].sort((left, right) => left.file - right.file),
				};
			}),
	};
}

function uploadStoryFiles(
	catalog: CatalogView,
	files: readonly File[],
	folder: number,
): CatalogView {
	const addresses = allocateFreeAddresses(catalog, folder, [], files.length);
	const additions = files.flatMap((file, index) => {
		const address = addresses[index];
		return address
			? [
					{
						address,
						item: {
							id: `story-upload-${catalog.revision}-${index}`,
							file: address.file,
							name: file.name.replace(/\.[^.]+$/u, ""),
							kind: file.type.startsWith("image/") ? "image" : "video",
							width: 1920,
							height: 1080,
							frames: file.type.startsWith("image/") ? null : 1_800,
							intrinsicBpm: null,
						},
					},
				]
			: [];
	});
	let next = catalog;
	for (const addition of additions) {
		const existing = next.folders.find(
			(entry) => entry.folder === addition.address.folder,
		);
		next = {
			...next,
			folders: existing
				? next.folders.map((entry) =>
						entry.folder === addition.address.folder
							? {
									...entry,
									items: [...entry.items, addition.item].sort(
										(left, right) => left.file - right.file,
									),
								}
							: entry,
					)
				: [
						...next.folders,
						{
							folder: addition.address.folder,
							name: "",
							items: [addition.item],
						},
					],
		};
	}
	return {
		...next,
		revision: catalog.revision + 1,
		itemCount: catalog.itemCount + additions.length,
	};
}

function storyThumbnail(file: number) {
	const hue = (file * 37) % 360;
	return `data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 160 90"><defs><linearGradient id="g"><stop stop-color="hsl(${hue} 75% 38%)"/><stop offset="1" stop-color="#070b12"/></linearGradient></defs><rect width="160" height="90" fill="url(#g)"/><circle cx="${35 + (file % 70)}" cy="45" r="23" fill="hsl(${(hue + 80) % 360} 80% 62%)" opacity=".7"/></svg>`)}`;
}

function StatefulPlayback() {
	const [takeover, setTakeover] = useState(false);
	return (
		<Frame active="media">
			<StatefulMediaStory
				embedded
				showNotice={false}
				title="Playback"
				headerAction={
					<SwitchField
						className="media-playback-takeover"
						label="Take over playback"
						aria-label="Take over playback"
						offLabel="Take over playback"
						onLabel="Release"
						checked={takeover}
						onChange={(event) => setTakeover(event.target.checked)}
					/>
				}
			/>
		</Frame>
	);
}

const storyCatalog = {
	revision: 12,
	itemCount: 4,
	folders: [
		{
			folder: 1,
			name: "Show content",
			items: library.map((item, index) => ({
				id: item.id,
				file: [12, 18, 24, 31][index] ?? index + 1,
				name: item.name,
				kind: index === 2 ? "image" : "video",
				width: 1920,
				height: 1080,
				frames: index === 2 ? null : 3_600,
				intrinsicBpm: index === 0 ? 120 : null,
			})),
		},
	],
} satisfies CatalogView;

function StatefulVisualizers() {
	const [selected, setSelected] = useState("aurora");
	return (
		<Frame active="visualizers">
			<VisualizersScreen
				items={visualizers}
				selectedId={selected}
				onSelect={setSelected}
			/>
		</Frame>
	);
}

function StatefulText() {
	const [items, setItems] = useState(initialText);
	const [selected, setSelected] = useState("prospero");
	return (
		<Frame active="text">
			<TextScreen
				items={items}
				selectedId={selected}
				onSelect={setSelected}
				onTextChange={(text) =>
					setItems((current) =>
						current.map((item) =>
							item.id === selected ? { ...item, text } : item,
						),
					)
				}
			/>
		</Frame>
	);
}

const meta = {
	title: "ToskLight/Media Server",
	parameters: { layout: "fullscreen" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

export const Components: Story = {
	render: () => (
		<div style={{ padding: 24, display: "grid", gap: 16 }}>
			<div className="media-metric-grid">
				<MediaMetric
					label="Connected desk"
					value="The Tempest"
					detail="Active show"
					tone="good"
				/>
				<MediaMetric
					label="DMX input"
					value="44 Hz"
					detail="Art-Net · Universe 1"
				/>
				<MediaMetric label="Library" value="286" detail="Addressable items" />
				<MediaMetric
					label="Output 2"
					value="Restart"
					detail="Stored picture changed"
					tone="warn"
				/>
			</div>
			<MediaPanel title="Operator panel" detail="Production surface grouping">
				<p>
					Panels, metrics, previews, and list selection share one Media Server
					language.
				</p>
			</MediaPanel>
			<MediaPreview title="Aurora Field" />
			<MediaListDetail
				label="Component example"
				items={visualizers.map((item) => ({
					id: item.id,
					title: item.name,
					detail: item.kind,
					meta: item.address,
				}))}
				selectedId="aurora"
				detail={<p>Selected detail remains visible on the right.</p>}
			/>
		</div>
	),
};

export const Dashboard: Story = {
	render: () => (
		<Frame active="dashboard">
			<DashboardScreen
				instance="Stage Rack"
				showName="The Tempest"
				outputs={outputs}
				libraryItems={286}
				dmxRate="44 Hz"
				recent={
					<ul>
						<li>23:31 · Light Desk identified The Tempest</li>
						<li>23:29 · Main output synchronized to Display 2</li>
						<li>23:27 · Storm Clouds import finished</li>
					</ul>
				}
			/>
		</Frame>
	),
};

export const Media: Story = {
	name: "Playback",
	render: () => <StatefulPlayback />,
};
export const Library: Story = { render: () => <StatefulLibrary /> };
export const Visualizers: Story = { render: () => <StatefulVisualizers /> };
export const Text: Story = { render: () => <StatefulText /> };

export const SettingsLibraries: Story = {
	render: () => (
		<Frame active="settings">
			<SettingsScreen
				active="libraries"
				onSelect={(section) => openStory(SETTINGS_STORY_BY_SECTION[section])}
			>
				<LibrariesSettings />
			</SettingsScreen>
		</Frame>
	),
};
export const SettingsOutputs: Story = {
	render: () => (
		<Frame active="settings">
			<SettingsScreen
				active="outputs"
				onSelect={(section) => openStory(SETTINGS_STORY_BY_SECTION[section])}
			>
				<OutputsSettings />
			</SettingsScreen>
		</Frame>
	),
};
export const SettingsNetworkAndInputs: Story = {
	render: () => (
		<Frame active="settings">
			<SettingsScreen
				active="network-inputs"
				onSelect={(section) => openStory(SETTINGS_STORY_BY_SECTION[section])}
			>
				<NetworkInputsSettings />
			</SettingsScreen>
		</Frame>
	),
};
export const SettingsLogs: Story = {
	render: () => (
		<Frame active="settings">
			<SettingsScreen
				active="logs"
				onSelect={(section) => openStory(SETTINGS_STORY_BY_SECTION[section])}
			>
				<LogsSettings />
			</SettingsScreen>
		</Frame>
	),
};
