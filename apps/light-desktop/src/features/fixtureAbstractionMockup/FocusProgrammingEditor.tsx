import { FocusZoomDialog } from "../../components/modals/specialDialogs/intention/FocusZoomDialog";

export interface FocusProgrammingEditorProps {
	fits: boolean;
	zoom: number;
	focus: number;
	onZoom(value: number): void;
	onFocus(value: number): void;
	onClose(): void;
}

/** Deterministic demo descriptors for the mockup only; production supplies the fixture's own. */
const DEMO_ZOOM = { minimum: 8, maximum: 48, step: .1, keyStep: 1, largeKeyStep: 5, convention: "beam" } as const;
const DEMO_FOCUS = { minimum: 0, maximum: 1, step: .01, keyStep: .01, largeKeyStep: .05 } as const;

/** Mockup adapter: its local state keeps Focus as 0–100, the dialog takes a normalized 0–1 setting. */
export function FocusProgrammingEditor({ zoom, focus, onZoom, onFocus, onClose }: FocusProgrammingEditorProps) {
	return <FocusZoomDialog zoom={{ ...DEMO_ZOOM, value: zoom }} focus={{ ...DEMO_FOCUS, value: focus / 100 }}
		onZoomChange={onZoom} onFocusChange={value => onFocus(Math.round(value * 100))} onClose={onClose} />;
}
