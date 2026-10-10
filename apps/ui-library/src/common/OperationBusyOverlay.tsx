import { useEffect, useRef, useState } from "react";
import { Button } from "./controls/foundation";

export interface OperationBusyOverlayProps {
	title: string;
	message: string;
	source?: string;
	startedAt?: number;
	label?: string;
	onCancel?: () => void;
	cancelLabel?: string;
}

/** One active operation inside its existing registered workflow modal, never a second stack. */
export function OperationBusyOverlay({
	title,
	message,
	source,
	startedAt,
	label = title,
	onCancel,
	cancelLabel = "Cancel",
}: OperationBusyOverlayProps) {
	const panel = useRef<HTMLElement>(null);
	const cancel = useRef<HTMLButtonElement>(null);
	const [started] = useState(() => startedAt ?? Date.now());
	const [now, setNow] = useState(Date.now());
	useEffect(() => {
		const previous =
			document.activeElement instanceof HTMLElement
				? document.activeElement
				: null;
		const dialog = panel.current?.closest("[role='dialog'], .nested-modal");
		const siblings: HTMLElement[] = [];
		let branch = panel.current?.parentElement;
		while (dialog && branch && branch !== dialog) {
			const parent = branch.parentElement;
			if (!parent) break;
			for (const sibling of parent.children) {
				if (sibling instanceof HTMLElement && sibling !== branch)
					siblings.push(sibling);
			}
			branch = parent;
		}
		const prior = siblings.map((element) => ({
			element,
			inert: Boolean(element.inert || element.hasAttribute("inert")),
			attribute: element.getAttribute("inert"),
		}));
		for (const { element } of prior) element.inert = true;
		const priorBusy = dialog?.getAttribute("aria-busy");
		dialog?.setAttribute("aria-busy", "true");
		panel.current?.focus();
		return () => {
			for (const { element, inert, attribute } of prior) {
				element.inert = inert;
				if (attribute === null) element.removeAttribute("inert");
				else element.setAttribute("inert", attribute);
			}
			if (priorBusy === null || priorBusy === undefined)
				dialog?.removeAttribute("aria-busy");
			else dialog?.setAttribute("aria-busy", priorBusy);
			if (previous?.isConnected) previous.focus();
		};
	}, []);
	useEffect(() => {
		const timer = globalThis.setInterval(() => setNow(Date.now()), 1000);
		return () => globalThis.clearInterval(timer);
	}, []);
	return (
		<div className="ui-operation-busy-overlay">
			<section
				ref={panel}
				tabIndex={-1}
				className="ui-operation-busy-panel"
				role="status"
				aria-label={label}
				aria-busy="true"
				onKeyDown={(event) => {
					if (event.key === "Tab") {
						event.preventDefault();
						(cancel.current ?? panel.current)?.focus();
					}
				}}
			>
				<span className="ui-spinner" aria-hidden="true" />
				<h2>{title}</h2>

				<progress aria-label={label} />
				<p>{message}</p>
				<p aria-live="off">
					{source ? `${source} · ` : ""}Elapsed{" "}
					{Math.max(0, Math.floor((now - (startedAt ?? started)) / 1000))} s
				</p>
				{onCancel && (
					<Button ref={cancel} onClick={onCancel}>
						{cancelLabel}
					</Button>
				)}
			</section>
		</div>
	);
}
