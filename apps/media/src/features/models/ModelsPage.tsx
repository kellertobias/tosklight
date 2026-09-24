import { Button, FileDropField } from "@tosklight/ui/controls";
import { TextField } from "@tosklight/ui/forms";
import {
	DEFAULT_POOL_COLOR_PALETTE,
	PoolCard,
	PoolGrid,
} from "@tosklight/ui/pools";
import { WindowFrame, WindowScrollArea } from "@tosklight/ui/window-kit";
import { useEffect, useMemo, useRef, useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { MediaErrorToast } from "../../app/ToastContext";
import { usePreviewOf } from "../../operator/PlaybackTakeoverContext";
import { ApiFailure, api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type {
	BuiltinModelId,
	ModelSlotView,
} from "../../shared/api/generated/media-wire";
import { useModels } from "../../shared/api/queries";
import {
	type LibrarySourceType,
	librarySourceGroups,
} from "../media-library/GeneratedLibraryBrowserView";

const MODEL_SLOT_COUNT = 255;

/** The models every Media Server ships, in their default slot order. Plane is the default. */
export const BUILTIN_MODELS: ReadonlyArray<{
	id: BuiltinModelId;
	label: string;
}> = [
	{ id: "plane", label: "Plane" },
	{ id: "cube", label: "Cube" },
	{ id: "sphere", label: "Sphere" },
	{ id: "cylinder", label: "Cylinder" },
	{ id: "pyramid", label: "Pyramid" },
];

/**
 * The picture of a slot's model, drawn by the server from the mesh the outputs draw, or null for
 * a model that cannot be loaded. The URL changes whenever the slot's model does.
 */
export function modelPreviewUrl(model: ModelSlotView): string | null {
	if (model.status !== "ready") return null;
	const version = model.builtin ?? `${model.vertices}-${model.triangles}-${model.name}`;
	return api.modelPreviewUrl(model.slot, version);
}

/** The preview URLs that failed to load, so their cards fall back to the plain colour. */
function useFailedPreviews(urls: readonly string[]): ReadonlySet<string> {
	const [failed, setFailed] = useState<ReadonlySet<string>>(new Set());
	const key = urls.join("\n");
	// biome-ignore lint/correctness/useExhaustiveDependencies: `key` stands for the URL list.
	useEffect(() => {
		let live = true;
		for (const url of urls) {
			const probe = new Image();
			probe.onerror = () => {
				if (live) setFailed((current) => new Set(current).add(url));
			};
			probe.src = url;
		}
		return () => {
			live = false;
		};
	}, [key]);
	return failed;
}

/** Where the selected slot's upload has got to. Nothing about an upload is silent. */
export type ModelUpload =
	| { state: "idle" }
	| { state: "uploading"; fileName: string; fraction: number }
	| { state: "importing"; fileName: string }
	| { state: "done"; fileName: string }
	| { state: "failed"; fileName?: string; message: string };

export function ModelsPage({
	onModeChange,
}: {
	onModeChange?: (mode: LibrarySourceType) => void;
}) {
	const models = useModels();
	const editing = useEditing(models.reload);
	const [selectedSlot, setSelectedSlot] = useState(1);
	// With preview on, the last previewed content is mapped onto the selected model.
	usePreviewOf({ model: selectedSlot });
	const [upload, setUpload] = useState<ModelUpload>({ state: "idle" });

	const select = (slot: number) => {
		if (upload.state === "uploading" || upload.state === "importing") return;
		setSelectedSlot(slot);
		setUpload({ state: "idle" });
	};

	const uploadModel = async (file: File) => {
		const slot = selectedSlot;
		setUpload({ state: "uploading", fileName: file.name, fraction: 0 });
		try {
			await api.uploadModel(slot, requestId(), file, (fraction) =>
				setUpload(
					fraction >= 1
						? { state: "importing", fileName: file.name }
						: { state: "uploading", fileName: file.name, fraction },
				),
			);
			setUpload({ state: "done", fileName: file.name });
			models.reload();
		} catch (error) {
			setUpload({
				state: "failed",
				fileName: file.name,
				message:
					error instanceof ApiFailure || error instanceof Error
						? error.message
						: "the model could not be uploaded",
			});
		}
	};

	return (
		<section className="media-page media-library-page">
			{editing.failure && (
				<MediaErrorToast
					message={editing.failure.message}
					onDismiss={editing.dismiss}
				/>
			)}
			<ResourceState resource={models} subject="the 3D model library">
				{(data) => (
					<ModelsLibraryView
						models={data}
						selectedSlot={selectedSlot}
						busy={editing.busy}
						upload={upload}
						onSelect={select}
						onModeChange={onModeChange}
						onUpload={(file) => void uploadModel(file)}
						onRejected={(message) => setUpload({ state: "failed", message })}
						onRename={(name) =>
							editing.save(() =>
								api.updateModel(selectedSlot, { requestId: requestId(), name }),
							)
						}
						onClear={() =>
							editing.save(() =>
								api.updateModel(selectedSlot, {
									requestId: requestId(),
									clear: true,
								}),
							)
						}
						onBuiltin={(builtin) =>
							editing.save(() =>
								api.updateModel(selectedSlot, {
									requestId: requestId(),
									builtin,
								}),
							)
						}
					/>
				)}
			</ResourceState>
		</section>
	);
}

export function ModelsLibraryView({
	models,
	selectedSlot,
	busy,
	upload,
	onSelect,
	onModeChange,
	onUpload,
	onRejected,
	onRename,
	onClear,
	onBuiltin,
}: {
	models: ModelSlotView[];
	selectedSlot: number;
	busy: boolean;
	upload: ModelUpload;
	onSelect(slot: number): void;
	onModeChange?: (mode: LibrarySourceType) => void;
	onUpload(file: File): void;
	onRejected(message: string): void;
	onRename(name: string): void;
	onClear(): void;
	onBuiltin(builtin: BuiltinModelId): void;
}) {
	const bySlot = useMemo(
		() => new Map(models.map((model) => [model.slot, model])),
		[models],
	);
	const selected = bySlot.get(selectedSlot);
	const failedPreviews = useFailedPreviews(
		models.flatMap((model) => modelPreviewUrl(model) ?? []),
	);
	const pictureOf = (model: ModelSlotView) => {
		const url = modelPreviewUrl(model);
		return url && !failedPreviews.has(url) ? url : null;
	};
	return (
		<WindowFrame
			title="Library"
			info={{
				primary: "Models",
				secondary: "Slots 1–255 · selected by the layer 3D model channel",
			}}
			groups={librarySourceGroups({ value: "models", onChange: onModeChange })}
			className="media-library-window"
		>
			<div className="media-effects-library-layout">
				<WindowScrollArea className="media-library-pool media-effects-library-pool">
					<div className="media-library-pool-heading">
						<span>Model slots</span>
						<small>{models.length}/255 assigned</small>
					</div>
					<PoolGrid
						className="media-file-pool-grid media-models-pool-grid"
						slotCount={MODEL_SLOT_COUNT}
						minimumCardWidth={112}
						slots={models.map((model) => {
							const picture = pictureOf(model);
							return {
								id: `model-${model.slot}`,
								position: model.slot - 1,
								card: {
									...(picture
										? { image: { src: picture, alt: `${model.name} model` } }
										: {}),
									number: model.slot,
									primary: model.name,
									secondary:
										model.status !== "ready"
											? "Cannot load"
											: model.builtin
												? "Built-in"
												: `${model.triangles} triangles`,
									color: DEFAULT_POOL_COLOR_PALETTE.dynamic,
									states: model.slot === selectedSlot ? ["selected"] : [],
								},
							};
						})}
						emptySlot={(index) => ({
							id: `empty-model-${index + 1}`,
							position: index,
							card: {
								number: index + 1,
								primary: "Empty",
								secondary: "Available model slot",
								states: [
									"empty",
									...(index + 1 === selectedSlot
										? (["selected"] as const)
										: []),
								],
							},
						})}
						renderSlot={(slot) => (
							<PoolCard
								model={slot.card}
								onClick={() => onSelect(slot.position + 1)}
							/>
						)}
					/>
				</WindowScrollArea>
				<WindowScrollArea className="media-library-inspector media-effects-library-inspector">
					<ModelSlotEditor
						key={`${selectedSlot}:${selected?.name ?? "empty"}:${selected?.builtin ?? ""}`}
						slot={selectedSlot}
						model={selected}
						picture={selected ? pictureOf(selected) : null}
						busy={busy}
						upload={upload}
						onUpload={onUpload}
						onRejected={onRejected}
						onRename={onRename}
						onClear={onClear}
						onBuiltin={onBuiltin}
					/>
				</WindowScrollArea>
			</div>
		</WindowFrame>
	);
}

function ModelSlotEditor({
	slot,
	model,
	picture,
	busy,
	upload,
	onUpload,
	onRejected,
	onRename,
	onClear,
	onBuiltin,
}: {
	slot: number;
	model?: ModelSlotView;
	picture: string | null;
	busy: boolean;
	upload: ModelUpload;
	onUpload(file: File): void;
	onRejected(message: string): void;
	onRename(name: string): void;
	onClear(): void;
	onBuiltin(builtin: BuiltinModelId): void;
}) {
	const [name, setName] = useState(model?.name ?? "");
	const picker = useRef<HTMLInputElement>(null);
	const transferring =
		upload.state === "uploading" || upload.state === "importing";
	const disabled = busy || transferring;

	return (
		<div className="media-library-editor media-model-slot-editor">
			<p className="media-library-eyebrow">Model slot</p>
			<h2>Slot {String(slot).padStart(3, "0")}</h2>
			{model ? (
				<p>
					{model.builtin ? "Built-in · " : ""}
					{model.vertices} vertices · {model.triangles} triangles
				</p>
			) : (
				<p>
					Choose a built-in model or upload one to assign this empty slot.
					Layers selecting an empty slot are mapped onto the Plane.
				</p>
			)}
			{model?.status === "unloadable" && (
				<p className="media-model-status-error" role="alert">
					This model cannot be loaded, so layers selecting it are mapped onto
					the Plane: {model.detail}
				</p>
			)}
			{model && (
				<figure className="media-model-preview">
					{picture ? (
						<img src={picture} alt={`Picture of the ${model.name} model`} />
					) : (
						<figcaption role="status">
							{model.status === "ready"
								? "No picture of this model"
								: "No picture: the model cannot be loaded"}
						</figcaption>
					)}
				</figure>
			)}
			{model?.builtin ? (
				<>
					<p className="media-model-fixed">
						A built-in preset: its shape is fixed and needs no file or settings.
						Clear the slot to use it for another model.
					</p>
					<div className="media-operator-toolbar">
						<Button disabled={disabled} onClick={onClear}>
							Clear slot
						</Button>
					</div>
				</>
			) : (
				<ImportedModelFields
					model={model}
					name={name}
					setName={setName}
					disabled={disabled}
					picker={picker}
					upload={upload}
					transferring={transferring}
					onUpload={onUpload}
					onRejected={onRejected}
					onRename={onRename}
					onClear={onClear}
					onBuiltin={onBuiltin}
				/>
			)}
			<p className="media-field-help">
				The model is centred and scaled to fit the output height at scale 1. The
				layer&apos;s look is wrapped onto it through its texture coordinates.
				Select it with the layer&apos;s 3D model channel; 0 draws the layer
				flat. No imported file is needed for the built-in models.
			</p>
		</div>
	);
}

/// An empty slot's or an imported model's controls: place a preset, name it, replace or clear it.
function ImportedModelFields({
	model,
	name,
	setName,
	disabled,
	picker,
	upload,
	transferring,
	onUpload,
	onRejected,
	onRename,
	onClear,
	onBuiltin,
}: {
	model?: ModelSlotView;
	name: string;
	setName(name: string): void;
	disabled: boolean;
	picker: React.RefObject<HTMLInputElement | null>;
	upload: ModelUpload;
	transferring: boolean;
	onUpload(file: File): void;
	onRejected(message: string): void;
	onRename(name: string): void;
	onClear(): void;
	onBuiltin(builtin: BuiltinModelId): void;
}) {
	return (
		<>
			<fieldset className="media-model-builtins">
				<legend>Built-in model</legend>
				<div className="media-operator-toolbar">
					{BUILTIN_MODELS.map((builtin) => (
						<Button
							key={builtin.id}
							aria-pressed={false}
							active={false}
							disabled={disabled}
							onClick={() => onBuiltin(builtin.id)}
						>
							{builtin.label}
						</Button>
					))}
				</div>
			</fieldset>
			{model && (
				<>
					<TextField
						label="Name"
						value={name}
						onChange={(event) => setName(event.target.value)}
					/>
					<div className="media-operator-toolbar">
						<Button
							variant="primary"
							disabled={disabled || !name.trim() || name.trim() === model.name}
							onClick={() => onRename(name.trim())}
						>
							Save name
						</Button>
						<Button disabled={disabled} onClick={onClear}>
							Clear slot
						</Button>
					</div>
				</>
			)}
			<input
				ref={picker}
				hidden
				type="file"
				accept=".glb,model/gltf-binary"
				aria-label="Model file"
				onChange={(event) => {
					const file = event.target.files?.[0];
					event.currentTarget.value = "";
					if (file) onUpload(file);
				}}
			/>
			<FileDropField
				label={model ? "Replace model" : "Model file"}
				description="glTF 2.0 Binary (.glb) with texture coordinates"
				constraints={{ extensions: [".glb"] }}
				disabled={disabled}
				status={uploadStatus(upload)}
				statusMessage={uploadMessage(upload)}
				onFiles={(files) => files[0] && onUpload(files[0])}
				onRejected={onRejected}
				onOpenPicker={() => picker.current?.click()}
			/>
			{transferring && (
				<div className="media-model-upload-progress" aria-live="polite">
					<progress
						aria-label="Model upload progress"
						max={1}
						value={upload.state === "uploading" ? upload.fraction : undefined}
					/>
					<span>{uploadMessage(upload)}</span>
				</div>
			)}
			{upload.state === "failed" && (
				<p className="media-model-status-error" role="alert">
					{upload.fileName ? `${upload.fileName}: ` : ""}
					{upload.message}
				</p>
			)}
		</>
	);
}

function uploadStatus(upload: ModelUpload) {
	switch (upload.state) {
		case "uploading":
		case "importing":
			return "loading" as const;
		case "done":
			return "success" as const;
		case "failed":
			return "error" as const;
		default:
			return "idle" as const;
	}
}

function uploadMessage(upload: ModelUpload): string | undefined {
	switch (upload.state) {
		case "uploading":
			return `Uploading ${upload.fileName} — ${Math.round(upload.fraction * 100)}%`;
		case "importing":
			return `Importing ${upload.fileName}…`;
		case "done":
			return `${upload.fileName} assigned`;
		case "failed":
			return "Upload failed";
		default:
			return undefined;
	}
}
