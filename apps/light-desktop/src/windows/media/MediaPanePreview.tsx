import { Button } from "@tosklight/ui";
import { useEffect, useRef, useState } from "react";
import type { MediaPreviewState } from "./mediaPaneModel";

export function MediaCompositePreview({
	preview,
	selected,
	onSelect,
}: {
	preview: MediaPreviewState;
	selected: boolean;
	onSelect(): void;
}) {
	const imageSrc = "imageSrc" in preview ? preview.imageSrc : undefined;
	const [imageFailed, setImageFailed] = useState(false);
	useEffect(() => setImageFailed(false), [imageSrc]);
	const outputAspectRatio = preview.outputSize
		? `${preview.outputSize.width} / ${preview.outputSize.height}`
		: "16 / 9";
	return (
		<Button
			type="button"
			fullWidth
			className={`media-composite-frame state-${preview.kind} ${selected ? "selected" : ""}`}
			data-preview-state={preview.kind}
			aria-label={`Master output ${selected ? "selected" : ""}`.trim()}
			aria-pressed={selected}
			onClick={onSelect}
		>
			<span
				className={`media-composite-picture ${imageSrc ? "has-image" : "is-empty"}`}
				data-testid="master-output-picture"
				style={{ aspectRatio: outputAspectRatio }}
			>
				{imageSrc && (
					<SafePreviewImage
						src={imageSrc}
						alt="Master output preview"
						onFailure={() => setImageFailed(true)}
					/>
				)}
					{preview.kind === "audio" && (
						<MusicNote className="media-composite-note" label="Audio output" />
					)}
					<span className="media-composite-safe-area" aria-hidden="true" />
			</span>
			<span className="media-composite-info">
				<strong>
						{preview.kind === "audio"
							? "Master audio output"
							: "Master output live preview"}
					</strong>
				{imageFailed ? (
					<span className="danger" role="alert">
						Live preview could not be loaded.
					</span>
				) : null}
				<PreviewStateMessage preview={preview} />
			</span>
		</Button>
	);
}

/** Stand-in artwork for an audio-only media source, which has no rendered frame. */
export function MusicNote({ className, label }: { className: string; label: string }) {
	return (
		<span className={className} role="img" aria-label={label} title={label}>
			<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
				<path
					d="M9 17.5V6.2l9-1.8v9.6"
					fill="none"
					stroke="currentColor"
					strokeWidth="1.6"
					strokeLinecap="round"
					strokeLinejoin="round"
				/>
				<circle cx="6.5" cy="17.5" r="2.5" fill="currentColor" />
				<circle cx="15.5" cy="14" r="2.5" fill="currentColor" />
			</svg>
		</span>
	);
}

export function SafePreviewImage({
	src,
	alt,
	onFailure,
}: {
	src: string;
	alt: string;
	onFailure(): void;
}) {
	const [loadedFrames, setLoadedFrames] = useState<string[]>([]);
	const currentSrc = loadedFrames.at(-1);
	const desiredSrc = useRef(src);
	desiredSrc.current = src;
	const [pendingSrc, setPendingSrc] = useState<string | null>(src);
	useEffect(() => {
		if (currentSrc !== src && pendingSrc === null) setPendingSrc(src);
	}, [currentSrc, pendingSrc, src]);
	const finishPending = (loaded: boolean) => {
		if (pendingSrc === null) return;
		const completed = pendingSrc;
		if (loaded)
			setLoadedFrames((frames) =>
				frames.at(-1) === completed ? frames : [...frames.slice(-1), completed],
			);
		else onFailure();
		setPendingSrc(desiredSrc.current === completed ? null : desiredSrc.current);
	};
	return (
		<>
			{loadedFrames.map((frame, index) => {
				const current = index === loadedFrames.length - 1;
				return (
					<img
						key={frame}
						className={current ? "is-current" : "is-previous"}
						aria-hidden={current ? undefined : true}
						alt={current ? alt : ""}
						src={frame}
						onError={onFailure}
					/>
				);
			})}
			{pendingSrc && (
				<img
					key={pendingSrc}
					hidden
					className="is-pending"
					aria-hidden="true"
					alt=""
					src={pendingSrc}
					onLoad={() => finishPending(true)}
					onError={() => finishPending(false)}
				/>
			)}
		</>
	);
}

export function PreviewStateMessage({ preview }: { preview: MediaPreviewState }) {
	switch (preview.kind) {
		case "ready":
			if (preview.imageSrc) return null;
			return (
				<span>
					No output · black
					{preview.capturedAt ? ` · ${preview.capturedAt}` : ""}
				</span>
			);
		case "stale":
			return (
				<span className="warning">
					Stale preview · {preview.capturedAt}
					{preview.detail ? ` · ${preview.detail}` : ""}
				</span>
			);
		case "offline":
			return (
				<span className="danger" title={preview.diagnostic ?? undefined}>
					Offline
					{preview.imageSrc ? " · showing last preview" : " · black output"} ·{" "}
					{preview.detail}
				</span>
			);
		case "failed_source":
			return (
				<span className="danger">
					Source {preview.source} failed · {preview.detail}
				</span>
			);
		case "audio":
			return <span>{preview.detail}</span>;
		case "missing_patch":
			return <span className="danger">Missing patch · {preview.detail}</span>;
		case "unsupported":
			return (
				<span className="warning">
					{preview.capability} unsupported · {preview.detail}
				</span>
			);
	}
}
