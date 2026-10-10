import { Button, ErrorAlert } from "@tosklight/ui";
import { useRef } from "react";
import {
	MvrImportLimitations,
	MvrRequiredDecisions,
} from "./MvrImportDecisions";
import { MvrFixtureRow } from "./MvrImportRows";
import type { QuickSetupModel } from "./QuickSetupModal";

export function MvrImportPreview({ model }: { model: QuickSetupModel }) {
	const body = useRef<HTMLDivElement>(null);
	const mvr = model.mvr;
	const preview = mvr.mvrPreview;
	if (!preview) return null;
	const needsName = mvr.mvrMode === "new" && !mvr.mvrName.trim();
	const needsConsent =
		(preview.profile_conflicts?.length ?? 0) > 0 &&
		!mvr.copyConflictingProfiles;
	const remaining = Number(needsName) + Number(needsConsent);
	const total =
		Number(mvr.mvrMode === "new") +
		Number((preview.profile_conflicts?.length ?? 0) > 0);
	const reason = mvr.mvrBusy
		? "Wait for the current MVR operation to finish."
		: needsName
			? "Enter a destination show name to continue."
			: needsConsent
				? "Confirm independent copies of conflicting immutable profiles to continue."
				: "";
	const nextUnresolved = () => {
		const row = body.current?.querySelector<HTMLElement>(
			'[data-import-unresolved="true"]',
		);
		row?.scrollIntoView?.({ block: "center", behavior: "smooth" });
		row
			?.querySelector<HTMLElement>(
				"input:not([disabled]), button:not([disabled]), select:not([disabled])",
			)
			?.focus();
	};
	return (
		<>
			<section
				className="import-workflow__summary"
				aria-label="MVR source and destination"
			>
				<b>
					{mvr.mvrInspectionFile?.name ?? "Inspected MVR archive"} →{" "}
					{mvr.mvrMode === "new"
						? mvr.mvrName.trim() || "New show · name required"
						: (mvr.mvrTarget?.name ?? "Select a destination show")}
				</b>
				<p>
					{preview.fixtures.length} fixtures · {preview.scenery} scenery objects
				</p>
				<div className="import-decision-summary" role="status">
					<strong>
						{remaining} required decisions remaining · {total} total
					</strong>
					<Button disabled={!remaining || mvr.mvrBusy} onClick={nextUnresolved}>
						Next unresolved
					</Button>
				</div>
			</section>
			<div className="import-workflow__body" ref={body}>
				{mvr.mvrError && <ErrorAlert role="alert">{mvr.mvrError}</ErrorAlert>}
				<MvrRequiredDecisions model={model} />
				<section aria-label="Fixture address resolutions">
					<h3>Fixture destinations</h3>
					{preview.address_conflicts.length > 0 && (
						<p>
							Address conflicts default to Import unpatched. This is a completed
							resolution; choose another destination only if needed.
						</p>
					)}
					<div
						className="import-mapping-row import-mapping-row--heading"
						aria-hidden="true"
					>
						<b>Source fixture</b>
						<b>Destination / resolution</b>
					</div>
					{preview.fixtures.map((fixture) => (
						<MvrFixtureRow key={fixture.uuid} fixture={fixture} model={model} />
					))}
				</section>
				<MvrImportLimitations model={model} />
			</div>
			<footer className="import-workflow__footer">
				{reason && <p role="status">{reason}</p>}
				<Button
					disabled={mvr.mvrOperation === "apply"}
					onClick={() => mvr.setMvrMode(null)}
				>
					Cancel
				</Button>
				<Button
					variant="primary"
					disabled={mvr.mvrBusy || remaining > 0}
					onClick={() => void mvr.applyMvr()}
				>
					{mvr.mvrBusy
						? "Importing…"
						: mvr.mvrMode === "new"
							? "Create and Open Show"
							: `Add to ${mvr.mvrTarget?.name}`}
				</Button>
			</footer>
		</>
	);
}
