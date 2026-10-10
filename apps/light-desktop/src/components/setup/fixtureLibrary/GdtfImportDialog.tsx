import {
	Button,
	ErrorAlert,
	ModalRegistration,
	ModalTitleBar,
	OperationBusyOverlay,
} from "@tosklight/ui";
import { type ReactNode, useRef, useState } from "react";
import { RootConfinedFilePickerButton } from "../../files/RootConfinedFilePickerButton";
import type { FixtureImportDialogsProps, PendingGdtfImport } from "./transfers";

type Props = Pick<
	FixtureImportDialogsProps,
	| "pendingGdtf"
	| "busy"
	| "error"
	| "close"
	| "confirmGdtfMappings"
	| "importGdtfFile"
	| "mappings"
	| "requirements"
> & { children: ReactNode };

function GdtfSourceSummary({ pending }: { pending: PendingGdtfImport }) {
	return (
		<div className="import-workflow__summary">
			<strong>
				{pending.profile.manufacturer} {pending.profile.name}
			</strong>
			<span>
				{pending.profile.modes.length}{" "}
				{pending.profile.modes.length === 1 ? "mode" : "modes"} → desk-wide
				fixture library
			</span>
			{pending.expectedRevision > 0 && (
				<small>
					Creates a new library revision. Patched fixtures keep their current
					revision.
				</small>
			)}
		</div>
	);
}

function GdtfLimitations({ pending }: { pending: PendingGdtfImport }) {
	if (!pending.diagnostics.length) return null;
	return (
		<section className="import-limitations">
			<p className="import-warning-summary">
				<span>
					{pending.diagnostics.length} import{" "}
					{pending.diagnostics.length === 1 ? "limitation" : "limitations"}.
					Review the source details before importing.
				</span>
				<span>
					<strong>{pending.diagnostics[0].node}</strong>:{" "}
					{pending.diagnostics[0].message}
				</span>
			</p>
			<details className="gdtf-import-diagnostics">
				<summary>
					Import limitations — show all {pending.diagnostics.length}
				</summary>
				<p>
					Attribute mappings do not certify physical measurements or
					calibration.
				</p>
				<ul>
					{pending.diagnostics.map((item, index) => (
						<li key={`${item.node}:${index}`}>
							<strong>{item.node}</strong>: {item.message}
						</li>
					))}
				</ul>
			</details>
		</section>
	);
}

function GdtfBusyProgress({
	readingFile,
	pending,
}: {
	readingFile: boolean;
	pending: boolean;
}) {
	return (
		<OperationBusyOverlay
			title={
				readingFile
					? "Loading selected GDTF file…"
					: pending
						? "Importing fixture…"
						: "Reading GDTF…"
			}
			message={
				readingFile
					? "Reading the selected GDTF file…"
					: pending
						? "Saving fixture and mappings…"
						: "Preparing fixture modes and attribute mappings…"
			}
		/>
	);
}

function GdtfImportFooter({
	pendingGdtf,
	busy,
	error,
	confirmGdtfMappings,
	requirements,
	readingFile,
	unresolved,
	requestClose,
}: Pick<
	Props,
	"pendingGdtf" | "busy" | "error" | "confirmGdtfMappings" | "requirements"
> & { readingFile: boolean; unresolved: number; requestClose: () => void }) {
	const operationBusy = busy || readingFile;
	const summary =
		error?.split(/\r?\n/).find((line) => line.trim()) ?? "GDTF import failed.";
	return (
		<div className="import-workflow__footer">
			{error && !operationBusy ? (
				<ErrorAlert as="p" copyText={error}>
					<strong>
						{pendingGdtf
							? "Import failed. Source and mappings are retained."
							: "GDTF preview failed."}
					</strong>
					<br />
					{summary}
				</ErrorAlert>
			) : (
				<p>
					{readingFile
						? "Reading the selected archive…"
						: busy
							? pendingGdtf
								? "Importing fixture…"
								: "Reading GDTF…"
							: unresolved > 0
								? `Choose a destination for ${unresolved} remaining ${unresolved === 1 ? "attribute" : "attributes"} to enable import.`
								: pendingGdtf
									? "Ready to import. Review any import limitations."
									: "Choose a GDTF archive to preview."}
				</p>
			)}
			<Button disabled={operationBusy} onClick={requestClose}>
				Cancel
			</Button>
			{pendingGdtf && (
				<Button
					variant="primary"
					disabled={operationBusy || unresolved > 0}
					onClick={() => void confirmGdtfMappings()}
				>
					{busy
						? "Importing…"
						: error
							? "Retry import"
							: requirements.length
								? "Import and remember mappings"
								: "Import fixture"}
				</Button>
			)}
		</div>
	);
}

export function GdtfImportDialog({
	pendingGdtf,
	busy,
	error,
	close,
	confirmGdtfMappings,
	importGdtfFile,
	mappings,
	requirements,
	children,
}: Props) {
	const decisions = useRef<HTMLDivElement>(null);
	const [readingFile, setReadingFile] = useState(false);
	const readingRef = useRef(false);
	const readBusyChanged = (value: boolean) => {
		readingRef.current = value;
		setReadingFile(value);
	};
	const operationBusy = busy || readingFile;
	const requestClose = () => {
		if (!busy && !readingRef.current) close();
	};
	const unresolved = requirements.filter(
		(requirement) => !mappings[requirement.attribute],
	).length;
	const nextUnresolved = () => {
		const trigger = decisions.current?.querySelector<HTMLButtonElement>(
			'[data-import-unresolved="true"] .ui-select-trigger',
		);
		trigger?.scrollIntoView({ block: "center" });
		trigger?.focus({ preventScroll: true });
	};
	return (
		<ModalRegistration onClose={requestClose}>
			<div className="stacked-modal-layer">
				<section
					className="nested-modal gdtf-import-modal import-workflow"
					role="dialog"
					aria-modal="true"
					aria-label="Import GDTF"
				>
					<div className="import-workflow__header">
						<ModalTitleBar
							title="Import GDTF"
							closeLabel="Close Import GDTF"
							closeDisabled={operationBusy}
							onClose={requestClose}
						/>
						{pendingGdtf && <GdtfSourceSummary pending={pendingGdtf} />}
					</div>
					<div className="import-workflow__body">
						{!pendingGdtf ? (
							<>
								<p>
									Select a GDTF archive. Every DMX mode will be imported into
									the desk-wide fixture library.
								</p>
								<RootConfinedFilePickerButton
									variant="primary"
									disabled={operationBusy}
									label={busy ? "Reading GDTF…" : "Choose GDTF file"}
									allowedExtensions={["gdtf"]}
									onReadBusyChange={readBusyChanged}
									onFiles={(files) => importGdtfFile(files[0])}
								/>
							</>
						) : (
							<>
								<div className="import-workflow__decisions" ref={decisions}>
									<div className="import-decision-summary">
										<h3>
											{requirements.length
												? "Required attribute mappings"
												: "No attribute mappings required"}
										</h3>
										<p role="status">
											{unresolved} unresolved of {requirements.length} required
											mappings
										</p>
										{unresolved > 0 && (
											<Button disabled={operationBusy} onClick={nextUnresolved}>
												Next unresolved
											</Button>
										)}
									</div>
									{requirements.length > 0 && (
										<p>
											Choose a compatible destination for each exact GDTF source
											attribute. Existing mappings remain editable. These
											choices are remembered for later GDTF imports on this
											desk.
										</p>
									)}
									{children}
								</div>
								<GdtfLimitations pending={pendingGdtf} />
							</>
						)}
					</div>
					<GdtfImportFooter
						pendingGdtf={pendingGdtf}
						busy={busy}
						error={error}
						confirmGdtfMappings={confirmGdtfMappings}
						requirements={requirements}
						readingFile={readingFile}
						unresolved={unresolved}
						requestClose={requestClose}
					/>
					{operationBusy && (
						<GdtfBusyProgress
							readingFile={readingFile}
							pending={Boolean(pendingGdtf)}
						/>
					)}
				</section>
			</div>
		</ModalRegistration>
	);
}
