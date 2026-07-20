import type { KeyboardEvent } from "react";
import { Button } from "../../common";
import { HorizontalTouchFader } from "../../control/HorizontalTouchFader";

interface OutputControlsProps {
	master: number;
	blackout: boolean;
	lampResult: string;
	lampActionsAvailable: boolean;
	onMaster(value: number): void;
	onBlackout(): void;
	onLamp(phase: "click" | "press" | "release"): void;
}

export function OutputControls(props: OutputControlsProps) {
	return (
		<>
			<h3>Output controls</h3>
			<section className="master-controls">
				<HorizontalTouchFader
					label="Grand master"
					value={props.master}
					onChange={props.onMaster}
				/>
				<Button
					className={props.blackout ? "danger active" : "danger"}
					onClick={props.onBlackout}
				>
					{props.blackout ? "RELEASE BLACKOUT" : "BLACKOUT"}
				</Button>
				<LampOnButton
					disabled={!props.lampActionsAvailable}
					onLamp={props.onLamp}
				/>
			</section>
			{props.lampResult && (
				<p className="lamp-command-result">{props.lampResult}</p>
			)}
		</>
	);
}

function LampOnButton({
	disabled,
	onLamp,
}: {
	disabled: boolean;
	onLamp(phase: "click" | "press" | "release"): void;
}) {
	const keyDown = (event: KeyboardEvent) => {
		if (!event.repeat && (event.key === "Enter" || event.key === " "))
			onLamp("press");
	};
	const keyUp = (event: KeyboardEvent) => {
		if (event.key === "Enter" || event.key === " ") onLamp("release");
	};
	return (
		<Button
			className="lamp-on-all"
			disabled={disabled}
			onClick={() => onLamp("click")}
			onPointerDown={() => onLamp("press")}
			onPointerUp={() => onLamp("release")}
			onPointerCancel={() => onLamp("release")}
			onKeyDown={keyDown}
			onKeyUp={keyUp}
		>
			All Lamps On
		</Button>
	);
}
