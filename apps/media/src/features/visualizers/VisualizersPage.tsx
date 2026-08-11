import { Button } from "@tosklight/ui/controls";
import { useEffect, useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { addressLabel } from "../../entities/catalog";
import {
	MediaListDetail,
	MediaPreview,
} from "../../operator/MediaServerSurface";
import type {
	UpdateVisualizer,
	VisualizerView,
} from "../../shared/api/generated/media-wire";
import { useVisualizers } from "../../shared/api/queries";
import { useVisualizerEditing } from "./editing";
import { VisualizerEditor } from "./VisualizerEditor";

export function VisualizersPage() {
	const visualizers = useVisualizers();
	const editing = useVisualizerEditing(visualizers.reload);
	const [selectedKey, setSelectedKey] = useState("");

	useEffect(() => {
		if (!selectedKey && visualizers.data?.[0])
			setSelectedKey(key(visualizers.data[0]));
	}, [selectedKey, visualizers.data]);

	return (
		<section className="media-page">
			{editing.failure && (
				<p className="media-state is-error" role="alert">
					{editing.failure.message}{" "}
					<Button size="compact" onClick={editing.dismiss}>
						Dismiss
					</Button>
				</p>
			)}
			<ResourceState
				resource={visualizers}
				subject="the visualizers"
				isEmpty={(data) => data.length === 0}
				empty="No generated visualizers are assigned to an address."
			>
				{(data) => {
					const selected =
						data.find((visualizer) => key(visualizer) === selectedKey) ??
						data[0];
					return (
						<MediaListDetail
							label="Visualizers"
							items={data.map((visualizer) => ({
								id: key(visualizer),
								title: visualizer.name,
								detail: visualizer.kind,
								meta: addressLabel(
									visualizer.address.folder,
									visualizer.address.file,
								),
							}))}
							selectedId={key(selected)}
							onSelect={(next) => {
								editing.cancel();
								setSelectedKey(next);
							}}
							detail={
								<VisualizerDetail
									visualizer={selected}
									editing={editing.editing === key(selected)}
									busy={editing.busy}
									onEdit={() => editing.begin(key(selected))}
									onCancel={editing.cancel}
									onSave={(edit) => void editing.save(selected, edit)}
								/>
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

function VisualizerDetail({
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
		<>
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
		</>
	);
}
