import { useEffect, useMemo, useRef, useState } from "react";
import { useSessionSnapshot } from "../../features/deskSnapshot/DeskSnapshotState";
import { Button, SelectField } from "../common";
import { useSoundDeviceSelection } from "../control/useSoundDeviceSelection";

export function SoundInputSettings() {
	const session = useSessionSnapshot();
	const mounted = useRef(true);
	const [requestError, setRequestError] = useState<string | null>(null);
	useEffect(
		() => () => {
			mounted.current = false;
		},
		[],
	);
	const sound = useSoundDeviceSelection(
		session?.desk.id ?? null,
		mounted,
		true,
	);
	const options = useMemo(
		() => [
			{ value: "", label: "Not assigned on this desk" },
			{ value: "default", label: "System default input" },
			...sound.devices
				.filter((device) => device.deviceId && device.deviceId !== "default")
				.map((device) => ({
					value: device.deviceId,
					label: device.label,
				})),
		],
		[sound.devices],
	);
	const requestPermission = async () => {
		setRequestError(null);
		try {
			const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
			for (const track of stream.getTracks()) track.stop();
			await sound.refreshInputs();
		} catch (reason) {
			setRequestError(reason instanceof Error ? reason.message : String(reason));
		}
	};
	return (
		<article className="sound-input-settings">
			<div>
				<b>Sound-to-Light audio input</b>
				<span>Microphone permission: {sound.permission}</span>
			</div>
			<SelectField
				label="Audio input"
				value={sound.deviceId}
				options={options}
				onChange={sound.setDeskDevice}
				description="This desk/browser selection is never stored in a portable show."
			/>
			<div>
				<Button onClick={() => void requestPermission()}>
					Request microphone access
				</Button>
				<Button onClick={() => void sound.refreshInputs()}>
					Refresh inputs
				</Button>
			</div>
			{requestError && <p role="alert">{requestError}</p>}
		</article>
	);
}
