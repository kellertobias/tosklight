import { Button } from "@tosklight/ui";
import { useState } from "react";
import {
	documentSession,
	type MvrPreview,
	type MvrPreviewFixture,
	type MvrResolution,
} from "./document/session";

/**
 * What an MVR archive brings, and what to do about the fixtures that need a decision.
 *
 * An archive written by another application does not always land cleanly: an unresolved GDTF
 * cannot be patched, and an address already in use cannot simply be taken. The desk
 * asks the operator about exactly those two cases before it writes anything, and so does this. A
 * fixture nothing is wrong with needs no decision and gets none.
 */
export function MvrImport({
	preview,
	onImported,
	onCancel,
	onError,
}: {
	preview: MvrPreview;
	onImported: (summary: string) => void;
	onCancel: () => void;
	onError: (reason: unknown) => void;
}) {
	const [resolutions, setResolutions] = useState<Record<string, MvrResolution>>(
		() => defaultResolutions(preview),
	);
	const [busy, setBusy] = useState(false);
	const [completed, setCompleted] = useState<{
		summary: string;
		warnings: string[];
	} | null>(null);
	const decide = (fixture: MvrPreviewFixture, resolution: MvrResolution) =>
		setResolutions((current) => ({ ...current, [fixture.uuid]: resolution }));

	const undecided = preview.fixtures.filter(needsDecision);

	async function apply() {
		setBusy(true);
		try {
			const report = await documentSession.importMvr(preview.token, resolutions);
			const unresolved = report.unresolvedFixtures
				? `, ${report.unresolvedFixtures} unresolved`
				: "";
			const summary = `Imported ${report.importedFixtures} fixtures${unresolved}`;
			if (report.warnings.length) {
				setCompleted({ summary, warnings: report.warnings });
			} else {
				onImported(summary);
			}
		} catch (reason) {
			onError(reason);
		} finally {
			setBusy(false);
		}
	}

	async function cancel() {
		setBusy(true);
		try {
			await documentSession.cancelMvrPreview(preview.token);
			onCancel();
		} catch (reason) {
			onError(reason);
		} finally {
			setBusy(false);
		}
	}

	if (completed) {
		return (
			<section className="viz-editor-mvr" aria-label="MVR import result">
				<h2>{completed.summary}</h2>
				<ImportWarnings warnings={completed.warnings} />
				<Button variant="primary" onClick={() => onImported(completed.summary)}>
					Done
				</Button>
			</section>
		);
	}

	return (
		<section className="viz-editor-mvr" aria-label="MVR import">
			<h2>Import MVR</h2>
			<p className="viz-editor-mvr-summary">
				{preview.fixtures.length} fixtures, {preview.scenery} scenery objects.
				{preview.missingProfiles.length
					? ` ${preview.missingProfiles.length} GDTF types have no matching profile.`
					: ""}
				{preview.addressConflicts.length
					? ` ${preview.addressConflicts.length} address conflicts.`
					: ""}
			</p>
			{preview.missingProfiles.length > 0 && (
				<ul className="viz-editor-mvr-missing">
					{preview.missingProfiles.map((profile) => (
						<li key={profile}>{profile}</li>
					))}
				</ul>
			)}
			<ImportWarnings warnings={preview.warnings} />
			{undecided.length === 0 ? (
				<p className="viz-editor-mvr-clean">
					Every fixture patches as it stands.
				</p>
			) : (
				<ul className="viz-editor-mvr-fixtures">
					{undecided.map((fixture) => (
						<FixtureDecision
							key={fixture.uuid}
							fixture={fixture}
							resolution={resolutions[fixture.uuid]}
							onDecide={(resolution) => decide(fixture, resolution)}
						/>
					))}
				</ul>
			)}
			<div className="viz-editor-mvr-actions">
				<Button variant="primary" disabled={busy} onClick={() => void apply()}>
					{busy ? "Importing…" : "Import"}
				</Button>
				<Button disabled={busy} onClick={() => void cancel()}>
					Cancel
				</Button>
			</div>
		</section>
	);
}

/** A fixture the operator has to decide about: no profile matches it, or its address is taken. */
function needsDecision(fixture: MvrPreviewFixture) {
	return fixture.conflicted || !fixture.matched;
}

/**
 * The decision each troubled fixture starts with.
 *
 * An unmatched GDTF stays as an unresolved import record with its retained source evidence;
 * it cannot become a programmable fixture until its profile is resolved. A conflicting address is also imported
 * unpatched rather than taking an address a patched fixture already holds.
 */
function defaultResolutions(preview: MvrPreview) {
	const decisions: Record<string, MvrResolution> = {};
	for (const fixture of preview.fixtures) {
		if (needsDecision(fixture)) {
			decisions[fixture.uuid] = { action: "import_unpatched" };
		}
	}
	return decisions;
}

function FixtureDecision({
	fixture,
	resolution,
	onDecide,
}: {
	fixture: MvrPreviewFixture;
	resolution: MvrResolution | undefined;
	onDecide: (resolution: MvrResolution) => void;
}) {
	const action = resolution?.action ?? "import_unpatched";
	const reason = !fixture.matched
		? "No matching profile"
		: "Address already patched";
	return (
		<li className="viz-editor-mvr-fixture">
			<span>
				<b>{fixture.name}</b>
				<small>
					{fixture.gdtfSpec} · {fixture.gdtfMode} ·{" "}
					{fixture.universe && fixture.address
						? `U${fixture.universe}.${fixture.address}`
						: "Unpatched"}{" "}
					· {reason}
				</small>
			</span>
			<label>
				<span className="viz-editor-mvr-label">
					Resolution for {fixture.name}
				</span>
				<select
					value={action}
					aria-label={`Resolution for ${fixture.name}`}
					onChange={(event) =>
						onDecide({
							action: event.target.value as MvrResolution["action"],
							universe: fixture.universe ?? 1,
							address: fixture.address ?? 1,
						})
					}
				>
					<option value="import_unpatched">
						{fixture.matched ? "Import unpatched" : "Keep unresolved"}
					</option>
					{fixture.matched && <option value="address">Choose address</option>}
					<option value="skip">Skip</option>
					{fixture.conflicted && <option value="replace">Replace</option>}
				</select>
			</label>
			{action === "address" && (
				<span className="viz-editor-mvr-address">
					<label>
						Universe
						<input
							type="number"
							min={1}
							max={65535}
							aria-label={`Universe for ${fixture.name}`}
							value={resolution?.universe ?? 1}
							onChange={(event) =>
								onDecide({
									action: "address",
									universe: Number(event.target.value),
									address: resolution?.address ?? 1,
								})
							}
						/>
					</label>
					<label>
						Address
						<input
							type="number"
							min={1}
							max={512}
							aria-label={`Address for ${fixture.name}`}
							value={resolution?.address ?? 1}
							onChange={(event) =>
								onDecide({
									action: "address",
									universe: resolution?.universe ?? 1,
									address: Number(event.target.value),
								})
							}
						/>
					</label>
				</span>
			)}
		</li>
	);
}

function ImportWarnings({ warnings }: { warnings: string[] }) {
	if (!warnings.length) return null;
	return (
		<section aria-label="Import limitations">
			<h3>Import limitations</h3>
			<ul>
				{warnings.map((warning, index) => (
					<li key={`${index}:${warning}`}>{warning}</li>
				))}
			</ul>
		</section>
	);
}
