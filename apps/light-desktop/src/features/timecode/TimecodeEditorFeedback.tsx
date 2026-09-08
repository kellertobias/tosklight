import { Button } from "@tosklight/ui";

export function TimecodeEditorFeedback({
	savingError,
	waveformError,
	busy,
	onRetrySave,
	onRetryWaveform,
}: {
	savingError: string | null;
	waveformError: string | null;
	busy: boolean;
	onRetrySave(): void;
	onRetryWaveform(): void;
}) {
	return (
		<>
			{waveformError && (
				<div className="timecode-error timecode-save-error" role="alert">
					<span>Waveform unavailable: {waveformError}</span>
					<Button onClick={onRetryWaveform}>Retry waveform</Button>
				</div>
			)}
			{savingError && (
				<div className="timecode-error timecode-save-error" role="alert">
					<span>{savingError}</span>
					<Button disabled={busy} onClick={onRetrySave}>
						Retry autosave
					</Button>
				</div>
			)}
		</>
	);
}
