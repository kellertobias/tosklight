import { SelectField } from "@tosklight/ui/controls";
import { TextField } from "@tosklight/ui/forms";
import { useEffect, useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { MediaErrorToast } from "../../app/ToastContext";
import { addressLabel } from "../../entities/catalog";
import { MediaPreview } from "../../operator/MediaServerSurface";
import { api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type {
	UpdateVisualizer,
	VisualizerView,
} from "../../shared/api/generated/media-wire";
import {
	useFolderPresentations,
	useVisualizers,
} from "../../shared/api/queries";
import { useMainOutputAspectRatio } from "../../shared/output/useMainOutputAspectRatio";
import { FolderPresentationEditor } from "../media-library/FolderPresentationEditor";
import {
	GeneratedLibraryBrowserView,
	type LibrarySourceType,
} from "../media-library/GeneratedLibraryBrowserView";
import { useVisualizerEditing } from "./editing";
import { BUILTIN_VISUALIZER_KINDS, visualizerPreviewUrl } from "./preview";
import { VisualizerEditor } from "./VisualizerEditor";

export function VisualizersPage({
	onModeChange,
}: {
	onModeChange?: (mode: LibrarySourceType) => void;
}) {
	const visualizers = useVisualizers();
	const folderPresentations = useFolderPresentations();
	const editing = useVisualizerEditing(visualizers.reload);
	const folderEditing = useEditing(folderPresentations.reload);
	const [selectedKey, setSelectedKey] = useState<string | null>(null);
	const [selectedSlot, setSelectedSlot] = useState<{
		folder: number;
		file: number;
	} | null>(null);
	const [inspectedFolder, setInspectedFolder] = useState<number | null>(null);
	const aspectRatio = useMainOutputAspectRatio();

	useEffect(() => {
		if (
			selectedKey === null &&
			selectedSlot === null &&
			visualizers.data?.[0]
		) {
			const first = visualizers.data[0];
			setSelectedKey(key(first));
			setSelectedSlot(first.address);
		}
	}, [selectedKey, selectedSlot, visualizers.data]);

	useEffect(() => {
		if (selectedKey) editing.begin(selectedKey);
	}, [selectedKey, editing.begin]);

	return (
		<section className="media-page media-library-page">
			{(editing.failure ?? folderEditing.failure) && (
				<MediaErrorToast
					message={
						(editing.failure ?? folderEditing.failure)?.message ??
						"Folder operation failed"
					}
					onDismiss={() => {
						editing.dismiss();
						folderEditing.dismiss();
					}}
				/>
			)}
			<ResourceState resource={visualizers} subject="the visualizers">
				{(data) => {
					const selected = data.find(
						(visualizer) => key(visualizer) === selectedKey,
					);
					const kinds = BUILTIN_VISUALIZER_KINDS.map((kind) => ({
						value: String(kind.typeId),
						label: kind.label,
					}));
					return (
						<GeneratedLibraryBrowserView
							type="visualizers"
							inspectedFolder={inspectedFolder}
							onInspectFolder={setInspectedFolder}
							folderPresentations={folderPresentations.data?.folders ?? []}
							renderFolderDetail={(folder) => {
								const presentation = folderPresentations.data?.folders.find(
									(candidate) => candidate.folder === folder,
								) ?? { folder, name: null, icon: null, pictureUrl: null };
								return (
									<FolderPresentationEditor
										presentation={presentation}
										busy={folderEditing.busy}
										onName={(name) =>
											folderEditing.save(() =>
												api.updateFolderPresentation(folder, {
													requestId: requestId(),
													name,
												}),
											)
										}
										onIcon={(icon) =>
											folderEditing.save(() =>
												api.updateFolderPresentation(folder, {
													requestId: requestId(),
													icon,
												}),
											)
										}
										onPicture={(picture) =>
											folderEditing.save(() =>
												api.uploadFolderPicture(folder, requestId(), picture),
											)
										}
										onRemovePicture={() =>
											folderEditing.save(() =>
												api.removeFolderPicture(folder, requestId()),
											)
										}
									/>
								);
							}}
							items={data.map((visualizer) => ({
								id: key(visualizer),
								folder: visualizer.address.folder,
								file: visualizer.address.file,
								name: visualizer.name,
								detail: visualizer.kind,
								image: {
									src: visualizerPreviewUrl(visualizer),
									alt: `${visualizer.name} preview`,
								},
							}))}
							selectedId={selectedKey ?? ""}
							onSelect={(next) => {
								setInspectedFolder(null);
								editing.cancel();
								setSelectedKey(next);
								editing.begin(next);
							}}
							onSelectSlot={(slot) => {
								setInspectedFolder(null);
								setSelectedSlot({ folder: slot.folder, file: slot.file });
								if (!slot.itemId) {
									setSelectedKey(null);
									editing.cancel();
								}
							}}
							onTypeChange={onModeChange}
							headerActions={[
								{
									id: "new-visualizer",
									label: "New visualizer",
									onPress: () => {
										setSelectedSlot(firstFreeSlot(data));
										setSelectedKey(null);
										editing.cancel();
									},
								},
							]}
							showDetail={selectedSlot !== null}
							detail={
								selectedSlot && !selected ? (
									<EmptyVisualizerDetail
										address={selectedSlot}
										kinds={kinds}
										busy={editing.busy}
										onChoose={(typeId, name) => {
											void editing
												.create({
													requestId: requestId(),
													folder: selectedSlot.folder,
													file: selectedSlot.file,
													typeId,
													name,
												})
												.then((created) => {
													if (created) {
														setSelectedKey(key(created));
														setSelectedSlot(created.address);
													}
												});
										}}
									/>
								) : selected ? (
									<VisualizerDetail
										visualizer={selected}
										aspectRatio={aspectRatio}
										kinds={kinds}
										busy={editing.busy}
										onChange={(edit) => editing.saveLive(selected, edit)}
									/>
								) : (
									<p>No visualizer is selected.</p>
								)
							}
							emptyDetail={
								<div className="media-library-reserved-copy">
									<h2>Empty visualizer folder</h2>
									<p>No generated source is assigned in this folder.</p>
								</div>
							}
						/>
					);
				}}
			</ResourceState>
		</section>
	);
}

export function key(visualizer: VisualizerView): string {
	return `${visualizer.address.folder}/${visualizer.address.file}`;
}

export function VisualizerDetail({
	visualizer,
	aspectRatio,
	kinds,
	busy,
	onChange,
}: {
	visualizer: VisualizerView;
	aspectRatio: number;
	kinds: Array<{ value: string; label: string }>;
	busy: boolean;
	onChange: (edit: UpdateVisualizer) => void;
}) {
	return (
		<div className="media-library-editor media-generated-library-detail">
			<div className="media-generated-sticky-preview">
				<MediaPreview
					title={visualizer.name}
					meta={addressLabel(
						visualizer.address.folder,
						visualizer.address.file,
					)}
					aspectRatio={aspectRatio}
					variant={
						visualizer.kind.toLowerCase().includes("particle")
							? "particles"
							: "aurora"
					}
				>
					<img src={visualizerPreviewUrl(visualizer)} alt="" />
				</MediaPreview>
			</div>
			<p className="media-source-address">
				{addressLabel(visualizer.address.folder, visualizer.address.file)}
			</p>
			<div className="media-source-identity-grid">
				<SelectField
					label="Built-in visualizer"
					ariaLabel="Built-in visualizer"
					value={String(visualizer.typeId)}
					options={kinds}
					onChange={(value) =>
						onChange({
							requestId: requestId(),
							typeId: Number(value),
						})
					}
				/>
				<VisualizerName visualizer={visualizer} onChange={onChange} />
			</div>
			<hr className="media-source-editor-separator" />
			<VisualizerEditor
				key={`${key(visualizer)}:${visualizer.typeId}`}
				visualizer={visualizer}
				busy={busy}
				onChange={onChange}
			/>
		</div>
	);
}

function EmptyVisualizerDetail({
	address,
	kinds,
	busy,
	onChoose,
}: {
	address: { folder: number; file: number };
	kinds: Array<{ value: string; label: string }>;
	busy: boolean;
	onChoose: (typeId: number, name?: string) => void;
}) {
	const [name, setName] = useState("");
	return (
		<div className="media-library-editor media-visualizer-editor media-generated-library-detail">
			<div className="media-generated-sticky-preview">
				<MediaPreview
					title={name || "Empty visualizer"}
					meta={addressLabel(address.folder, address.file)}
					variant="aurora"
				/>
			</div>
			<p className="media-source-address">
				{addressLabel(address.folder, address.file)}
			</p>
			<div className="media-source-identity-grid">
				<SelectField
					label="Built-in visualizer"
					ariaLabel="Built-in visualizer"
					value=""
					options={[{ value: "", label: "Choose a visualizer" }, ...kinds]}
					onChange={(value) =>
						value && onChoose(Number(value), name.trim() || undefined)
					}
					disabled={busy}
				/>
				<TextField
					label="Name"
					value={name}
					onChange={(event) => setName(event.target.value)}
				/>
			</div>
			<hr className="media-source-editor-separator" />
			<p className="media-field-help">
				Choose the kind first, then tune its live controls.
			</p>
		</div>
	);
}

function VisualizerName({
	visualizer,
	onChange,
}: {
	visualizer: VisualizerView;
	onChange: (edit: UpdateVisualizer) => void;
}) {
	const [name, setName] = useState(visualizer.name);
	return (
		<TextField
			label="Name"
			value={name}
			onChange={(event) => {
				setName(event.target.value);
				onChange({
					requestId: requestId(),
					name: event.target.value,
				});
			}}
		/>
	);
}

export function firstFreeSlot(visualizers: VisualizerView[]): {
	folder: number;
	file: number;
} {
	const taken = new Set(visualizers.map(key));
	for (let folder = 250; folder <= 255; folder += 1)
		for (let file = 1; file <= 254; file += 1)
			if (!taken.has(`${folder}/${file}`)) return { folder, file };
	return { folder: 250, file: 1 };
}
