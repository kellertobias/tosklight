// What the server is hearing.
//
// A meter, not an instrument: this is how an operator confirms the desk feed is arriving, that the
// gain is sane, and that the beat detector is following the music. It draws whatever the last frame
// carried and says plainly when frames have stopped, because a frozen meter that looks live is
// worse than one that admits it is not receiving.

import type {
	AudioView,
	AudioVoiceView,
} from "../../shared/api/generated/media-wire";

export interface AudioMetersProps {
	audio: AudioView;
	/** Whether frames are still arriving. */
	live: boolean;
}

export function AudioMeters({ audio, live }: AudioMetersProps) {
	return (
		<article className="media-audio-monitor" aria-label="Audio monitor">
			<header>
				<h2>{audio.capturing ? audio.device : "Not capturing"}</h2>
				<span
					className={`media-badge ${audio.capturing && live ? "is-good" : "is-bad"}`}
					role="status"
				>
					{status(audio, live)}
				</span>
				{audio.clipping && (
					<span className="media-badge is-bad" role="alert">
						Input clipping — turn the source down
					</span>
				)}
			</header>

			{/* What the detector hears: a lamp that flashes on each hit over the instrument's level. */}
			<div className="media-audio-hits" aria-label="Detected instruments">
				<Hit label="Kick" tone="kick" voice={audio.kick} />
				<Hit label="Snare" tone="snare" voice={audio.snare} />
				<Hit label="Hi-hat" tone="hihat" voice={audio.hihat} />
				{/* The beat's bar is its phase: it sweeps once per beat. */}
				<Hit
					label="Beat"
					tone="beat"
					voice={{ level: audio.beatPhase, hit: audio.beat }}
				/>
			</div>

			{audio.detail && <p className="media-state is-notice">{audio.detail}</p>}

			<div className="media-audio-bands">
				<Bar label="Bass" value={audio.bands.bass} />
				<Bar label="Mid" value={audio.bands.mid} />
				<Bar label="Treble" value={audio.bands.treble} />
				<Bar label="Level" value={audio.energy} />
				{/* Peak is what tells an operator they are clipping, so it is named for that. */}
				<Bar label="Peak" value={audio.peak} warnAbove={0.98} />
			</div>

			<Waveform points={audio.waveform.points} />
			<Spectrum bands={audio.spectrum} />

			<dl className="media-facts">
				<dt>Tempo</dt>
				<dd>
					{audio.bpm > 0
						? `${audio.bpm.toFixed(1)} BPM · ${Math.round(clamp01(audio.tempoConfidence) * 100)}% steady`
						: "listening for a tempo"}
				</dd>
				<dt>Gain</dt>
				<dd>{audio.gain.toFixed(2)}×</dd>
			</dl>
		</article>
	);
}

function clamp01(value: number): number {
	return Math.min(Math.max(value, 0), 1);
}

function Hit({
	label,
	tone,
	voice,
}: {
	label: string;
	tone: "kick" | "snare" | "hihat" | "beat";
	voice: AudioVoiceView;
}) {
	const level = Math.round(clamp01(voice.level) * 100);
	const struck = voice.hit > 0.5;
	return (
		<div className={`media-audio-hit is-${tone}${struck ? " is-struck" : ""}`}>
			<span
				className="media-audio-hit-lamp"
				style={{ opacity: Math.max(0.12, clamp01(voice.hit)) }}
				aria-hidden="true"
			/>
			<span className="media-audio-hit-label">{label}</span>
			<div
				className="media-audio-bar-track"
				role="meter"
				aria-label={`${label} level`}
				aria-valuenow={level}
				aria-valuemin={0}
				aria-valuemax={100}
			>
				<span style={{ width: `${level}%` }} />
			</div>
			<span className="media-visually-hidden">
				{struck ? `${label} struck` : `${label} quiet`}
			</span>
		</div>
	);
}

function status(audio: AudioView, live: boolean): string {
	if (!audio.capturing) return "No input device";
	if (!live) return "Not receiving";
	return "Listening";
}

function Bar({
	label,
	value,
	warnAbove,
}: {
	label: string;
	value: number;
	warnAbove?: number;
}) {
	const percent = Math.round(Math.min(Math.max(value, 0), 1) * 100);
	return (
		<div className="media-audio-bar">
			<span>{label}</span>
			<div
				className={`media-audio-bar-track${warnAbove !== undefined && value >= warnAbove ? " is-hot" : ""}`}
				role="meter"
				aria-label={label}
				aria-valuenow={percent}
				aria-valuemin={0}
				aria-valuemax={100}
			>
				<span style={{ width: `${percent}%` }} />
			</div>
			<span className="media-audio-bar-value">{percent}%</span>
		</div>
	);
}

/// The window, drawn as one path. Downsampled by the server for display, never for measurement.
function Waveform({ points }: { points: number[] }) {
	if (points.length === 0) {
		return <p className="media-audio-empty">No window to draw.</p>;
	}
	const step = 100 / Math.max(1, points.length - 1);
	const path = points
		.map((point, index) => {
			const y = 50 - Math.max(-1, Math.min(1, point)) * 48;
			return `${index === 0 ? "M" : "L"}${(index * step).toFixed(3)},${y.toFixed(3)}`;
		})
		.join(" ");

	return (
		<svg
			className="media-audio-waveform"
			viewBox="0 0 100 100"
			preserveAspectRatio="none"
			role="img"
			aria-label="Waveform"
		>
			<path d={path} vectorEffect="non-scaling-stroke" />
		</svg>
	);
}

function Spectrum({ bands }: { bands: number[] }) {
	if (bands.length === 0) {
		return <p className="media-audio-empty">No spectrum to draw.</p>;
	}
	const width = 100 / bands.length;
	return (
		<svg
			className="media-audio-spectrum"
			viewBox="0 0 100 100"
			preserveAspectRatio="none"
			role="img"
			aria-label="Spectrum"
		>
			{bands.map((band, index) => {
				const height = Math.min(Math.max(band, 0), 1) * 100;
				return (
					<rect
						// The band's position *is* its identity: band three is always band three.
						key={index}
						x={index * width}
						y={100 - height}
						width={width * 0.8}
						height={height}
					/>
				);
			})}
		</svg>
	);
}
