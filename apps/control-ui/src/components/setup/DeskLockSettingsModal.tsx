import { useEffect, useState } from "react";
import { useServer } from "../../api/ServerContext";
import {
	Button,
	FormField,
	FormLayout,
	ModalTitleBar,
	SelectField,
	TextAreaField,
	TextField,
} from "../common";
import { useDeskLockActions } from "../../features/deskLock/DeskLockActionsProvider";
import { useDeskLock } from "../../features/deskLock/DeskLockState";
import { RootConfinedFilePickerButton } from "../files/RootConfinedFilePickerButton";

export function DeskLockSettingsModal({ onClose }: { onClose: () => void }) {
	const server = useServer();
	const deskLock = useDeskLock();
	const deskLockActions = useDeskLockActions();
	const [message, setMessage] = useState(deskLock?.message ?? "Desk locked");
	const [wallpaper, setWallpaper] = useState<string | null>(deskLock?.wallpaper ?? null);
	const [unlockMode, setUnlockMode] = useState<"button" | "pin">(deskLock?.unlock_mode ?? "button");
	const [pin, setPin] = useState("");
	useEffect(() => {
		if (!deskLock) return;
		setMessage(deskLock.message);
		setWallpaper(deskLock.wallpaper);
		setUnlockMode(deskLock.unlock_mode);
	}, [deskLock]);
	const save = async () => {
		const saved = await deskLockActions?.configureDeskLock({
			message,
			wallpaper,
			unlock_mode: unlockMode,
			...(pin ? { pin } : {}),
		});
		if (saved) onClose();
	};
	return (
		<div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && onClose()}>
			<section className="nested-modal desk-lock-settings-modal" role="dialog" aria-modal="true" aria-label="Desk Lock">
				<ModalTitleBar
					title="Desk Lock"
					closeLabel="Close Desk Lock settings"
					onClose={onClose}
					actions={<Button variant="primary" onClick={() => void save()}>Save Lock Configuration</Button>}
				/>
				<p>Locking this desk blocks every connected screen and its assigned hardware without changing playback, programmer, or output state.</p>
				<FormLayout labelPlacement="side">
					<TextAreaField label="Lock message" value={message} onChange={(event) => setMessage(event.target.value)} />
					<SelectField
						label="Unlock control"
						value={unlockMode}
						onChange={setUnlockMode}
						options={[
							{ value: "button", label: "Unlock button" },
							{ value: "pin", label: "PIN required" },
						]}
					/>
					{unlockMode === "pin" && (
						<TextField
							label="New PIN"
							secure
							inputMode="numeric"
							value={pin}
							description="4–12 digits. Leave empty to retain the configured PIN."
							onChange={(event) => setPin(event.target.value.replace(/\D/g, ""))}
						/>
					)}
					<FormField label="Wallpaper">
						<RootConfinedFilePickerButton
							label="Choose lock wallpaper"
							allowedExtensions={["png", "jpg", "jpeg", "gif", "webp"]}
							onFiles={(files) => {
								const file = files[0];
								if (!file) return;
								const reader = new FileReader();
								reader.onload = () => setWallpaper(String(reader.result));
								reader.readAsDataURL(file);
							}}
						/>
						{wallpaper && <Button onClick={() => setWallpaper(null)}>Use default wallpaper</Button>}
					</FormField>
				</FormLayout>
				{server.error && <p className="modal-error" role="alert">{server.error}</p>}
			</section>
		</div>
	);
}
