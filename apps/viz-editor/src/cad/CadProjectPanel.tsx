import { Button } from "@tosklight/ui";
import { companyLogoUrl, parseCompanyLogo } from "../document/companyLogo";
import type { DocumentSummary } from "../document/session";

export interface CadPaperwork {
	project: string;
	lightingDesigner: string;
	venue: string;
	contactEmail: string;
	contactPhone: string;
	showDate: string;
	showVersion: string;
	/** The company logo as the show stores it, or empty. */
	companyLogo: string;
}

type TextField = Exclude<keyof CadPaperwork, "companyLogo">;

function Field({
	label,
	ariaLabel,
	field,
	type = "text",
	paperwork,
	onChange,
}: {
	label: string;
	ariaLabel?: string;
	field: TextField;
	type?: "text" | "email" | "tel" | "date";
	paperwork: CadPaperwork;
	onChange(field: TextField, value: string): void;
}) {
	return (
		<label>
			{label}
			<input
				type={type}
				aria-label={ariaLabel}
				value={paperwork[field]}
				onChange={(event) => onChange(field, event.currentTarget.value)}
			/>
		</label>
	);
}

/**
 * The show's paperwork, which titles every printed page.
 *
 * It reads as two columns: the show itself on the left — project, venue, date and version — and the
 * lighting designer on the right — name, phone, email and the company logo. The designer can be made
 * this computer's default, so a show created here afterwards starts with them.
 */
export function CadProjectPanel({
	paperwork,
	documentInfo,
	saving,
	status,
	onChange,
	onSave,
	onUploadLogo,
	onRemoveLogo,
	onMakeDefault,
}: {
	paperwork: CadPaperwork;
	documentInfo: DocumentSummary | null;
	saving: boolean;
	/** What the last logo or default action did, so none of them is silent. */
	status?: string;
	onChange(field: TextField, value: string): void;
	onSave(): void;
	onUploadLogo(): void;
	onRemoveLogo(): void;
	onMakeDefault(): void;
}) {
	const logo = parseCompanyLogo(paperwork.companyLogo);
	const field = { paperwork, onChange };
	return (
		<section className="cad-print-project-info" aria-label="Project information">
			<h3>Project information</h3>
			<div className="cad-project-columns">
				<fieldset className="cad-project-show">
					<legend>Show</legend>
					<Field label="Project" field="project" {...field} />
					<Field label="Venue" field="venue" {...field} />
					<Field label="Show date" field="showDate" type="date" {...field} />
					<Field label="Version" ariaLabel="Show version" field="showVersion" {...field} />
				</fieldset>
				<fieldset className="cad-project-designer">
					<legend>Lighting designer</legend>
					<Field
						label="Name"
						ariaLabel="Lighting designer name"
						field="lightingDesigner"
						{...field}
					/>
					<Field
						label="Phone"
						ariaLabel="Lighting designer phone"
						field="contactPhone"
						type="tel"
						{...field}
					/>
					<Field
						label="Email"
						ariaLabel="Lighting designer email"
						field="contactEmail"
						type="email"
						{...field}
					/>
					<div className="cad-project-logo">
						<span>Company logo</span>
						{logo ? (
							<img src={companyLogoUrl(logo)} alt="Company logo" />
						) : (
							<p>No logo. The ToskLight mark prints in its place.</p>
						)}
						<div className="cad-project-logo-actions">
							<Button onClick={onUploadLogo}>{logo ? "Replace logo" : "Upload logo"}</Button>
							{logo ? <Button onClick={onRemoveLogo}>Remove logo</Button> : null}
						</div>
					</div>
					<Button className="cad-project-make-default" onClick={onMakeDefault}>
						Make Default
					</Button>
				</fieldset>
			</div>
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
			{status ? (
				<p className="cad-project-status" role="status">
					{status}
				</p>
			) : null}
			<Button disabled={saving} onClick={onSave}>
				{saving ? "Saving…" : "Save project info"}
			</Button>
		</section>
	);
}

function formatLastSaved(seconds?: number) {
	return seconds ? new Date(seconds * 1000).toLocaleString() : "—";
}
