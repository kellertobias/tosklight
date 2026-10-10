import { CheckboxField, TextInput } from "@tosklight/ui";
import { useId } from "react";
import type { QuickSetupModel } from "./QuickSetupModal";

export function MvrRequiredDecisions({ model }: { model: QuickSetupModel }) {
	const nameId = useId();
	const mvr = model.mvr;
	const conflicts = mvr.mvrPreview?.profile_conflicts ?? [];
	return (
		<section
			className="import-workflow__decisions"
			aria-label="Required import decisions"
		>
			{mvr.mvrMode === "new" && (
				<div data-import-unresolved={!mvr.mvrName.trim()}>
					<label htmlFor={nameId}>Destination show name</label>

					<TextInput
						id={nameId}
						clearable
						value={mvr.mvrName}
						onChange={(event) => mvr.setMvrName(event.target.value)}
						placeholder="Show name"
						aria-label="Show name"
					/>
				</div>
			)}
			{conflicts.length > 0 && (
				<section
					data-import-unresolved={!mvr.copyConflictingProfiles}
					aria-label="Immutable profile collision decision"
				>
					<h3>Required: keep conflicting profiles as independent copies</h3>
					<p>
						These immutable fixture profiles differ from this computer's
						existing revisions:
					</p>
					<ul>
						{conflicts.map((conflict) => (
							<li key={`${conflict.profile_id}:${conflict.revision}`}>
								{conflict.name} · revision {conflict.revision} ·{" "}
								{conflict.fixtures.length} fixtures
							</li>
						))}
					</ul>
					<CheckboxField
						label="Import conflicting profiles as new identities"
						checked={mvr.copyConflictingProfiles}
						onChange={(event) =>
							mvr.setCopyConflictingProfiles(event.target.checked)
						}
						disabled={mvr.mvrBusy}
					/>
					<p>
						The exact archive profiles are copied. Existing profiles and
						unrelated fixtures stay unchanged. Identity-bound installed
						calibration is retained and becomes inactive until revalidated for
						the new profile identity. Retained GDTF source evidence keeps its
						original association; export generates GDTF if it no longer matches.
					</p>
				</section>
			)}
		</section>
	);
}

export function MvrImportLimitations({ model }: { model: QuickSetupModel }) {
	const preview = model.mvr.mvrPreview;
	if (!preview) return null;
	const count =
		preview.warnings.length +
		preview.address_conflicts.length +
		preview.missing_profiles.length;
	if (!count) return null;
	return (
		<section className="import-limitations" aria-label="Import limitations">
			<p className="import-warning-summary">
				{preview.missing_profiles.length > 0
					? `${preview.missing_profiles.length} fixture profiles will be imported as unresolved. They remain visible but do not output DMX; verify their mapping in Fixture Library.`
					: "Review archive limitations before importing. Address conflicts keep your selected resolution."}
			</p>
			<details>
				<summary>All import warnings and limitations ({count})</summary>
				<ul>
					{preview.missing_profiles.map((message, index) => (
						<li key={`profile-${index}-${message}`}>
							Missing profile: {message}
						</li>
					))}
					{preview.address_conflicts.map((message, index) => (
						<li key={`address-${index}-${message}`}>{message}</li>
					))}
					{preview.warnings.map((message, index) => (
						<li key={`warning-${index}-${message}`}>{message}</li>
					))}
				</ul>
			</details>
		</section>
	);
}
