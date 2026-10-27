import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import { CadProjectPanel } from "./cad/CadProjectPanel";
import { documentSession, type DocumentSummary } from "./document/session";
import { usePaperworkAutosave } from "./document/usePaperworkAutosave";

const LOGO_FILTER = [{ name: "Logo image", extensions: ["png", "jpg", "jpeg", "webp"] }];

/** Project information writes to the active native show as the operator edits it. */
function LoadedPaperwork({ document, onDocument, onError }: {
	document: DocumentSummary;
	onDocument(document: DocumentSummary): void;
	onError(reason: unknown): void;
}) {
	const { draft, change, saving, error, retry } = usePaperworkAutosave(document, onDocument);
	const [status, setStatus] = useState("");
	const alive = useRef(true);
	const logoRequest = useRef(0);
	useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
	return (
		<>
			<CadProjectPanel
				paperwork={draft} documentInfo={document} saving={saving} error={error}
				status={status} onRetry={retry} onChange={change}
				onUploadLogo={async () => {
					const request = ++logoRequest.current;
					try {
						const path = await open({ multiple: false, filters: LOGO_FILTER });
						if (typeof path !== "string" || !alive.current || request !== logoRequest.current) return;
						setStatus("Reading logo…");
						const logo = await documentSession.readCompanyLogo(path);
						if (!alive.current || request !== logoRequest.current) return;
						change("companyLogo", JSON.stringify(logo));
						setStatus("");
					} catch (reason) {
						if (!alive.current || request !== logoRequest.current) return;
						setStatus("");
						onError(reason);
					}
				}}
				onRemoveLogo={() => {
					logoRequest.current += 1;
					change("companyLogo", "");
					setStatus("");
				}}
				onMakeDefault={async () => {
					try {
						await documentSession.saveLightingDesignerDefault({
							lightingDesigner: draft.lightingDesigner, contactPhone: draft.contactPhone,
							contactEmail: draft.contactEmail, companyLogo: draft.companyLogo,
						});
						if (alive.current) setStatus("Saved as the lighting designer for new shows on this computer.");
					} catch (reason) { if (alive.current) onError(reason); }
				}}
			/>
		</>
	);
}


export function ShowPaperwork({ document, onDocument, onError }: {
	document: DocumentSummary | null;
	onDocument(document: DocumentSummary): void;
	onError(reason: unknown): void;
}) {
	return <section className="viz-show-paperwork" aria-label="Show information">
		{document ? <LoadedPaperwork key={document.showId} document={document} onDocument={onDocument} onError={onError} />
			: <p>Open or create a show to describe it.</p>}
	</section>;
}
