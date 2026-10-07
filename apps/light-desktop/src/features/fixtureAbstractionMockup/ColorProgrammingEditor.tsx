import { useState, type ReactNode } from "react";
import { ColorDialogLayout } from "../../components/modals/specialDialogs/intention/ColorDialogLayout";

/** Thin mockup wrapper: the local story recipe supplies every slot; the shell owns only layout. */
export function ColorProgrammingEditor({ fits, page, onPage, onClose, picker, expandedPicker, blend, balance, comparison, preview }: {
	fits: boolean; page: string; onPage(page: string): void; onClose(): void;
	picker: ReactNode; expandedPicker: ReactNode; blend: ReactNode; balance: ReactNode; comparison: ReactNode; preview?: ReactNode;
}) {
	const [expanded, setExpanded] = useState(false);
	return <ColorDialogLayout fits={fits} page={page === "mix" ? "mix" : "white"} expanded={expanded}
		onPage={onPage} onExpand={() => setExpanded(true)} onClose={onClose}
		compactPicker={picker} whiteBlend={blend} whiteBalance={balance} expandedControls={expandedPicker}
		approximation={comparison} mediaPreview={preview}
		compactClassName="fixture-abstraction-panel fam-inline-dialog" modalClassName="fixture-abstraction-panel" layerClassName="fixture-abstraction-layer" />;
}
