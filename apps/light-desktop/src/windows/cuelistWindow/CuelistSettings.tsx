import { Button, ModalPortal, ModalTitleBar } from "../../components/common";
import { CuelistSettingsFields } from "./CuelistSettingsFields";
import {
	RenumberCuesDialog,
	UnsavedSettingsDialog,
} from "./CuelistSettingsOverlays";
import {
	type CuelistSettingsController,
	type CuelistSettingsProps,
	useCuelistSettings,
} from "./useCuelistSettings";

function ModeControl({
	controller,
}: {
	controller: CuelistSettingsController;
}) {
	const { draft, replaceDraft, modeMenuOpen, setModeMenuOpen } = controller;
	const chooseMode = (mode: "sequence" | "chaser") => {
		replaceDraft({
			...draft,
			mode,
			speed_group:
				mode === "chaser" && draft.speed_group == null
					? "A"
					: draft.speed_group,
		});
		setModeMenuOpen(false);
	};
	return (
		<div className="cuelist-mode-title-menu">
			<Button
				className="cuelist-mode-title-trigger"
				aria-haspopup="menu"
				aria-expanded={modeMenuOpen}
				onClick={() => setModeMenuOpen((open) => !open)}
			>
				<span>Mode</span>
				<small>({draft.mode === "chaser" ? "Chaser" : "Sequence"})</small>
				<i aria-hidden="true">▾</i>
			</Button>
			{modeMenuOpen && (
				<div className="cuelist-mode-title-panel" role="menu" aria-label="Mode">
					{(["sequence", "chaser"] as const).map((mode) => (
						<Button
							key={mode}
							role="menuitemradio"
							aria-checked={draft.mode === mode}
							onClick={() => chooseMode(mode)}
						>
							<span aria-hidden="true">{draft.mode === mode ? "✓" : ""}</span>
							{mode === "chaser" ? "Chaser" : "Sequence"}
						</Button>
					))}
				</div>
			)}
		</div>
	);
}

export function CuelistSettings(props: CuelistSettingsProps) {
	const controller = useCuelistSettings(props);
	const { draft, requestClose, settingsError, submit, setRenumberOpen } =
		controller;
	return (
		<ModalPortal>
			<div
				className="stacked-modal-layer cuelist-settings-backdrop"
				onPointerDown={(event) => {
					if (event.target === event.currentTarget) requestClose();
				}}
			>
				<section
					className="nested-modal cuelist-settings-modal"
					role="dialog"
					aria-modal="true"
					aria-label="Cuelist Settings"
				>
					<ModalTitleBar
						title="Cuelist Settings"
						details={
							<>
								<b>{draft.name}</b>
								<small>
									{draft.cues.length} {draft.cues.length === 1 ? "Cue" : "Cues"}
								</small>
							</>
						}
						actions={
							<>
								<ModeControl controller={controller} />
								<Button
									disabled={!draft.cues.length}
									onClick={() => setRenumberOpen(true)}
								>
									Renumber Cues
								</Button>
								<Button variant="primary" onClick={() => void submit()}>
									Save
								</Button>
							</>
						}
						closeLabel="Close Cuelist Settings"
						onClose={requestClose}
					/>
					<CuelistSettingsFields
						controller={controller}
						priority={props.object.body.priority}
					/>
					{settingsError && (
						<p className="ui-field-error" role="alert">
							{settingsError}
						</p>
					)}
					<RenumberCuesDialog controller={controller} />
					<UnsavedSettingsDialog
						controller={controller}
						discard={props.close}
					/>
				</section>
			</div>
		</ModalPortal>
	);
}
