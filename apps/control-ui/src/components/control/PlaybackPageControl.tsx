import { Button } from "../common";
import {
	PlaybackPageMenu,
	PlaybackPageRenameDialog,
} from "./PlaybackPageDialogs";
import { usePlaybackPageControl } from "./usePlaybackPageControl";

export function PlaybackPageControl() {
	const page = usePlaybackPageControl();
	return (
		<>
			<div className="playback-page-controls">
				<Button
					className="playback-page-chevron"
					aria-label="Previous playback page"
					disabled={
						page.busy || !page.ready || page.previousPageNumber == null
					}
					onClick={() => {
						if (page.previousPageNumber != null)
							void page.selectPage(page.previousPageNumber);
					}}
				>
					<svg viewBox="0 0 24 24" aria-hidden="true">
						<path d="m5 15 7-7 7 7" />
					</svg>
				</Button>
				<Button
					className="playback-page-current"
					aria-label={
						page.ready
							? `Select playback page. Page ${page.activePageNumber} ${page.currentPageName}`
							: "Playback page loading"
					}
					disabled={page.busy || !page.ready}
					onClick={page.openPageMenu}
				>
					<span>Page</span>
					<strong>{page.activePageNumber ?? "—"}</strong>
					<small>{page.currentPageName}</small>
				</Button>
				<Button
					className="playback-page-chevron"
					aria-label="Next playback page"
					disabled={page.busy || !page.canAdvance}
					onClick={() => void page.nextPage()}
				>
					<svg viewBox="0 0 24 24" aria-hidden="true">
						<path d="m5 9 7 7 7-7" />
					</svg>
				</Button>
			</div>
			<PlaybackPageMenu
				open={page.pagePickerOpen}
				initialFailure={page.pageFailure}
				onClose={page.closeMenu}
			/>
			<PlaybackPageRenameDialog
				page={page.renamePage}
				onClose={page.closeRename}
			/>
		</>
	);
}
