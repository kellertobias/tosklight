import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import { Button, Input } from "../../components/common";
import {
	DEMO_PLAYBACK_STRIP_BUTTONS,
	DEMO_PLAYBACK_STRIP_SLOTS,
	DEMO_PLAYBACK_TOP_SLOTS,
} from "./demoPlaybackMapping";
import {
	useDemoPlaybackControls,
	type DemoPlaybackControlsValue,
} from "./useDemoPlaybackControls";

interface DemoPlaybackButtonProps {
	controls: DemoPlaybackControlsValue;
	slot: number;
	button?: number;
	label?: string;
}

function DemoPlaybackButton({
	controls,
	slot,
	button = 1,
	label = String(button),
}: DemoPlaybackButtonProps) {
	const [pressed, setPressed] = useState(false);
	const enabled = controls.playbackNumber(slot) !== null;
	const release = useCallback(() => {
		setPressed(false);
		controls.release(slot, button);
	}, [button, controls, slot]);
	const releaseRef = useRef(release);
	useEffect(() => {
		releaseRef.current = release;
	}, [release]);
	useEffect(() => {
		if (!enabled) releaseRef.current();
	}, [enabled]);
	useEffect(() => () => releaseRef.current(), []);
	return (
		<Button
			className={`product-demo-playback-button ${pressed ? "local-pressed" : ""}`}
			aria-label={`Playback ${slot} button ${button}`}
			disabled={!enabled}
			onPointerDown={(event) => {
				if (!enabled) return;
				event.currentTarget.setPointerCapture(event.pointerId);
				setPressed(true);
				controls.press(slot, button);
			}}
			onPointerUp={release}
			onPointerCancel={release}
			onLostPointerCapture={release}
		>
			{label}
		</Button>
	);
}

function DemoPlaybackStrip({
	controls,
	slot,
}: {
	controls: DemoPlaybackControlsValue;
	slot: number;
}) {
	const value = controls.faderLevel(slot);
	const enabled = value !== null;
	return (
		<article className="product-demo-playback-strip">
			<b>PB {slot}</b>
			<DemoPlaybackButton controls={controls} slot={slot} />
			<label
				className="product-demo-playback-fader"
				style={
					enabled
						? ({ "--demo-playback-level": value } as CSSProperties)
						: undefined
				}
			>
				<span>FADER</span>
				<strong>{enabled ? `${Math.round(value * 100)}%` : "—"}</strong>
				<Input
					aria-label={`Playback ${slot} fader`}
					disabled={!enabled}
					type="range"
					min="0"
					max="1"
					step=".001"
					value={value ?? 0}
					onInput={(event) =>
						controls.setMaster(slot, Number(event.currentTarget.value))
					}
				/>
			</label>
			<footer>
				{DEMO_PLAYBACK_STRIP_BUTTONS.slice(1).map((button) => (
					<DemoPlaybackButton
						controls={controls}
						slot={slot}
						button={button}
						key={button}
					/>
				))}
			</footer>
		</article>
	);
}

export function DemoPlaybackControls() {
	const controls = useDemoPlaybackControls();
	const pending = controls.status.kind === "ready" ? null : controls.status;
	return (
		<section
			className="product-demo-playbacks"
			aria-label="Virtual playback controls"
			aria-busy={controls.status.kind === "loading" || undefined}
			style={{ position: "relative" }}
		>
			{pending && (
				<p
					className="product-demo-playback-status"
					role={pending.kind === "error" ? "alert" : "status"}
					style={PLAYBACK_STATUS_STYLE}
				>
					{pending.message}
				</p>
			)}
			<div className="product-demo-playback-top-row">
				{DEMO_PLAYBACK_TOP_SLOTS.map((slot) => (
					<DemoPlaybackButton
						controls={controls}
						slot={slot}
						label={String(slot)}
						key={slot}
					/>
				))}
			</div>
			<div className="product-demo-playback-strips">
				{DEMO_PLAYBACK_STRIP_SLOTS.map((slot) => (
					<DemoPlaybackStrip controls={controls} slot={slot} key={slot} />
				))}
			</div>
		</section>
	);
}

const PLAYBACK_STATUS_STYLE: CSSProperties = {
	position: "absolute",
	inset: 4,
	zIndex: 3,
	display: "grid",
	placeItems: "center",
	margin: 0,
	padding: 8,
	background: "rgba(7, 11, 14, 0.88)",
	color: "#a9e9f0",
	font: "700 10px ui-monospace, monospace",
	textAlign: "center",
	pointerEvents: "none",
};
