import { ModalRegistration, ModalTitleBar } from "@tosklight/ui";
import { createPortal } from "react-dom";
import type { SpeedGroupId } from "../../api/types";
import type { SoundToLightController } from "./useSoundToLight";

export function SoundToLightLoading({
	group,
	controller,
	onClose,
}: {
	group: SpeedGroupId;
	controller: SoundToLightController;
	onClose: () => void;
}) {
	return createPortal(
		<ModalRegistration onClose={onClose}>
			<div
				className="stacked-modal-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && onClose()
				}
			>
				<section
					className="nested-modal"
					role="dialog"
					aria-modal="true"
					aria-label={`Speed Group ${group} Sound to Light`}
				>
					<ModalTitleBar
						title={`Speed Group ${group} · Sound to Light`}
						closeLabel="Close Sound-to-Light configuration"
						onClose={onClose}
					/>
					<p>
						{controller.loading
							? "Loading Speed Group configuration…"
							: (controller.error ??
								"Speed Group configuration is not available.")}
					</p>
				</section>
			</div>
		</ModalRegistration>,
		document.body,
	);
}
