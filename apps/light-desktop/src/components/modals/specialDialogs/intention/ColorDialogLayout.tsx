import { Button, ModalFrame } from "@tosklight/ui";
import { useEffect, useRef, type ReactNode } from "react";
import "./ColorDialogLayout.css";

/** Compact page one holds the 2D picker and White Blend; page two holds White balance or Media preview. */
export type ColorDialogPage = "mix" | "white";

export interface ColorDialogLayoutProps {
	/** The compact encoder-area budget is available. When false, the full modal is shown instead. */
	fits: boolean;
	/** Controlled compact page. */
	page: ColorDialogPage;
	/** Controlled expansion into the full modal. */
	expanded: boolean;
	onPage(page: ColorDialogPage): void;
	onExpand(): void;
	onClose(): void;
	/** Compact page one: supplied hue/saturation 2D picker. */
	compactPicker: ReactNode;
	/** Compact page one: supplied White Blend fader, placed above the page buttons. */
	whiteBlend: ReactNode;
	/** Compact page two: supplied Temperature and Duv content. */
	whiteBalance: ReactNode;
	/** Modal: supplied large picker with every color fader. */
	expandedControls: ReactNode;
	/** Modal: supplied passive per-fixture approximation. */
	approximation?: ReactNode;
	/** Modal (TL-554): supplied Direct section (reference head, overflow, Direct status). */
	native?: ReactNode;
	/** Media: replaces White balance on page two and the approximation in the modal. */
	mediaPreview?: ReactNode;
	/** Extra classes for the compact surface, the modal dialog and the modal layer. */
	compactClassName?: string;
	modalClassName?: string;
	layerClassName?: string;
}

const join = (...names: (string | undefined)[]) => names.filter(Boolean).join(" ");

/**
 * Presentation shell for the Color Special Dialog. It owns ordering, page navigation and the
 * compact/modal switch only; every control, value and approximation is supplied by the caller.
 */
export function ColorDialogLayout({
	fits, page, expanded, onPage, onExpand, onClose,
	compactPicker, whiteBlend, whiteBalance, expandedControls, approximation, native, mediaPreview,
	compactClassName, modalClassName, layerClassName,
}: ColorDialogLayoutProps) {
	const modal = expanded || !fits;
	const media = mediaPreview !== undefined && mediaPreview !== null;
	const compact = useRef<HTMLElement>(null);
	const nextPage: ColorDialogPage = page === "mix" ? "white" : "mix";
	const nextLabel = nextPage === "mix" ? "Color" : media ? "Preview" : "White balance";
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
		if (!modal) compact.current?.querySelector<HTMLElement>('[role="application"], [role="slider"], input, button')?.focus();
	}, [modal, page]);
	if (modal) return <ModalFrame title={media ? "Media color" : "Color"} ariaLabel="Color Special Dialog"
		closeLabel="Close Special Dialog" onClose={onClose} className={join("color-dialog-layer", layerClassName)}
		dialogClassName={join("color-dialog-modal", modalClassName)}>
		<div className="color-dialog-modal-body" data-testid="editor-page">
			<section className="color-dialog-controls" aria-label="Color controls" data-testid="full-color-editor">{expandedControls}</section>
			{media ? <section className="color-dialog-media-preview" aria-label="Media preview">{mediaPreview}</section>
				: approximation !== undefined && <section className="color-dialog-approximation" aria-label="Color approximation"><h3>Color approximation</h3>{approximation}</section>}
			{!media && native !== undefined && native !== null && <section className="color-dialog-native" aria-label="Direct color"><h3>Direct color</h3>{native}</section>}
		</div>
	</ModalFrame>;
	const shown = page === "mix" ? "mix" : media ? "preview" : "white";
	return <section ref={compact} className={join("color-dialog-compact", compactClassName)} role="dialog" aria-label="Color Special Dialog">
		<div className="color-dialog-compact-page" data-page={shown} data-testid="editor-page">
			{page === "mix" ? compactPicker : media ? mediaPreview : whiteBalance}
			<div className="color-dialog-actions">
				{page === "mix" && whiteBlend}
				<div className="color-dialog-buttons">
					<Button aria-label={`Switch to ${nextLabel}`} onClick={() => onPage(nextPage)}>{nextLabel}</Button>
					<Button onClick={onExpand}>Expand</Button>
				</div>
			</div>
		</div>
	</section>;
}
