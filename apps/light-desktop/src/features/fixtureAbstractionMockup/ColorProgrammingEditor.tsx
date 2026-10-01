import { Button, ModalFrame } from "@tosklight/ui";
import { useEffect, useRef, useState, type ReactNode } from "react";

/** The compact pages and full modal edit the same local color recipe. */
export function ColorProgrammingEditor({ fits, page, onPage, onClose, picker, expandedPicker, blend, balance, comparison, preview }: {
	fits: boolean; page: string; onPage(page: string): void; onClose(): void;
	picker: ReactNode; expandedPicker: ReactNode; blend: ReactNode; balance: ReactNode; comparison: ReactNode; preview?: ReactNode;
}) {
	const [expanded, setExpanded] = useState(false);
	const modal = expanded || !fits;
	const inline = useRef<HTMLElement>(null);
	const nextPage = page === "mix" ? "white" : "mix";
	const nextLabel = nextPage === "mix" ? "Color" : preview ? "Preview" : "White balance";
	useEffect(() => {
		if (modal) return;
		const escape = (event: KeyboardEvent) => {
			if (event.key !== "Escape" || event.defaultPrevented || document.querySelector('.ui-modal-stack-layer[data-modal-top="true"]')) return;
			event.preventDefault(); onClose();
		};
		document.addEventListener("keydown", escape);
		return () => document.removeEventListener("keydown", escape);
	}, [modal, onClose]);
	useEffect(() => {
		if (!modal) inline.current?.querySelector<HTMLElement>('[role="application"], [role="slider"], input, button')?.focus();
	}, [modal, page]);
	if (modal) return <ModalFrame title={preview ? "Media color" : "Color"} ariaLabel="Color Special Dialog"
		closeLabel="Close Special Dialog" onClose={onClose} className="fixture-abstraction-layer"
		dialogClassName="fixture-abstraction-panel fam-full-color-modal">
		<div className="fam-full-color-layout" data-testid="editor-page">
			<section className="fam-full-picker" aria-label="Color controls" data-testid="full-color-editor">{expandedPicker}</section>
			{preview ? <section className="fam-full-media-preview" aria-label="Media preview">{preview}</section>
				: <section className="fam-full-color-comparison" aria-label="Color approximation"><h3>Color approximation</h3>{comparison}</section>}
		</div>
	</ModalFrame>;
	return <section ref={inline} className="fixture-abstraction-panel fam-inline-dialog fam-color-editor" role="dialog" aria-label="Color Special Dialog">
		<div className={`fam-editor-page ${page === "mix" ? "fam-editor-mix" : preview ? "fam-editor-preview" : "fam-editor-white"}`} data-testid="editor-page">
			{page === "mix" ? picker : preview ?? balance}
			<div className="fam-compact-color-actions">
				{page === "mix" && blend}
				<div className="fam-editor-buttons">
					<Button aria-label={`Switch to ${nextLabel}`} onClick={() => onPage(nextPage)}>{nextLabel}</Button>
					<Button onClick={() => setExpanded(true)}>Expand</Button>
				</div>
			</div>
		</div>
	</section>;
}
