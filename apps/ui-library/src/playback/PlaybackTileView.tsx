import type { CSSProperties, ReactNode } from "react";
import { Button } from "../controls";
import type { VerticalTouchFaderAction } from "../faders";
import { PlaybackArtwork } from "./PlaybackArtwork";
import {
	playbackCardColor,
	type PlaybackCardCallbacks,
	type PlaybackCardSlots,
	type PlaybackCardViewModel,
} from "./PlaybackCards";

export interface PlaybackTileViewProps {
	model: PlaybackCardViewModel;
	action?: VerticalTouchFaderAction;
	/** Optional context such as Solo membership; the underlying control action is unchanged. */
	actionLabel?: ReactNode;
	callbacks?: PlaybackCardCallbacks;
	slots?: PlaybackCardSlots;
}

/** A software Playback tile has one real control, including its identity and picture. */
export function PlaybackTileView({ model, action, actionLabel = action?.label, callbacks = {}, slots = {} }: PlaybackTileViewProps) {
	const kind = model.assigned ? (model.kind ?? "cue-list") : "empty";
	const { id, label: _label, className, style, ...buttonProps } = action ?? {};
	const classes = [
		"playback-card touch-playback-card playback-tile-card",
		`playback-kind-${kind}`,
		model.assigned ? "playback-colored" : "empty",
		model.selected && "selected",
		model.artwork?.transparent && "cue-preview-transparent",
		(model.artwork?.image || model.artwork?.icon) && "has-playback-artwork",
		model.className,
	].filter(Boolean).join(" ");
	return (
		<article
			className={classes}
			style={{ "--playback-color": playbackCardColor(model), ...model.style } as CSSProperties}
			data-ui-component="playback-tile"
			data-page={model.page}
			data-playback-slot={model.slot}
			data-playback-row={model.row}
			data-row-units={model.rowUnits}
			data-playback-kind={kind}
			data-button-count={action ? 1 : 0}
			data-has-fader={false}
			data-selected-playback={model.selected || undefined}
			data-selection-pending={model.selectionPending || undefined}
			data-image-source={model.artwork?.imageSource}
			data-preview-notice={model.artwork?.notice}
			onPointerDownCapture={callbacks.onPointerDownCapture}
			onClickCapture={callbacks.onClickCapture}
		>
			<Button
				{...buttonProps}
				data-playback-primary-tile="true"
				className={`playback-tile-action ${className ?? ""}`.trim()}
				style={style}
				disabled={Boolean(model.disabled || action?.disabled || !action)}
				aria-label={`Playback ${model.page}.${model.slot}: ${model.name}`}
				aria-description={[typeof actionLabel === "string" ? actionLabel : action?.label, model.description].filter(Boolean).join(" · ") || undefined}
				title={model.description}
				onClick={action?.onClick ?? callbacks.onActivate}
			>
				<PlaybackArtwork artwork={model.artwork} />
				<span className="playback-tile-heading">
					<strong className="playback-tile-title">{model.name}</strong>
					<span className="playback-tile-number">{model.page}.{model.slot}</span>
				</span>
				<span className="playback-tile-footer">
					{model.assigned && actionLabel && <span className="playback-tile-action-label">{actionLabel}</span>}
					{(model.summary || model.status) && <span className="playback-tile-summary">
						{model.summary && <>
							<span className="playback-tile-cue-label">{model.summary.label}</span>
							{model.summary.detail && <small>{model.summary.detail}</small>}
						</>}
						{model.status && <span className={`playback-tile-status playback-status-${model.status.kind}`}>{model.status.label}</span>}
						{model.summary?.warning && <em className="playback-tile-warning">{model.summary.warning}</em>}
					</span>}
				</span>
			</Button>
			{slots.overlays}
		</article>
	);
}
