import { Button } from "@tosklight/ui";
import type { DocumentSummary } from "../document/session";

export interface CadPaperwork {
	project: string;
	lightingDesigner: string;
	venue: string;
	contactEmail: string;
	contactPhone: string;
	showDate: string;
	showVersion: string;
}

/**
 * The project paperwork that titles every printed page.
 *
 * It lives beside the print pages rather than above them: a shallow window could not show both
 * at once, and an operator setting up paperwork is not the same operator arranging pages.
 */
export function CadProjectPanel({
	paperwork,
	documentInfo,
	saving,
	onChange,
	onSave,
}: {
	paperwork: CadPaperwork;
	documentInfo: DocumentSummary | null;
	saving: boolean;
	onChange(field: keyof CadPaperwork, value: string): void;
	onSave(): void;
}) {
	return (
				<section
					className="cad-print-project-info"
					aria-label="Project information"
				>
					<h3>Project information</h3>
					<label>
						Project
						<input
							value={paperwork.project}
							onChange={(event) =>
								onChange("project", event.currentTarget.value)
							}
						/>
					</label>
					<label>
						Lighting designer
						<input
							value={paperwork.lightingDesigner}
							onChange={(event) =>
								onChange("lightingDesigner", event.currentTarget.value)
							}
						/>
					</label>
					<label>
						Venue
						<input
							value={paperwork.venue}
							onChange={(event) =>
								onChange("venue", event.currentTarget.value)
							}
						/>
					</label>
					<label>
						Contact email
						<input
							type="email"
							value={paperwork.contactEmail}
							onChange={(event) =>
								onChange("contactEmail", event.currentTarget.value)
							}
						/>
					</label>
					<label>
						Contact phone
						<input
							type="tel"
							value={paperwork.contactPhone}
							onChange={(event) =>
								onChange("contactPhone", event.currentTarget.value)
							}
						/>
					</label>
					<label>
						Show date
						<input
							type="date"
							value={paperwork.showDate}
							onChange={(event) =>
								onChange("showDate", event.currentTarget.value)
							}
						/>
					</label>
					<label>
						Show version
						<input
							value={paperwork.showVersion}
							onChange={(event) =>
								onChange("showVersion", event.currentTarget.value)
							}
						/>
					</label>
					<dl>
						<div>
							<dt>Show name</dt>
							<dd>{documentInfo?.name || "—"}</dd>
						</div>
						<div>
							<dt>Last saved</dt>
							<dd>{formatLastSaved(documentInfo?.lastSavedAt)}</dd>
						</div>
						<div>
							<dt>Fixtures</dt>
							<dd>{documentInfo?.fixtureCount ?? 0}</dd>
						</div>
						<div>
							<dt>Universes used</dt>
							<dd>{documentInfo?.universeCount ?? 0}</dd>
						</div>
					</dl>
					<Button
						disabled={saving}
						onClick={onSave}
					>
						{saving ? "Saving…" : "Save project info"}
					</Button>
				</section>
	);
}

function formatLastSaved(seconds?: number) {
	return seconds ? new Date(seconds * 1000).toLocaleString() : "—";
}
