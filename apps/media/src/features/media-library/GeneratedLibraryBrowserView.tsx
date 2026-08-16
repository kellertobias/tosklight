import type { TitleAction, TitleActionGroup } from "@tosklight/ui/controls";
import {
	DEFAULT_POOL_COLOR_PALETTE,
	PoolCard,
	PoolGrid,
} from "@tosklight/ui/pools";
import {
	ButtonGrid,
	WindowFrame,
	WindowScrollArea,
} from "@tosklight/ui/window-kit";
import { type ReactNode, useMemo, useState } from "react";
import type { FolderPresentation } from "./FolderPresentationEditor";

export type LibrarySourceType = "media" | "visualizers" | "text";

export interface GeneratedLibraryItem {
	id: string;
	folder: number;
	file: number;
	name: string;
	detail: string;
	preview?: ReactNode;
	image?: { src: string; alt: string };
}

export function librarySourceGroups({
	value,
	onChange,
	actions = [],
}: {
	value: LibrarySourceType;
	onChange?: (value: LibrarySourceType) => void;
	actions?: TitleAction[];
}): TitleActionGroup[] {
	return [
		...(actions.length
			? [{ id: "library-actions", actions } satisfies TitleActionGroup]
			: []),
		{
			id: "library-source-type",
			kind: "tabs",
			activeId: value,
			onActiveChange: (next) => onChange?.(next as LibrarySourceType),
			actions: [
				{ id: "media", label: "Media" },
				{ id: "visualizers", label: "Visualizers" },
				{ id: "text", label: "Text" },
			],
		},
	];
}

export function GeneratedLibraryBrowserView({
	type,
	items,
	selectedId,
	onSelect,
	onSelectSlot,
	onTypeChange,
	detail,
	headerActions,
	folderPresentations = [],
	inspectedFolder,
	onInspectFolder,
	renderFolderDetail,
	emptyDetail,
	showDetail = false,
}: {
	type: Exclude<LibrarySourceType, "media">;
	items: GeneratedLibraryItem[];
	selectedId: string;
	onSelect?: (id: string) => void;
	onSelectSlot?: (slot: {
		folder: number;
		file: number;
		itemId?: string;
	}) => void;
	onTypeChange?: (value: LibrarySourceType) => void;
	detail: ReactNode;
	headerActions?: TitleAction[];
	folderPresentations?: FolderPresentation[];
	inspectedFolder?: number | null;
	onInspectFolder?: (folder: number | null) => void;
	renderFolderDetail?: (folder: number) => ReactNode;
	emptyDetail: ReactNode;
	showDetail?: boolean;
}) {
	const folders = type === "text" ? range(200, 249) : range(250, 255);
	const selected = items.find((item) => item.id === selectedId);
	const [folder, setFolder] = useState(selected?.folder ?? folders[0]);
	const [inspectingFolder, setInspectingFolder] = useState<number | null>(null);
	const activeFolder = inspectedFolder ?? folder;
	const activeInspector =
		inspectedFolder === undefined ? inspectingFolder : inspectedFolder;

	const visible = useMemo(
		() => items.filter((item) => item.folder === activeFolder),
		[activeFolder, items],
	);
	const visibleSelected = visible.some((item) => item.id === selectedId);
	const sourceColor =
		type === "text"
			? DEFAULT_POOL_COLOR_PALETTE.macro
			: DEFAULT_POOL_COLOR_PALETTE.dynamic;

	return (
		<WindowFrame
			title="Library"
			info={{
				primary: type === "text" ? "Text sources" : "Generated visualizers",
				secondary:
					type === "text"
						? "Folders 200–249 · 12,700 addressable sources"
						: "Folders 250–255 · 1,524 addressable sources",
			}}
			groups={librarySourceGroups({
				value: type,
				onChange: onTypeChange,
				actions: headerActions,
			})}
			className="media-library-window"
		>
			<div className="media-catalog-browser">
				<WindowScrollArea className="media-library-folders">
					<div className="media-library-pool-heading">
						<span>Folders</span>
						<small>{type === "text" ? "200–249" : "250–255"}</small>
					</div>
					<ButtonGrid className="media-library-folder-pool" minimum={68}>
						{folders.map((number) => {
							const presentation = folderPresentations.find(
								(candidate) => candidate.folder === number,
							);
							const count = items.filter(
								(item) => item.folder === number,
							).length;
							return (
								<PoolCard
									key={number}
									model={{
										number: String(number).padStart(3, "0"),
										primary:
											presentation?.name ??
											(type === "text" ? "Text" : "Visualizers"),
										secondary: `${count}/254`,
										icon: presentation?.icon ?? undefined,
										image: presentation?.pictureUrl
											? {
													src: presentation.pictureUrl,
													alt: `${presentation.name ?? `Folder ${number}`} preview`,
												}
											: undefined,
										color: DEFAULT_POOL_COLOR_PALETTE.group,
										states: activeFolder === number ? ["selected"] : [],
									}}
									data-folder={number}
									onClick={() => {
										setFolder(number);
										setInspectingFolder(number);
										onInspectFolder?.(number);
									}}
								/>
							);
						})}
					</ButtonGrid>
				</WindowScrollArea>

				<WindowScrollArea className="media-library-pool">
					<div className="media-library-pool-heading">
						<span>Folder {String(activeFolder).padStart(3, "0")}</span>
						<small>{visible.length} sources</small>
					</div>
					<PoolGrid
						className="media-file-pool-grid media-library-file-pool-grid"
						slotCount={254}
						minimumCardWidth={112}
						slots={visible.map((item) => ({
							id: item.id,
							position: item.file - 1,
							card: {
								number: item.file,
								primary: item.name,
								secondary: item.detail,
								image: item.image,
								color: sourceColor,
								states: item.id === selectedId ? ["selected"] : [],
							},
						}))}
						emptySlot={(index) => ({
							id: `empty-${activeFolder}-${index + 1}`,
							position: index,
							card: { number: index + 1, primary: "", states: ["empty"] },
						})}
						renderSlot={(slot) => {
							const item = visible.find(
								(candidate) => candidate.file - 1 === slot.position,
							);
							return (
								<PoolCard
									model={slot.card}
									onClick={() => {
										setInspectingFolder(null);
										onInspectFolder?.(null);
										onSelectSlot?.({
											folder: activeFolder,
											file: slot.position + 1,
											...(item ? { itemId: item.id } : {}),
										});
										if (item) onSelect?.(item.id);
									}}
								/>
							);
						}}
					/>
					{visible.length === 0 && (
						<div className="media-library-empty-folder" role="status">
							<strong>This folder is empty</strong>
							<span>No source is assigned in this folder.</span>
						</div>
					)}
				</WindowScrollArea>

				<aside className="media-library-inspector">
					{activeInspector !== null
						? (renderFolderDetail?.(activeInspector) ?? emptyDetail)
						: showDetail || visibleSelected
							? detail
							: emptyDetail}
				</aside>
			</div>
		</WindowFrame>
	);
}

function range(first: number, last: number) {
	return Array.from({ length: last - first + 1 }, (_, index) => first + index);
}
