import {
	Button,
	ErrorAlert,
	ModalRegistration,
	ModalTitleBar,
} from "@tosklight/ui";
import { type ReactNode, useRef } from "react";
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
		<ModalRegistration onClose={close}>
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
							onClose={close}
						/>
						{pendingGdtf && <GdtfSourceSummary pending={pendingGdtf} />}
					</div>
					<div className="import-workflow__body">
						{error && (
							<ErrorAlert as="p" role="alert">
								{error}
							</ErrorAlert>
						)}
						{!pendingGdtf ? (
							<>
								<p>
									Select a GDTF archive. Every DMX mode will be imported into
									the desk-wide fixture library.
								</p>
								<RootConfinedFilePickerButton
									variant="primary"
									disabled={busy}
									label={busy ? "Reading GDTF…" : "Choose GDTF file"}
									allowedExtensions={["gdtf"]}
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
											<Button disabled={busy} onClick={nextUnresolved}>
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
					<div className="import-workflow__footer">
						<p>
							{busy
								? pendingGdtf
									? "Importing fixture…"
									: "Reading GDTF…"
								: unresolved > 0
									? `Choose a destination for ${unresolved} remaining ${unresolved === 1 ? "attribute" : "attributes"} to enable import.`
									: pendingGdtf
										? "Ready to import. Review any import limitations."
										: "Choose a GDTF archive to preview."}
						</p>
						<Button onClick={close}>Cancel</Button>
						{pendingGdtf && (
							<Button
								variant="primary"
								disabled={busy || unresolved > 0}
								onClick={() => void confirmGdtfMappings()}
							>
								{busy
									? "Importing…"
									: requirements.length
										? "Import and remember mappings"
										: "Import fixture"}
							</Button>
						)}
					</div>
				</section>
			</div>
		</ModalRegistration>
	);
}
