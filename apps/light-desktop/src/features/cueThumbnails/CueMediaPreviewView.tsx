import { Button } from "@tosklight/ui";
import type { CueMediaPreview } from "./useCueMediaPreviews";

/** Operator wording for a Media Server Cue preview that has no picture right now. */
export function cueMediaPreviewNotice(preview: CueMediaPreview): string | null {
	switch (preview.state) {
		case "ready":
			return null;
		case "empty":
			return "Empty media";
		case "loading":
			return "Loading media preview";
		case "offline":
			return "Media Server offline";
		case "missing":
			return "Media output missing";
	}
}

function scopeLabel(preview: CueMediaPreview) {
	return preview.entry.scope === "layer"
		? `Layer ${(preview.entry.layer ?? 0) + 1}`
		: "Program";
}

/**
 * A Media Server Cue preview in a Cue row.
 *
 * A picture is only ever the one fetched for this Cue's own server, output, layer, and state.
 * Every other state is an explicit, labelled placeholder: never a broken image, never a stale
 * picture of something else. An offline server can be asked again from the placeholder itself.
 */
export function CueMediaPreviewContent({
	cueNumber,
	preview,
	onOpen,
	onRetry,
}: {
	cueNumber: string;
	preview: CueMediaPreview;
	onOpen: () => void;
	onRetry?: () => void;
}) {
	const notice = cueMediaPreviewNotice(preview);
	const attributes = {
		"data-preview-kind": "media",
		"data-media-scope": preview.entry.scope,
		"data-media-state": preview.state,
		"data-media-server": preview.entry.serverFixtureId,
		"data-media-layer":
			preview.entry.layer == null ? undefined : String(preview.entry.layer),
		"data-media-preview-key": preview.entry.previewKey,
	};
	const className = `cue-media-preview scope-${preview.entry.scope} state-${preview.state}`;
	if (preview.state === "ready" || preview.state === "empty")
		return (
			<Button
				type="button"
				className={`cue-preview-image-button ${className}`}
				aria-label={`Open Cue ${cueNumber} ${scopeLabel(preview)} preview`}
				title={notice ?? undefined}
				{...attributes}
				onClick={(event) => {
					event.stopPropagation();
					onOpen();
				}}
			>
				<img src={preview.src} alt="" />
				{notice && <span className="cue-media-preview-notice">{notice}</span>}
			</Button>
		);
	if (preview.state === "offline" && onRetry)
		return (
			<Button
				type="button"
				className={`cue-media-preview-placeholder ${className}`}
				aria-label={`Cue ${cueNumber} ${notice}. Retry the ${scopeLabel(preview)} preview`}
				title={preview.error}
				{...attributes}
				onClick={(event) => {
					event.stopPropagation();
					onRetry();
				}}
			>
				<span className="cue-media-preview-notice">{notice}</span>
				<span className="cue-media-preview-action">Retry</span>
			</Button>
		);
	return (
		<span
			className={`cue-media-preview-placeholder ${className}`}
			role="status"
			aria-label={`Cue ${cueNumber} ${notice}`}
			title={"error" in preview ? preview.error : undefined}
			{...attributes}
		>
			<span className="cue-media-preview-notice">{notice}</span>
		</span>
	);
}
