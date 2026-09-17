import { ModalPortal, ModalTitleBar } from "@tosklight/ui";
import { useEffect, useRef, useState } from "react";
import type { RecordUpdateOption, UpdateSettings } from "../../api/types";
import { useProgrammingUpdate } from "../../features/programmingUpdate/ProgrammingUpdateProvider";
import { useApp } from "../../state/AppContext";
import {
	loadRecordSettings,
	RecordDefaultsFields,
	saveRecordSettings,
} from "../setup/ProgrammerDefaults";

export function StoreSettingsModal() {
	const { state, dispatch } = useApp();
	const update = useProgrammingUpdate();
	const [settings, setSettings] = useState(loadRecordSettings);
	const [deskSettings, setDeskSettings] = useState<UpdateSettings | null>(null);
	const [error, setError] = useState<string | null>(null);
	const saveQueue = useRef(Promise.resolve());
	const open = state.storeSettingsOpen;
	// biome-ignore lint/correctness/useExhaustiveDependencies: reload each time the modal opens
	useEffect(() => {
		if (!open) return;
		let current = true;
		setSettings(loadRecordSettings());
		setDeskSettings(null);
		setError(null);
		void (async () => {
			try {
				const next = (await update?.loadSettings()) ?? null;
				if (!current) return;
				setDeskSettings(next);
				if (!next) setError("The Record default could not be loaded.");
			} catch (reason) {
				if (current) setError(errorMessage(reason));
			}
		})();
		return () => {
			current = false;
		};
	}, [open]);
	if (!open) return null;
	const close = () =>
		dispatch({ type: "SET_MODAL", modal: "storeSettingsOpen", value: false });
	const change = (next: typeof settings) => {
		setSettings(next);
		saveRecordSettings(next);
	};
	const changeRecordDefault = (recordDefault: RecordUpdateOption) => {
		if (!deskSettings) return;
		const next = { ...deskSettings, record_default: recordDefault };
		setDeskSettings(next);
		saveQueue.current = saveQueue.current.then(async () => {
			try {
				const saved = await update?.saveSettings(next);
				setError(saved ? null : "The Record default was not saved.");
			} catch (reason) {
				setError(errorMessage(reason));
			}
		});
	};
	return (
		<ModalPortal onClose={close}>
			<div
				className="modal-backdrop"
				onPointerDown={(event) =>
					event.target === event.currentTarget && close()
				}
			>
				<section
					className="modal-card store-settings-modal workflow-theme record-workflow"
					role="dialog"
					aria-modal="true"
					aria-label="Record Settings"
				>
					<ModalTitleBar
						title={
							<>
								<span className="workflow-badge">RECORD</span> Settings
							</>
						}
						accept={{
							id: "done",
							label: "Done",
							variant: "primary",
							onPress: close,
						}}
						closeLabel="Close Record Settings"
						onClose={close}
					/>
					<p>
						Defaults for the next Record on this desk. They do not change show
						programming.
					</p>
					<RecordDefaultsFields
						settings={settings}
						onChange={change}
						recordDefault={deskSettings?.record_default ?? "smart"}
						recordDefaultDisabled={!deskSettings}
						onRecordDefault={changeRecordDefault}
					/>
					{error && (
						<p className="modal-error" role="alert">
							{error}
						</p>
					)}
				</section>
			</div>
		</ModalPortal>
	);
}

function errorMessage(reason: unknown) {
	return reason instanceof Error ? reason.message : String(reason);
}
