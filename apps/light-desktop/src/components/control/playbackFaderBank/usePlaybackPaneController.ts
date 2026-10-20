import { useMemo, useRef, useState } from "react";
import { poolCardSizing } from "../../../state/reducerHelpers";
import {
	type PlaybackFaderBankProps,
	usePlaybackBankController,
} from "./controller";
import { reflowPlaybackCells } from "./paneTileGeometry";
import { usePaneTileGeometry } from "./usePaneTileGeometry";

/** Keep pane geometry separate from authoritative Playback state and rendering. */
export function usePlaybackPaneController(props: PlaybackFaderBankProps) {
	const sourceController = usePlaybackBankController(props);
	const [bankElement, setBankElement] = useState<HTMLDivElement | null>(null);
	const viewport = useRef<HTMLDivElement | null>(null);
	const paneGeometry = usePaneTileGeometry(
		props.tilePresentation ? props.tileLayout : undefined,
		viewport.current,
		bankElement,
		sourceController.slots.length,
		poolCardSizing(sourceController.state),
	);
	const reflowedSlots = useMemo(
		() =>
			paneGeometry
				? reflowPlaybackCells(sourceController.slots, paneGeometry.columns)
				: sourceController.slots,
		[sourceController.slots, paneGeometry?.columns],
	);
	const controller = paneGeometry
		? {
				...sourceController,
				slots: reflowedSlots,
				columns: paneGeometry.columns,
				rowCount: paneGeometry.rows,
			}
		: sourceController;
	return { controller, paneGeometry, viewport, bankElement, setBankElement };
}
