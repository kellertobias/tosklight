import { ModalFrame } from "@tosklight/ui/modals";
import type { Cue } from "../../api/types";
import type { useCuelistPreviews } from "./useCuelistPreviews";

type Picture = NonNullable<
	ReturnType<ReturnType<typeof useCuelistPreviews>["pictureAt"]>
>;

export function CuePreviewModal({
	cue,
	picture,
	onClose,
}: {
	cue: Cue;
	picture: Picture;
	onClose: () => void;
}) {
	return (
		<ModalFrame
			id={`cue-preview-${cue.id ?? cue.number}`}
			ariaLabel={`Cue ${cue.number} preview image`}
			title={`Cue ${cue.number}${cue.name ? ` · ${cue.name}` : ""}`}
			closeLabel="Close Cue preview"
			dialogClassName="cuelist-preview-modal"
			onClose={onClose}
		>
			<div
				className={`cuelist-preview-modal-body ${picture.scope ? `scope-${picture.scope}` : ""}`.trim()}
			>
				{picture.notice && <p role="status">{picture.notice}</p>}
				<img src={picture.src} alt={`Cue ${cue.number} preview`} />
			</div>
		</ModalFrame>
	);
}
