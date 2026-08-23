import {
	Button,
	Input,
	ModalTitleBar,
	NumberField,
	SelectField,
	TextInput,
	type TitleAction,
} from "@tosklight/ui";
import { useEffect, useState } from "react";
import type { DiscoveredPeer } from "../../api/client/discovery";
import { RootConfinedFilePickerButton } from "../files/RootConfinedFilePickerButton";
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
				<ModalTitleBar title="Save Named Revision" onClose={() => setRevisionOpen(false)} />
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
				<ModalTitleBar title="Save Revision Copy" onClose={() => setCopySaveOpen(false)} />
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

function SaveAsDestinations({ model }: ModelProps) {
	const { activeShowId, revisionCopy } = model.view;
	const { lifecycle } = model.authorities;
	const { requestOverwrite } = model.actions;
	const shows = lifecycle?.shows ?? [];
	const destinations = shows.filter((show) => show.id !== activeShowId);
	if (model.view.activeShowIsProvisional || destinations.length === 0)
		return null;
	return (
		<>
			<h4>Or replace an existing Latest Autosave</h4>
			<div className="show-library overwrite-destination-list">
				{destinations.map((show) => (
					<article key={show.id}>
						<span>
							<b>{show.name}</b>
							<small>
								{show.id === revisionCopy?.show_id
									? "Original show"
									: "Existing show"}
							</small>
						</span>
						<Button onClick={() => requestOverwrite(show)}>
							Choose Destination
						</Button>
					</article>
				))}
			</div>
		</>
	);
}

function SaveAsDialog({ model }: ModelProps) {
	const { activeShowIsProvisional, flashDriveConnected } = model.view;
	const { openMvrExport } = model.mvr;
	const { saveAs } = model.actions;
	const dialogs = model.dialogs;
	if (!dialogs.saveAsOpen) return null;
	return (
		<StackedModal onClose={() => dialogs.setSaveAsOpen(false)}>
			<div
				className="nested-modal"
				role="dialog"
				aria-modal="true"
				aria-label="Save show"
			>
				<ModalTitleBar
					title={activeShowIsProvisional ? "Name Empty Show" : "Save Show As"}
					groups={[
						{
							id: "export",
							actions: [
								{
									id: "export-mvr",
									label: "Export as MVR",
									onPress: openMvrExport,
								},
							],
						},
					]}
					accept={{
						id: "save",
						label: activeShowIsProvisional
							? "Name Empty Show"
							: "Save as New Show",
						variant: "primary",
						disabled: !dialogs.showName.trim(),
						onPress: () => void saveAs(),
					}}
					closeLabel="Close Save Show"
					onClose={() => dialogs.setSaveAsOpen(false)}
				/>
				{activeShowIsProvisional && (
					<p>
						This empty show is already autosaved. Naming it keeps the same show
						and all current programming.
					</p>
				)}
				<div className="save-destination">
					<Button
						className={dialogs.destination === "local" ? "active" : ""}
						onClick={() => dialogs.setDestination("local")}
					>
						This desk
					</Button>
					{flashDriveConnected && (
						<Button
							className={dialogs.destination === "flash" ? "active" : ""}
							onClick={() => dialogs.setDestination("flash")}
						>
							Connected flash drive
						</Button>
					)}
				</div>
				<TextInput
					clearable
					className="show-name-input"
					autoFocus
					value={dialogs.showName}
					onChange={(event) => dialogs.setShowName(event.target.value)}
					onKeyboardCommit={(value) => void saveAs(value)}
					placeholder="New show name"
					aria-label="Show name"
				/>
				<SaveAsDestinations model={model} />
			</div>
		</StackedModal>
	);
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
				<ModalTitleBar title={`Replace ${overwriteTarget.name} Latest Autosave?`} closeDisabled={overwriteBusy} onClose={close} />
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

function NamedRevisionList({ model, showId }: ModelProps & { showId: string }) {
	const revisions = model.view.revisionsByShow[showId] ?? [];
	if (revisions.length === 0) return <small>No manually saved revisions</small>;
	return revisions.map((revision) => (
		<Button
			key={revision.revision}
			onClick={() =>
				void model.actions.loadNamedRevision(showId, revision.revision)
			}
		>
			<span>
				<b>
					Revision {revision.revision} · {revision.name}
				</b>
				<small>{new Date(revision.created_at).toLocaleString()}</small>
			</span>
			<i>Load Revision as Copy</i>
		</Button>
	));
}

/**
 * The Viz editors on the network, offered as somewhere to load a rig from.
 *
 * Nothing is rendered when there is nothing to load: an operator with no visualizer running
 * should not be shown a button that can only fail. Two editors are two buttons, told apart by
 * name and — in the tooltip — address, because that is what distinguishes them.
 */
function useLoadFromVisualizerActions(model: QuickSetupModel): TitleAction[] {
	const { lifecycle } = model.authorities;
	const open = model.dialogs.loadOpen;
	const [visualizers, setVisualizers] = useState<DiscoveredPeer[]>([]);
	useEffect(() => {
		if (!open || !lifecycle) {
			setVisualizers([]);
			return;
		}
		let current = true;
		// Read once as the menu opens: a peer that leaves while it is open is caught by the
		// load itself failing, which is honest, rather than by a list that flickers.
		void lifecycle
			.discoveredVisualizers()
			.then((found) => current && setVisualizers(found));
		return () => {
			current = false;
		};
	}, [open, lifecycle]);
	return visualizers.map((visualizer) => ({
		id: `visualizer-${visualizer.instance}`,
		label: `Load from Visualizer · ${visualizer.name}: ${visualizer.show}`,
		ariaLabel: `Load from ${visualizer.name} at ${visualizer.address}`,
		onPress: async () => {
			if (await lifecycle?.loadFromVisualizer(visualizer.instance))
				model.dialogs.setLoadOpen(false);
		},
	}));
}

function LoadShowLibrary({ model }: ModelProps) {
	const { lifecycle } = model.authorities;
	const { activeShowId, revisionsByShow } = model.view;
	return (
		<div className="show-library revision-show-library">
			<article className="built-in-default-show">
				<span>
					<b>Built-in Default Stage Show</b>
					<small>Untouched completed demo show</small>
				</span>
				<Button
					variant="primary"
					onClick={async () => {
						if (await lifecycle?.openCleanDefaultShow())
							model.dialogs.setLoadOpen(false);
					}}
				>
					Load Clean Built-in Default
				</Button>
			</article>
			{(lifecycle?.shows ?? []).map((show) => (
				<article
					key={show.id}
					className={show.id === activeShowId ? "active" : ""}
				>
					<span>
						<b>{show.name}</b>
						<small>
							{(revisionsByShow[show.id] ?? []).length} named revisions
						</small>
					</span>
					<Button
						onClick={() => {
							void lifecycle?.openShow(show.id);
							model.dialogs.setLoadOpen(false);
						}}
					>
						Load Latest Autosave
					</Button>
					<div className="named-revision-list">
						<NamedRevisionList model={model} showId={show.id} />
					</div>
				</article>
			))}
		</div>
	);
}

function LoadDialog({ model }: ModelProps) {
	const { lifecycle } = model.authorities;
	const dialogs = model.dialogs;
	const visualizerActions = useLoadFromVisualizerActions(model);
	if (!dialogs.loadOpen) return null;
	return (
		<StackedModal onClose={() => dialogs.setLoadOpen(false)}>
			<div
				className="nested-modal load-show-modal"
				role="dialog"
				aria-modal="true"
				aria-label="Load show"
			>
				<ModalTitleBar
					title="Load Show"
					groups={[
						{
							id: "load",
							actions: [
								{
									id: "partial",
									label: "Partial Show Load",
									onPress: () => {
										dialogs.setLoadOpen(false);
										dialogs.setSelectiveImportOpen(true);
									},
								},
								{
									id: "mvr",
									label: "Load from MVR",
									onPress: () =>
										model.mvr.openMvrImport(() => dialogs.setLoadOpen(false)),
								},
								...visualizerActions,
								{
									id: "usb",
									label: "Show from USB",
									onPress: () => dialogs.usbShowPickerTrigger.current?.(),
								},
								{
									id: "os",
									label: "Show from OS",
									onPress: () => dialogs.osShowPickerInput.current?.click(),
								},
							],
						},
					]}
					closeLabel="Close Load Show"
					onClose={() => dialogs.setLoadOpen(false)}
				/>
				<Input
					ref={dialogs.osShowPickerInput}
					hidden
					type="file"
					accept=".show"
					onChange={(event) => {
						const file = event.target.files?.[0];
						if (file) void lifecycle?.uploadShow(file);
						event.target.value = "";
					}}
				/>
				<p>
					Load Latest Autosave always resumes that show&apos;s newest work. Load
					Clean Built-in Default creates a separate show from the untouched
					built-in rig. Load Revision as Copy creates and activates a separate
					autosaved show without changing the original.
				</p>
				<LoadShowLibrary model={model} />
				<RootConfinedFilePickerButton
					hideButton
					triggerRef={dialogs.usbShowPickerTrigger}
					label="Show from USB"
					allowedExtensions={["show"]}
					onFiles={(files) => {
						const file = files[0];
						if (file) return lifecycle?.uploadShow(file);
					}}
				/>
			</div>
		</StackedModal>
	);
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
				shows={lifecycle?.shows ?? []}
				closeTriggerRef={dialogs.selectiveImportClose}
				onClose={() => dialogs.setSelectiveImportOpen(false)}
				loadCatalog={selectiveImport.catalog}
				previewImport={selectiveImport.preview}
				applyImport={selectiveImport.apply}
			/>
		</StackedModal>
	);
}

function NewShowDialog({ model }: ModelProps) {
	const { lifecycle } = model.authorities;
	const { newShowOpen, setNewShowOpen } = model.dialogs;
	if (!newShowOpen) return null;
	return (
		<StackedModal onClose={() => setNewShowOpen(false)}>
			<div
				className="nested-modal new-show-modal"
				role="dialog"
				aria-modal="true"
				aria-label="New show"
			>
				<ModalTitleBar
					title="New Show"
					groups={[{ id: "new-show-source", actions: [{ id: "mvr", label: "Load from MVR", onPress: () => model.mvr.openMvrImport(() => setNewShowOpen(false)) }] }]}
					onClose={() => setNewShowOpen(false)}
				/>
				<p>
					Create and open a new empty show. The current show remains saved on
					this desk.
				</p>
				<Button
					className="primary"
					onClick={async () => {
						if (await lifecycle?.initializeEmptyShow()) setNewShowOpen(false);
					}}
				>
					Create Empty Show
				</Button>
			</div>
		</StackedModal>
	);
}

function MvrShowPicker({ model }: ModelProps) {
	const { lifecycle } = model.authorities;
	const { mvrMode, setMvrTarget, inspectExport } = model.mvr;
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
						<Button
							onClick={() =>
								mvrMode === "export"
									? void inspectExport(show)
									: setMvrTarget(show)
							}
						>
							Select
						</Button>
					</article>
				))}
			</div>
		</>
	);
}

function MvrFilePicker({ model }: ModelProps) {
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
	if (mvrMode === "export" || model.mvr.mvrPreview) return null;
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
				onFiles={(files) => {
					const file = files[0];
					if (file) return inspectMvr(file);
				}}
			/>
		</>
	);
}

function MvrFixtureRow({
	fixture,
	model,
}: ModelProps & {
	fixture: NonNullable<
		QuickSetupModel["mvr"]["mvrPreview"]
	>["fixtures"][number];
}) {
	const { mvrPreview, mvrResolutions, setMvrResolutions } = model.mvr;
	const resolution = mvrResolutions[fixture.uuid];
	const conflicted = mvrPreview?.address_conflicts.some((warning) =>
		warning.startsWith(fixture.name),
	);
	const update = (change: Record<string, string | number>) =>
		setMvrResolutions((current) => ({
			...current,
			[fixture.uuid]: {
				...current[fixture.uuid],
				action: "address",
				...change,
			},
		}));
	return (
		<article>
			<span>
				<b>{fixture.name}</b>
				<small>
					{fixture.gdtf_spec} · {fixture.gdtf_mode}
					{fixture.universe && fixture.address
						? ` · U${fixture.universe}.${fixture.address}`
						: " · Unpatched"}
				</small>
			</span>
			{conflicted && (
				<div>
					<SelectField
						label={`Resolution for ${fixture.name}`}
						value={resolution?.action ?? "import_unpatched"}
						options={[
							{ value: "import_unpatched", label: "Import unpatched" },
							{ value: "address", label: "Choose address" },
							{ value: "skip", label: "Skip" },
							{ value: "replace", label: "Replace conflict" },
						]}
						onChange={(action) =>
							setMvrResolutions((current) => ({
								...current,
								[fixture.uuid]: {
									action,
									universe: fixture.universe ?? 1,
									address: fixture.address ?? 1,
								},
							}))
						}
					/>
					{resolution?.action === "address" && (
						<div className="mvr-address-fields">
							<NumberField
								label="Universe"
								min={1}
								max={65535}
								aria-label={`Universe for ${fixture.name}`}
								value={resolution.universe ?? 1}
								onChange={(event) =>
									update({ universe: Number(event.target.value) })
								}
							/>
							<NumberField
								label="Address"
								min={1}
								max={512}
								aria-label={`Address for ${fixture.name}`}
								value={resolution.address ?? 1}
								onChange={(event) =>
									update({ address: Number(event.target.value) })
								}
							/>
						</div>
					)}
				</div>
			)}
		</article>
	);
}

function MvrImportPreview({ model }: ModelProps) {
	const mvr = model.mvr;
	if (!mvr.mvrPreview) return null;
	return (
		<>
			<div className="mvr-summary">
				<b>
					{mvr.mvrPreview.fixtures.length} fixtures · {mvr.mvrPreview.scenery}{" "}
					scenery objects
				</b>
				{mvr.mvrPreview.missing_profiles.length > 0 && (
					<p className="modal-warning">
						{mvr.mvrPreview.missing_profiles.length} fixture profiles will be
						imported as unresolved.
					</p>
				)}
				{mvr.mvrPreview.address_conflicts.map((warning) => (
					<p className="modal-warning" key={warning}>
						{warning}
					</p>
				))}
			</div>
			{mvr.mvrMode === "new" && (
				<TextInput
					clearable
					value={mvr.mvrName}
					onChange={(event) => mvr.setMvrName(event.target.value)}
					placeholder="Show name"
					aria-label="Show name"
				/>
			)}
			<div className="mvr-fixture-list">
				{mvr.mvrPreview.fixtures.map((fixture) => (
					<MvrFixtureRow key={fixture.uuid} fixture={fixture} model={model} />
				))}
			</div>
			<Button
				className="primary"
				disabled={mvr.mvrBusy || (mvr.mvrMode === "new" && !mvr.mvrName.trim())}
				onClick={() => void mvr.applyMvr()}
			>
				{mvr.mvrBusy
					? "Importing…"
					: mvr.mvrMode === "new"
						? "Create and Open Show"
						: `Add to ${mvr.mvrTarget?.name}`}
			</Button>
		</>
	);
}

function MvrExportPreview({ model }: ModelProps) {
	const { lifecycle } = model.authorities;
	const mvr = model.mvr;
	if (mvr.mvrMode !== "export" || !mvr.mvrTarget || !mvr.mvrExportPreview)
		return null;
	const target = mvr.mvrTarget;
	return (
		<>
			<div className="mvr-summary">
				<b>
					{mvr.mvrExportPreview.fixtures} fixtures ·{" "}
					{mvr.mvrExportPreview.scenery} scenery objects
				</b>
				<p>Not included: {mvr.mvrExportPreview.omitted.join(", ")}</p>
				{mvr.mvrExportPreview.warnings.map((warning) => (
					<p className="modal-warning" key={warning}>
						{warning}
					</p>
				))}
			</div>
			<Button
				className="primary"
				onClick={() => {
					void lifecycle?.downloadMvr(target);
					mvr.setMvrMode(null);
				}}
			>
				Download {target.name}.mvr
			</Button>
		</>
	);
}

function MvrDialog({ model }: ModelProps) {
	const mvr = model.mvr;
	if (!mvr.mvrMode) return null;
	const needsShow = mvr.mvrMode !== "new" && !mvr.mvrTarget;
	return (
		<StackedModal onClose={() => mvr.setMvrMode(null)}>
			<div
				className="nested-modal mvr-modal"
				role="dialog"
				aria-modal="true"
				aria-label="MVR import and export"
			>
				<ModalTitleBar
					title={mvr.mvrMode === "new"
						? "New Show from MVR"
						: mvr.mvrMode === "merge"
							? "Add MVR to Show"
							: "Export Show as MVR"}
					onClose={() => mvr.setMvrMode(null)}
				/>
				{needsShow && <MvrShowPicker model={model} />}
				{!needsShow && <MvrFilePicker model={model} />}
				<MvrImportPreview model={model} />
				<MvrExportPreview model={model} />
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
				<ModalTitleBar title="Shut Down Desk?" onClose={() => setConfirmShutdown(false)} />
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
