import { Button, InputModal } from "@tosklight/ui";
import { useState } from "react";
import type { MediaPointTimeControl } from "./mediaPaneModel";
import {
	formatPointTime,
	parsePointFrames,
	parsePointTime,
} from "./mediaPointTime";

/**
 * An In or Out point entered by typing, never by a 0–65535 fader: `mm:ss.ff` at the server's
 * frame rate, or a frame count while that rate is unknown.
 */
export function MediaPointTimeField({
	control,
	disabled,
	onChange,
}: {
	control: MediaPointTimeControl;
	disabled: boolean;
	onChange(controlId: string, frames: number): void;
}) {
	const [editing, setEditing] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const timed = control.framesPerSecond != null;
	const title = timed
		? `${control.label} (mm:ss.ff${control.reference === "end" ? " before end" : ""})`
		: `${control.label} (frames${control.reference === "end" ? " before end" : ""})`;
	const close = () => {
		setError(null);
		setEditing(false);
	};
	return (
		<div
			className={`media-point-time-control ${disabled ? "disabled" : ""}`}
			data-reference={control.reference}
		>
			<span className="media-point-time-label">{control.label}</span>
			<Button
				className="media-point-time-value"
				disabled={disabled}
				aria-label={`${control.label}: ${control.display}. Enter ${timed ? "a time" : "a frame count"}`}
				onClick={() => setEditing(true)}
			>
				{control.display}
			</Button>
			{timed ? (
				<small>
					{control.reference === "start"
						? "From the clip's start"
						: "Before the clip's end; 00:00.00 plays to the end"}{" "}
					· {control.framesPerSecond} fps
				</small>
			) : null}
			{control.rateNotice ? (
				<div className="media-point-time-notice" role="status">
					<small>{control.rateNotice}</small>
					{control.onRetryFrameRate ? (
						<Button disabled={disabled} onClick={control.onRetryFrameRate}>
							Check frame rate again
						</Button>
					) : null}
				</div>
			) : null}
			{control.description && <small>{control.description}</small>}
			{editing && !disabled ? (
				<InputModal
					kind="text"
					label={title}
					value={
						control.framesPerSecond != null
							? formatPointTime(control.value, control.framesPerSecond)
							: String(control.value)
					}
					placeholder={timed ? "mm:ss.ff" : "frames"}
					error={error ?? undefined}
					onDraftChange={() => setError(null)}
					onCancel={close}
					onCommit={(text) => {
						const parsed =
							control.framesPerSecond != null
								? parsePointTime(text, control.framesPerSecond)
								: parsePointFrames(text);
						if (!parsed.ok) {
							setError(parsed.error);
							return;
						}
						close();
						if (parsed.frames !== control.value)
							onChange(control.id, parsed.frames);
					}}
				/>
			) : null}
		</div>
	);
}
