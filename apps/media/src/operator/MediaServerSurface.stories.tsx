import type { Meta, StoryObj } from "@storybook/react-vite";
import {
	Button,
	NumberField,
	SwitchField,
	TextAreaField,
} from "@tosklight/ui/controls";
import { WindowFrame } from "@tosklight/ui/window-kit";
import { type ReactNode, useState } from "react";
import { StatefulMediaStory } from "../../../light-desktop/src/windows/MediaPaneWindow.stories";
import { AudioMeters } from "../features/audio/AudioMeters";
import {
	GeneratedLibraryBrowserView,
	type LibrarySourceType,
} from "../features/media-library/GeneratedLibraryBrowserView";
import {
	allocateFreeAddresses,
	LibraryBrowserView,
} from "../features/media-library/LibraryPage";
import type {
	AudioView,
	CatalogView,
} from "../shared/api/generated/media-wire";
import {
	LibrariesSettings,
	LogsSettings,
	NetworkInputsSettings,
	OutputsSettings,
	SettingsScreen,
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
	media: "media",
	library: "library",
	audio: "audio",
	dmx: "settings-dmx-input",
	settings: "settings-libraries",
};

const SETTINGS_STORY_BY_SECTION = {
	libraries: "settings-libraries",
	"picture-output": "settings-outputs",
	"sound-output": "settings-outputs",
	network: "settings-network-and-inputs",
	dmx: "settings-dmx-input",
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

function openLibraryType(type: LibrarySourceType) {
	openStory(type === "media" ? "library" : type);
}

const audioAnalysis = {
	capturing: true,
	device: "USB Audio CODEC",
	detail: null,
	waveform: {
		points: [
			0, 0.28, -0.12, 0.64, -0.48, 0.2, 0.78, -0.32, 0.1, 0.52, -0.68, 0.34,
			0.08, -0.22, 0.58, -0.14, 0,
		],
	},
	spectrum: [
		0.78, 0.86, 0.72, 0.63, 0.58, 0.66, 0.54, 0.47, 0.41, 0.35, 0.3, 0.24, 0.2,
		0.17, 0.13, 0.09,
	],
	bands: { bass: 0.82, mid: 0.61, treble: 0.38 },
	energy: 0.69,
	peak: 0.91,
	beat: 0.84,
	bpm: 124.8,
	beatPhase: 0.32,
} satisfies AudioView;

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
		address: "250 / 3",
		folder: 250,
		file: 3,
		kind: "Reactive field",
		controls: ["Speed", "Amount", "Reactivity", "Smoothing"],
		variant: "aurora" as const,
	},
	{
		id: "particles",
		name: "Ember Particles",
		address: "250 / 4",
		folder: 250,
		file: 4,
		kind: "Particle system",
		controls: ["Count", "Size", "Gravity", "Lifetime"],
		variant: "particles" as const,
	},
	{
		id: "equalizer",
		name: "Band Equalizer",
		address: "250 / 5",
		folder: 250,
		file: 5,
		kind: "Audio bars",
		controls: ["Amount", "Decay", "Smoothing", "Mirror"],
		variant: "particles" as const,
	},
];

const initialText = [
	{
		id: "prospero",
		name: "Prospero title",
		address: "200 / 3",
		folder: 200,
		file: 3,
		kind: "Fixed words",
		text: "We are such stuff\nas dreams are made on",
		enabled: true,
	},
	{
		id: "interval",
		name: "Interval countdown",
		address: "200 / 4",
		folder: 200,
		file: 4,
		kind: "Countdown",
		text: "Interval · 12:34",
		enabled: true,
	},
	{
		id: "doors",
		name: "Doors open",
		address: "200 / 5",
		folder: 200,
		file: 5,
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
				onModeChange={openLibraryType}
				catalog={catalog}
				thumbnailUrl={(_, file) => storyThumbnail(file)}
				onRenameFolder={(folder, name) =>
					setCatalog((current) => withStoryFolderName(current, folder, name))
				}
				onSetFolderIcon={(folder, icon) =>
					setCatalog((current) => withStoryFolderIcon(current, folder, icon))
				}
				onSwapFolders={(first, second) =>
					setCatalog((current) => swapStoryFolders(current, first, second))
				}
				onUpdateItem={(item, update) =>
					setCatalog((current) => withStoryItem(current, item.id, update))
				}
				onMoveItems={(items, folder) =>
					setCatalog((current) => moveStoryItems(current, items, folder))
				}
				onReorderItem={(item, destination) =>
					setCatalog((current) =>
						reorderStoryItem(
							current,
							item.id,
							destination.folder,
							destination.file,
						),
					)
				}
				onUpload={(files, folder) =>
					setCatalog((current) => uploadStoryFiles(current, files, folder))
				}
			/>
		</Frame>
	);
}

function swapStoryFolders(
	catalog: CatalogView,
	first: number,
	second: number,
): CatalogView {
	return {
		...catalog,
		revision: catalog.revision + 1,
		folders: catalog.folders
			.map((entry) => ({
				...entry,
				folder:
					entry.folder === first
						? second
						: entry.folder === second
							? first
							: entry.folder,
			}))
			.sort((left, right) => left.folder - right.folder),
	};
}

function reorderStoryItem(
	catalog: CatalogView,
	id: string,
	folder: number,
	file: number,
): CatalogView {
	const source = catalog.folders
		.flatMap((entry) =>
			entry.items.map((item) => ({ folder: entry.folder, item })),
		)
		.find((entry) => entry.item.id === id);
	const target = catalog.folders
		.find((entry) => entry.folder === folder)
		?.items.find((item) => item.file === file);
	if (!source || !target) return catalog;
	return {
		...catalog,
		revision: catalog.revision + 1,
		folders: catalog.folders.map((entry) => ({
			...entry,
			items: entry.items
				.map((item) =>
					item.id === id
						? { ...item, file }
						: item.id === target.id
							? { ...item, file: source.item.file }
							: item,
				)
				.sort((left, right) => left.file - right.file),
		})),
	};
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

function withStoryFolderIcon(
	catalog: CatalogView,
	folder: number,
	icon: string,
): CatalogView {
	return {
		...catalog,
		folders: catalog.folders.map((entry) =>
			entry.folder === folder ? { ...entry, icon } : entry,
		),
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
						offLabel="Release"
						onLabel="Take over playback"
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
	itemCount: 5,
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
		{
			folder: 900,
			name: "Parking",
			items: [
				{
					id: "parked-archive",
					file: 1,
					name: "Unused finale alternate",
					kind: "video",
					width: 1920,
					height: 1080,
					frames: 2_400,
					intrinsicBpm: 128,
				},
			],
		},
	],
} satisfies CatalogView;

function StatefulVisualizers() {
	const [selected, setSelected] = useState("aurora");
	const item = visualizers.find((candidate) => candidate.id === selected);
	return (
		<Frame active="library">
			<GeneratedLibraryBrowserView
				type="visualizers"
				items={visualizers.map((visualizer) => ({
					id: visualizer.id,
					folder: visualizer.folder,
					file: visualizer.file,
					name: visualizer.name,
					detail: visualizer.kind,
				}))}
				selectedId={selected}
				onSelect={setSelected}
				onTypeChange={openLibraryType}
				detail={
					item ? (
						<div className="media-library-editor media-generated-library-detail">
							<MediaPreview title={item.name} variant={item.variant} />
							<h2>{item.name}</h2>
							<p>
								{item.address} · {item.kind}
							</p>
							{item.controls.map((control, index) => (
								<NumberField
									key={control}
									label={control}
									value={String(72 - index * 9)}
								/>
							))}
							<Button variant="primary">Save visualizer</Button>
						</div>
					) : null
				}
				emptyDetail={<p>No visualizer is assigned in this folder.</p>}
			/>
		</Frame>
	);
}

function StatefulText() {
	const [items, setItems] = useState(initialText);
	const [selected, setSelected] = useState("prospero");
	const item = items.find((candidate) => candidate.id === selected);
	return (
		<Frame active="library">
			<GeneratedLibraryBrowserView
				type="text"
				items={items.map((text) => ({
					id: text.id,
					folder: text.folder,
					file: text.file,
					name: text.name,
					detail: text.kind,
				}))}
				selectedId={selected}
				onSelect={setSelected}
				onTypeChange={openLibraryType}
				headerActions={[
					{
						id: "new-text-source",
						label: "New text source",
						onPress: () => undefined,
					},
				]}
				detail={
					item ? (
						<div className="media-library-editor media-generated-library-detail">
							<MediaPreview title={item.name} variant="text">
								<span className="media-text-preview-words">{item.text}</span>
							</MediaPreview>
							<h2>{item.name}</h2>
							<p>
								{item.address} · {item.kind}
							</p>
							<TextAreaField
								label="Words"
								value={item.text}
								onChange={(event) =>
									setItems((current) =>
										current.map((candidate) =>
											candidate.id === selected
												? { ...candidate, text: event.target.value }
												: candidate,
										),
									)
								}
							/>
							<Button variant="primary">Save text</Button>
						</div>
					) : null
				}
				emptyDetail={<p>No text source is assigned in this folder.</p>}
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

export const Media: Story = {
	name: "Playback",
	render: () => <StatefulPlayback />,
};
export const Library: Story = { render: () => <StatefulLibrary /> };
export const Visualizers: Story = { render: () => <StatefulVisualizers /> };
export const Text: Story = { render: () => <StatefulText /> };
export const Audio: Story = {
	render: () => (
		<Frame active="audio">
			<WindowFrame
				title="Audio"
				info={{
					primary: "USB Audio CODEC",
					secondary: "Live audio analysis",
				}}
				className="media-route-window"
			>
				<section className="media-page">
					<AudioMeters audio={audioAnalysis} live />
				</section>
			</WindowFrame>
		</Frame>
	),
};

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
				active="picture-output"
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
				active="network"
				onSelect={(section) => openStory(SETTINGS_STORY_BY_SECTION[section])}
			>
				<NetworkInputsSettings active="network" />
			</SettingsScreen>
		</Frame>
	),
};
export const SettingsDmxInput: Story = {
	render: () => (
		<Frame active="settings">
			<SettingsScreen
				active="dmx"
				onSelect={(section) => openStory(SETTINGS_STORY_BY_SECTION[section])}
			>
				<NetworkInputsSettings active="dmx" />
			</SettingsScreen>
		</Frame>
	),
};
export const SettingsAudioInput: Story = {
	render: () => (
		<Frame active="settings">
			<SettingsScreen
				active="network"
				onSelect={(section) => openStory(SETTINGS_STORY_BY_SECTION[section])}
			>
				<NetworkInputsSettings
					active="audio"
					audioMonitor={<AudioMeters audio={audioAnalysis} live />}
				/>
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
				<LogsSettings active="logs" />
			</SettingsScreen>
		</Frame>
	),
};
export const DmxDiagnostics: Story = {
	render: () => (
		<Frame active="dmx">
			<WindowFrame
				title="Diagnostics"
				info={{ primary: "DMX", secondary: "Input and channel diagnostics" }}
			>
				<LogsSettings active="dmx-diagnostics" />
			</WindowFrame>
		</Frame>
	),
};
