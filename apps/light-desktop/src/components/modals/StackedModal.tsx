import { ModalRegistration } from "@tosklight/ui";
import type { PropsWithChildren } from "react";
import { createPortal } from "react-dom";

export function StackedModal({
	children,
	onClose,
}: PropsWithChildren<{ onClose: () => void }>) {
	return createPortal(
		<ModalRegistration onClose={onClose}>
			<div
				className="stacked-modal-layer"
				onPointerDown={(event) => {
					if (event.target === event.currentTarget) onClose();
				}}
			>
				{children}
			</div>
		</ModalRegistration>,
		document.body,
	);
}
