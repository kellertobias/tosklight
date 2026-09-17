import { NumberField } from "@tosklight/ui/forms";
import { useState } from "react";
import type { PlaybackView } from "../../shared/api/generated/media-wire";
import { SettingsSaveState } from "./SettingsSaveState";

/**
 * The rate every layer's In and Out point channels count in. The desk and the layer page show
 * the points as `mm:ss.ff` at this rate.
 */
export function PointFrameRateSettings({
	playback,
	busy,
	failed,
	onSave,
}: {
	playback: PlaybackView;
	busy: boolean;
	failed: boolean;
	onSave: (frameRate: number) => void;
}) {
	const [draft, setDraft] = useState(String(playback.frameRate));
	const valid = (text: string) => {
		const value = Number(text);
		return (
			text.trim() !== "" &&
			Number.isInteger(value) &&
			value >= 1 &&
			value <= playback.maximumFrameRate
		);
	};
	return (
		<article className="media-settings-section" aria-label="In and Out points">
			<div className="media-settings-section-heading">
				<h2>In and Out points</h2>
				<SettingsSaveState busy={busy} failed={failed} />
			</div>
			<p>
				A layer's In and Out point channels count frames. This rate turns them
				into a time, so the desk and the layer page show and take them as
				mm:ss.ff, and every clip starts and stops at the same time whatever its
				own frame rate. Applies immediately, also to layers that are playing.
			</p>
			<NumberField
				label="Frame rate (fps)"
				step={1}
				min={1}
				max={playback.maximumFrameRate}
				unit="fps"
				value={draft}
				error={
					draft.trim() === "" || valid(draft)
						? undefined
						: `Enter whole frames per second from 1 to ${playback.maximumFrameRate}.`
				}
				onChange={(event) => {
					const next = event.target.value;
					setDraft(next);
					if (valid(next) && Number(next) !== playback.frameRate)
						onSave(Number(next));
				}}
			/>
		</article>
	);
}
