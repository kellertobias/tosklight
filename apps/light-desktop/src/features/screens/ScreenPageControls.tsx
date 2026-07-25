import { useRef, useState } from "react";
import type { ScreenConfiguration } from "../../api/types";
import { Button } from "../../components/common";
import {
	canAdvancePlaybackPage,
	nextPlaybackPageNumber,
} from "../../components/control/PlaybackPageDialogs";
import { usePlaybackTopologyActions } from "../playbackTopology/PlaybackTopologyProvider";
import { usePlaybackPagesView } from "../playbackTopology/PlaybackTopologyView";
import { useScreens } from "./ScreensContext";

/**
 * Page chrome for one secondary screen. Only an `independent` screen owns a
 * page, and it changes it exclusively through the narrow `setScreenPage`
 * action; a missing Page is created by one typed topology action first.
 */
export function ScreenPageControls({
	screen,
	page,
}: {
	screen: ScreenConfiguration;
	page: number;
}) {
	const { setScreenPage } = useScreens();
	const topology = usePlaybackPagesView();
	const actions = usePlaybackTopologyActions();
	const [picker, setPicker] = useState(false);
	const authority = useRef(actions);
	authority.current = actions;

	const independent = screen.page_mode === "independent";
	const ready = topology.ready && Boolean(actions);
	const writable = independent && ready;
	const pages = topology.pages.map((item) => item.body);

	const setPage = (next: number) => {
		if (writable) void setScreenPage(screen.id, next);
	};
	/** Creates the missing Page, then adopts it only under surviving authority. */
	const createPage = async (next: number) => {
		if (!writable || !actions) return false;
		const outcome = await actions.createPage(next);
		if (!outcome || authority.current !== actions) return false;
		setPage(next);
		return true;
	};
	const advance = async () => {
		const next = page + 1;
		if (pages.some((item) => item.number === next)) return setPage(next);
		await createPage(next);
	};
	const addPage = async () => {
		const next = nextPlaybackPageNumber(pages);
		if (next != null && (await createPage(next))) setPicker(false);
	};

	return (
		<div className="screen-page-controls">
			<Button disabled={!writable || page <= 1} onClick={() => setPage(page - 1)}>
				▲ PAGE UP
			</Button>
			<Button onClick={() => writable && setPicker(true)}>
				<strong>{page}</strong>
				<span>
					{pages.find((item) => item.number === page)?.name ?? `Page ${page}`}
				</span>
			</Button>
			<Button
				disabled={!writable || !canAdvancePlaybackPage(pages, page)}
				onClick={() => void advance()}
			>
				PAGE DOWN ▼
			</Button>
			{picker && (
				<div className="screen-page-picker">
					<Button onClick={() => setPicker(false)}>×</Button>
					{pages.map((item) => (
						<Button
							className={item.number === page ? "active" : ""}
							key={item.number}
							onClick={() => {
								setPage(item.number);
								setPicker(false);
							}}
						>
							{item.number} · {item.name}
						</Button>
					))}
					<Button
						disabled={!writable || nextPlaybackPageNumber(pages) == null}
						onClick={() => void addPage()}
					>
						Add new page
					</Button>
				</div>
			)}
		</div>
	);
}
