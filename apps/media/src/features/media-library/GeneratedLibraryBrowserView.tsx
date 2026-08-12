import { MultiValueToggle } from "@tosklight/ui/controls";
import {
	DEFAULT_POOL_COLOR_PALETTE,
	PoolCard,
	PoolGrid,
} from "@tosklight/ui/pools";
import { WindowFrame, WindowScrollArea } from "@tosklight/ui/window-kit";
import { type ReactNode, useEffect, useMemo, useState } from "react";

export type LibrarySourceType = "media" | "visualizers" | "text";

export interface GeneratedLibraryItem {
	id: string;
	folder: number;
	file: number;
	name: string;
	detail: string;
	preview?: ReactNode;
}

export function LibrarySourceToggle({
	value,
	onChange,
}: {
	value: LibrarySourceType;
	onChange?: (value: LibrarySourceType) => void;
}) {
	return (
		<MultiValueToggle
			ariaLabel="Library source type"
			value={value}
			options={[
				{ value: "media", label: "Media" },
				{ value: "visualizers", label: "Visualizers" },
				{ value: "text", label: "Text" },
			]}
			onChange={(next) => onChange?.(next)}
		/>
	);
}

export function GeneratedLibraryBrowserView({
	type,
	items,
	selectedId,
	onSelect,
	onTypeChange,
	detail,
	headerAction,
	emptyDetail,
	showDetail = false,
}: {
	type: Exclude<LibrarySourceType, "media">;
	items: GeneratedLibraryItem[];
	selectedId: string;
	onSelect?: (id: string) => void;
	onTypeChange?: (value: LibrarySourceType) => void;
	detail: ReactNode;
	headerAction?: ReactNode;
	emptyDetail: ReactNode;
	showDetail?: boolean;
}) {
	const folders = type === "text" ? range(200, 249) : range(250, 255);
	const selected = items.find((item) => item.id === selectedId);
	const [folder, setFolder] = useState(selected?.folder ?? folders[0]);

	useEffect(() => {
		if (selected) setFolder(selected.folder);
	}, [selected]);

	const visible = useMemo(
		() => items.filter((item) => item.folder === folder),
		[folder, items],
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
			toolbar={
				<div className="media-library-title-tools">
					<LibrarySourceToggle value={type} onChange={onTypeChange} />
					{headerAction}
				</div>
			}
			className="media-library-window"
		>
			<div className="media-catalog-browser">
				<WindowScrollArea className="media-library-folders">
					<div className="media-library-pool-heading">
						<span>Folders</span>
						<small>{type === "text" ? "200–249" : "250–255"}</small>
					</div>
					<div className="media-library-folder-pool">
						{folders.map((number) => {
							const count = items.filter(
								(item) => item.folder === number,
							).length;
							return (
								<PoolCard
									key={number}
									model={{
										number: String(number).padStart(3, "0"),
										primary: type === "text" ? "Text" : "Visualizers",
										secondary: `${count}/254`,
										color: DEFAULT_POOL_COLOR_PALETTE.group,
										states: folder === number ? ["selected"] : [],
									}}
									data-folder={number}
									onClick={() => {
										setFolder(number);
										onSelect?.(
											items.find((item) => item.folder === number)?.id ?? "",
										);
									}}
								/>
							);
						})}
					</div>
				</WindowScrollArea>

				<WindowScrollArea className="media-library-pool">
					<div className="media-library-pool-heading">
						<span>Folder {String(folder).padStart(3, "0")}</span>
						<small>{visible.length} sources</small>
					</div>
					<PoolGrid
						slotCount={254}
						columns={5}
						minimumCardWidth={92}
						slots={visible.map((item) => ({
							id: item.id,
							position: item.file - 1,
							card: {
								number: item.file,
								primary: item.name,
								secondary: item.detail,
								color: sourceColor,
								states: item.id === selectedId ? ["selected"] : [],
							},
						}))}
						emptySlot={(index) => ({
							id: `empty-${folder}-${index + 1}`,
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
									onClick={item ? () => onSelect?.(item.id) : undefined}
								/>
							);
						}}
					/>
				</WindowScrollArea>

				<aside className="media-library-inspector">
					{showDetail || visibleSelected ? detail : emptyDetail}
				</aside>
			</div>
		</WindowFrame>
	);
}

function range(first: number, last: number) {
	return Array.from({ length: last - first + 1 }, (_, index) => first + index);
}
