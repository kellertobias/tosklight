import {
	Button,
	ErrorAlert,
	ModalRegistration,
	ModalTitleBar,
	TextField,
} from "@tosklight/ui";
import { PlaybackSoloSettings } from "../../modals/PlaybackSoloSettings";
import type { PlaybackGridSolo } from "./usePlaybackGridSolo";

export function PlaybackGridSoloControls({ solo }: { solo: PlaybackGridSolo }) {
	if (!solo.available) return null;
	return (
		<>
			<section
				className="playback-solo-toolbar"
				aria-label="Playback Solo configuration"
			>
				{solo.selecting ? (
					<>
						<p role="status">
							Select Playback members. These gestures do not operate controls.{" "}
							{solo.selected.length} selected across pages.
						</p>
						<TextField
							label="Solo Region name"
							value={solo.name}
							onChange={(event) => solo.setName(event.target.value)}
						/>
						<Button
							disabled={solo.selected.length < 2 || solo.saving}
							onClick={() => void solo.create()}
						>
							{solo.saving ? "Creating Solo Region…" : "Create Solo Region"}
						</Button>
						<Button disabled={solo.saving} onClick={solo.cancel}>
							Cancel Solo selection
						</Button>
					</>
				) : (
					<Button disabled={!solo.ready || solo.saving} onClick={solo.begin}>
						Select Solo members
					</Button>
				)}
				<Button
					disabled={!solo.ready || solo.saving}
					onClick={() => solo.setManaging(true)}
				>
					Manage Solo Regions
				</Button>
				{solo.error && <ErrorAlert role="alert">{solo.error}</ErrorAlert>}
			</section>
			{solo.managing && (
				<ModalRegistration onClose={() => solo.setManaging(false)}>
					<div className="stacked-modal-layer">
						<section
							className="nested-modal playback-solo-management"
							role="dialog"
							aria-modal="true"
							aria-label="Solo Regions"
						>
							<ModalTitleBar
								title="Solo Regions"
								closeLabel="Close Solo Regions"
								onClose={() => solo.setManaging(false)}
							/>
							<PlaybackSoloSettings />
						</section>
					</div>
				</ModalRegistration>
			)}
		</>
	);
}
