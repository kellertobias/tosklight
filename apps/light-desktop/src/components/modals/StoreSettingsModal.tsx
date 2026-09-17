import { ModalPortal, ModalTitleBar } from "@tosklight/ui";
import { useEffect, useState } from "react";
import { useApp } from "../../state/AppContext";
import {
	loadRecordSettings,
	RecordDefaultsFields,
	saveRecordSettings,
} from "../setup/ProgrammerDefaults";

export function StoreSettingsModal() {
	const { state, dispatch } = useApp();
	const [settings, setSettings] = useState(loadRecordSettings);
	useEffect(() => {
		if (state.storeSettingsOpen) setSettings(loadRecordSettings());
	}, [state.storeSettingsOpen]);
	if (!state.storeSettingsOpen) return null;
	const close = () =>
		dispatch({ type: "SET_MODAL", modal: "storeSettingsOpen", value: false });
	const change = (next: typeof settings) => {
		setSettings(next);
		saveRecordSettings(next);
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
					<RecordDefaultsFields settings={settings} onChange={change} />
				</section>
			</div>
		</ModalPortal>
	);
}
