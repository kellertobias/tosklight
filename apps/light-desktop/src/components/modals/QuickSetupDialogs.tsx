import { Button, ErrorAlert, ModalTitleBar, TextInput } from "@tosklight/ui";
import { useEffect, useRef, useState } from "react";
import { RootConfinedFilePickerButton } from "../files/RootConfinedFilePickerButton";
import { MvrImportPreview } from "./MvrImportPreview";
import { MvrInspectionProgress } from "./MvrInspectionProgress";
import { NewShowDialog } from "./NewShowDialog";
import { ShowLoadBrowser } from "./ShowLoadBrowser";
import { ShowSaveBrowser } from "./ShowSaveBrowser";

export { MvrImportPreview } from "./MvrImportPreview";

import type { QuickSetupModel } from "./QuickSetupModal";
import { SelectiveShowImportModal } from "./SelectiveShowImportModal";
import { StackedModal } from "./StackedModal";

interface ModelProps {
	model: QuickSetupModel;
}

function NamedRevisionDialog({ model }: ModelProps) {
	const { activeRevisions } = model.view;
	const { saveNamedRevision } = model.actions;
	const { revisionName, setRevisionName, setRevisionOpen } = model.dialogs;
	if (!model.dialogs.revisionOpen) return null;
	return (
		<StackedModal onClose={() => setRevisionOpen(false)}>
			<div
				className="nested-modal named-revision-modal"
				role="dialog"
				aria-modal="true"
				aria-label="Save named revision"
			>
				<ModalTitleBar
					title="Save Named Revision"
					onClose={() => setRevisionOpen(false)}
				/>
				<p>
					This creates a restore point from the current autosaved show. Autosave
					continues afterward.
				</p>
				<TextInput
					clearable
					className="show-name-input"
					autoFocus
					value={revisionName}
					onChange={(event) => setRevisionName(event.target.value)}
					onKeyboardCommit={(value) => void saveNamedRevision(value)}
					placeholder="e.g. Before trying alternate cue timing"
					aria-label="Revision name"
				/>
				<footer>
					<Button onClick={() => setRevisionOpen(false)}>Cancel</Button>
					<Button
						variant="primary"
						disabled={!revisionName.trim()}
						onClick={() => void saveNamedRevision()}
					>
						Save Revision {(activeRevisions[0]?.revision ?? 0) + 1}
					</Button>
				</footer>
			</div>
		</StackedModal>
	);
}

function CopySaveDialog({ model }: ModelProps) {
	const { originalShow } = model.view;
	const { requestOverwrite } = model.actions;
	const { copySaveOpen, setCopySaveOpen } = model.dialogs;
	if (!copySaveOpen) return null;
	return (
		<StackedModal onClose={() => setCopySaveOpen(false)}>
			<div
				className="nested-modal revision-copy-save-modal"
				role="dialog"
				aria-modal="true"
				aria-label="Save revision copy"
			>
				<ModalTitleBar
					title="Save Revision Copy"
					onClose={() => setCopySaveOpen(false)}
				/>
				<p>
					Autosave already protects this copy. Choose where this copy should
					remain.
				</p>
				<div className="dialog-grid">
					<Button variant="primary" onClick={() => setCopySaveOpen(false)}>
						Keep as Separate Show
					</Button>
					{originalShow ? (
						<Button onClick={() => requestOverwrite(originalShow)}>
							Overwrite Original Show
						</Button>
					) : (
						<p className="modal-warning">
							The original show is no longer available. This copy remains an
							independent show.
						</p>
					)}
					<Button onClick={() => setCopySaveOpen(false)}>Cancel</Button>
				</div>
			</div>
		</StackedModal>
	);
}

function SaveAsDialog({ model }: ModelProps) {
	return model.dialogs.saveAsOpen ? <ShowSaveBrowser model={model} /> : null;
}

function OverwriteDialog({ model }: ModelProps) {
	const { confirmOverwrite } = model.actions;
	const { overwriteBusy, overwriteTarget, setOverwriteTarget } = model.dialogs;
	if (!overwriteTarget) return null;
	const close = () => {
		if (!overwriteBusy) setOverwriteTarget(null);
	};
	return (
		<StackedModal onClose={close}>
			<div
				className="nested-modal overwrite-show-confirm"
				role="alertdialog"
				aria-modal="true"
				aria-label={`Confirm overwrite ${overwriteTarget.name}`}
			>
				<ModalTitleBar
					title={`Replace ${overwriteTarget.name} Latest Autosave?`}
					closeDisabled={overwriteBusy}
					onClose={close}
				/>
				<p>
					This replaces only <b>{overwriteTarget.name}</b>&apos;s mutable Latest
					Autosave with the active show state. Its identity and named revisions
					are preserved.
				</p>
				<p>
					The active revision copy and its immutable source revision are
					retained.
				</p>
				<div className="modal-actions">
					<Button autoFocus disabled={overwriteBusy} onClick={close}>
						Cancel
					</Button>
					<Button
						className="danger"
						disabled={overwriteBusy}
						onClick={() => void confirmOverwrite()}
					>
						{overwriteBusy
							? "Replacing Latest Autosave…"
							: `Replace ${overwriteTarget.name} Latest Autosave`}
					</Button>
				</div>
			</div>
		</StackedModal>
	);
}

function LoadDialog({ model }: ModelProps) {
	if (!model.dialogs.loadOpen) return null;
	return <ShowLoadBrowser model={model} />;
}

function SelectiveImportDialog({ model }: ModelProps) {
	const { activeShow } = model.view;
	const { lifecycle, selectiveImport } = model.authorities;
	const dialogs = model.dialogs;
	if (!dialogs.selectiveImportOpen || !activeShow) return null;
	return (
		<StackedModal onClose={() => dialogs.selectiveImportClose.current?.()}>
			<SelectiveShowImportModal
				activeShow={activeShow}
				shows={
					model.dialogs.partialSource
						? [
								model.dialogs.partialSource,
								...(lifecycle?.shows ?? []).filter(
									(show) => show.id !== model.dialogs.partialSource?.id,
								),
							]
						: (lifecycle?.shows ?? [])
				}
				initialSourceShowId={model.dialogs.partialSource?.id}
				closeTriggerRef={dialogs.selectiveImportClose}
				onClose={() => dialogs.setSelectiveImportOpen(false)}
				loadCatalog={selectiveImport.catalog}
				previewImport={selectiveImport.preview}
				applyImport={selectiveImport.apply}
			/>
		</StackedModal>
	);
}

function MvrShowPicker({ model }: ModelProps) {
	const { lifecycle } = model.authorities;
	const { setMvrTarget } = model.mvr;
	return (
		<>
			<p>Select any show in the desk library.</p>
			<div className="show-library">
				{(lifecycle?.shows ?? []).map((show) => (
					<article key={show.id}>
						<span>
							<b>{show.name}</b>
							<small>Autosaved show file</small>
						</span>
						<Button onClick={() => setMvrTarget(show)}>Select</Button>
					</article>
				))}
			</div>
		</>
	);
}

function MvrFilePicker({
	model,
	onReadBusyChange,
}: ModelProps & { onReadBusyChange: (busy: boolean) => void }) {
	const {
		inspectMvr,
		mvrBusy,
		mvrFilePickerRequested,
		mvrFilePickerTrigger,
		mvrMode,
		mvrTarget,
		setMvrFilePickerRequested,
	} = model.mvr;
	useEffect(() => {
		if (!mvrFilePickerRequested) return;
		const timer = globalThis.setTimeout(() => {
			if (!mvrFilePickerTrigger.current) return;
			setMvrFilePickerRequested(false);
			mvrFilePickerTrigger.current();
		}, 0);
		return () => globalThis.clearTimeout(timer);
	}, [mvrFilePickerRequested, mvrFilePickerTrigger, setMvrFilePickerRequested]);
	if (model.mvr.mvrPreview) return null;
	return (
		<>
			<p>
				{mvrMode === "merge" && mvrTarget ? (
					<>
						Import into <b>{mvrTarget.name}</b>. Existing programming and
						unmatched scenery are retained.
					</>
				) : (
					<>
						Create a new show from MVR fixtures, patch, transforms, and scene
						geometry.
					</>
				)}
			</p>
			<RootConfinedFilePickerButton
				triggerRef={mvrFilePickerTrigger}
				variant="primary"
				disabled={mvrBusy}
				label={mvrBusy ? "Inspecting…" : "Choose MVR file"}
				allowedExtensions={["mvr"]}
				onReadBusyChange={onReadBusyChange}
				onFiles={(files) => {
					const file = files[0];
					if (file) return inspectMvr(file);
				}}
			/>
		</>
	);
}

export function MvrDialog({ model }: ModelProps) {
	const mvr = model.mvr;
	const [readingFile, setReadingFile] = useState(false);
	const readingRef = useRef(false);
	const [readStartedAt, setReadStartedAt] = useState(Date.now());
	const readBusyChanged = (value: boolean) => {
		readingRef.current = value;
		setReadingFile(value);
		if (value) setReadStartedAt(Date.now());
	};
	const requestClose = () => {
		if (!readingRef.current) mvr.setMvrMode(null);
	};
	if (!mvr.mvrMode) return null;
	const needsShow = mvr.mvrMode !== "new" && !mvr.mvrTarget;
	return (
		<StackedModal onClose={requestClose}>
			<div
				className="nested-modal mvr-modal import-workflow"
				role="dialog"
				aria-modal="true"
				aria-label="MVR import"
			>
				<header className="import-workflow__header">
					<ModalTitleBar
						title={
							mvr.mvrMode === "new" ? "New Show from MVR" : "Add MVR to Show"
						}
						closeDisabled={mvr.mvrOperation === "apply" || readingFile}
						onClose={requestClose}
					/>
				</header>
				{readingFile && (
					<MvrInspectionProgress
						operation="read"
						startedAt={readStartedAt}
						file={null}
						onCancel={requestClose}
					/>
				)}
				{mvr.mvrOperation && mvr.mvrStartedAt !== null && (
					<MvrInspectionProgress
						operation={mvr.mvrOperation}
						startedAt={mvr.mvrStartedAt}
						file={mvr.mvrInspectionFile}
						onCancel={() => mvr.setMvrMode(null)}
					/>
				)}
				{!mvr.mvrPreview && (
					<div className="import-workflow__body">
						{mvr.mvrError && (
							<ErrorAlert role="alert">{mvr.mvrError}</ErrorAlert>
						)}
						{needsShow && <MvrShowPicker model={model} />}
						{!needsShow && (
							<MvrFilePicker model={model} onReadBusyChange={readBusyChanged} />
						)}
					</div>
				)}
				<MvrImportPreview model={model} />
				{!mvr.mvrPreview && (
					<footer className="import-workflow__footer">
						<Button
							disabled={mvr.mvrOperation === "apply" || readingFile}
							onClick={requestClose}
						>
							Cancel
						</Button>
					</footer>
				)}
			</div>
		</StackedModal>
	);
}

function ShutdownDialog({ model }: ModelProps) {
	const { confirmShutdown, setConfirmShutdown } = model.dialogs;
	if (!confirmShutdown) return null;
	return (
		<StackedModal onClose={() => setConfirmShutdown(false)}>
			<div
				className="nested-modal shutdown-modal"
				role="alertdialog"
				aria-modal="true"
			>
				<ModalTitleBar
					title="Shut Down Desk?"
					onClose={() => setConfirmShutdown(false)}
				/>
				<p>
					Hazardous fixtures will be driven to their safe values before the
					server stops. This desk application will then close.
				</p>
				<div className="modal-actions">
					<Button onClick={() => setConfirmShutdown(false)}>Cancel</Button>
					<Button
						className="danger"
						onClick={() => void model.actions.shutDownDesk()}
					>
						Shut Down Safely
					</Button>
				</div>
			</div>
		</StackedModal>
	);
}

export function QuickSetupDialogs({ model }: ModelProps) {
	return (
		<>
			<NamedRevisionDialog model={model} />
			<CopySaveDialog model={model} />
			<SaveAsDialog model={model} />
			<OverwriteDialog model={model} />
			<LoadDialog model={model} />
			<SelectiveImportDialog model={model} />
			<NewShowDialog model={model} />
			<MvrDialog model={model} />
			<ShutdownDialog model={model} />
		</>
	);
}
