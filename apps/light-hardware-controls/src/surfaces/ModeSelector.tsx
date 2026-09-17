import type {
	DeviceStatus,
	HardwareMode,
	LastInput,
} from "../controller/types";
import { describeDevice } from "../transport/nativeBridge";

const modes: Array<{ mode: HardwareMode; label: string }> = [
	{ mode: "osc", label: "OSC" },
	{ mode: "native", label: "Native Hardware" },
];

interface ModeSelectorProps {
	activeMode: HardwareMode;
	setMode: (mode: HardwareMode) => Promise<void>;
}

export function ModeSelector({ activeMode, setMode }: ModeSelectorProps) {
	return (
		<fieldset className="mode-selector" aria-label="Input mode">
			{modes.map(({ mode, label }) => (
				<button
					key={mode}
					type="button"
					aria-pressed={activeMode === mode}
					className={activeMode === mode ? "active" : ""}
					onClick={() => void setMode(mode)}
				>
					{label}
				</button>
			))}
		</fieldset>
	);
}

interface LinkStatusProps {
	activeMode: HardwareMode;
	connected: boolean;
	page: number;
	device: DeviceStatus;
	lastInput: LastInput | null;
	linkError: string | null;
}

export function LinkStatus({
	activeMode,
	connected,
	page,
	device,
	lastInput,
	linkError,
}: LinkStatusProps) {
	const desk = connected
		? `● Desk connected · page ${page}`
		: linkError
			? `✕ ${linkError}`
			: "○ Connecting to desk…";
	const input =
		activeMode === "native"
			? "Input from the attached device · on-screen controls mirror only"
			: lastInput
				? `Last OSC input: ${lastInput.path}`
				: "No OSC input yet";
	return (
		<div className="link-status" role="status" aria-label="Link status">
			<strong className="link-mode">
				{activeMode === "native" ? "Native Hardware" : "OSC"}
			</strong>
			{activeMode === "native" && (
				<span className={`device-state-${device.state}`}>
					{describeDevice(device)}
				</span>
			)}
			<span
				className={
					connected ? "connected" : linkError ? "device-state-error" : ""
				}
			>
				{desk}
			</span>
			<small>{input}</small>
		</div>
	);
}
