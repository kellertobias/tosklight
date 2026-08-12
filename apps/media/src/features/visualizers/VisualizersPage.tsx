import { Button } from "@tosklight/ui/controls";
import { useEffect, useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { addressLabel } from "../../entities/catalog";
import { MediaPreview } from "../../operator/MediaServerSurface";
import type {
	UpdateVisualizer,
	VisualizerView,
} from "../../shared/api/generated/media-wire";
import { useVisualizers } from "../../shared/api/queries";
import {
	GeneratedLibraryBrowserView,
	type LibrarySourceType,
} from "../media-library/GeneratedLibraryBrowserView";
import { useVisualizerEditing } from "./editing";
import { VisualizerEditor } from "./VisualizerEditor";

export function VisualizersPage({
	onModeChange,
}: {
	onModeChange?: (mode: LibrarySourceType) => void;
}) {
	const visualizers = useVisualizers();
	const editing = useVisualizerEditing(visualizers.reload);
	const [selectedKey, setSelectedKey] = useState<string | null>(null);

	useEffect(() => {
		if (selectedKey === null && visualizers.data?.[0])
			setSelectedKey(key(visualizers.data[0]));
	}, [selectedKey, visualizers.data]);

	return (
		<section className="media-page media-library-page">
			{editing.failure && (
				<p className="media-state is-error" role="alert">
					{editing.failure.message}{" "}
					<Button size="compact" onClick={editing.dismiss}>
						Dismiss
					</Button>
				</p>
			)}
			<ResourceState resource={visualizers} subject="the visualizers">
				{(data) => {
					const selected = data.find(
						(visualizer) => key(visualizer) === selectedKey,
					);
					return (
						<GeneratedLibraryBrowserView
							type="visualizers"
							items={data.map((visualizer) => ({
								id: key(visualizer),
								folder: visualizer.address.folder,
								file: visualizer.address.file,
								name: visualizer.name,
								detail: visualizer.kind,
							}))}
							selectedId={selectedKey ?? ""}
							onSelect={(next) => {
								editing.cancel();
								setSelectedKey(next);
							}}
							onTypeChange={onModeChange}
							detail={
								selected ? (
									<VisualizerDetail
										visualizer={selected}
										editing={editing.editing === key(selected)}
										busy={editing.busy}
										onEdit={() => editing.begin(key(selected))}
										onCancel={editing.cancel}
										onSave={(edit) => void editing.save(selected, edit)}
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
	editing,
	busy,
	onEdit,
	onCancel,
	onSave,
}: {
	visualizer: VisualizerView;
	editing: boolean;
	busy: boolean;
	onEdit: () => void;
	onCancel: () => void;
	onSave: (edit: UpdateVisualizer) => void;
}) {
	return (
		<div className="media-library-editor media-generated-library-detail">
			<MediaPreview
				title={visualizer.name}
				variant={
					visualizer.kind.toLowerCase().includes("particle")
						? "particles"
						: "aurora"
				}
			/>
			<header className="media-detail-heading">
				<div>
					<h2>{visualizer.name}</h2>
					<p>
						{visualizer.kind} ·{" "}
						{addressLabel(visualizer.address.folder, visualizer.address.file)}
					</p>
				</div>
				{!editing && <Button onClick={onEdit}>Tune visualizer</Button>}
			</header>
			{editing ? (
				<VisualizerEditor
					visualizer={visualizer}
					busy={busy}
					onSave={onSave}
					onCancel={onCancel}
				/>
			) : (
				<p className="media-visualizer-uses">
					Controls: {visualizer.uses.join(", ")}
				</p>
			)}
		</div>
	);
}
