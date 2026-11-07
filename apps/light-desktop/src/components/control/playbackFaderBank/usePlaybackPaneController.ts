import { useMemo, useRef, useState } from "react";
import { poolCardSizing } from "../../../state/reducerHelpers";
import {
	type PlaybackFaderBankProps,
	usePlaybackBankController,
} from "./controller";
import { projectPlaybackPaneFootprints } from "./paneFootprints";
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
	const paneColumns = paneGeometry?.columns;
	const slots = useMemo(() => {
		const cells = paneColumns
			? reflowPlaybackCells(sourceController.slots, paneColumns)
			: sourceController.slots;
		return sourceController.paneFootprints
			? projectPlaybackPaneFootprints(cells)
			: cells;
	}, [sourceController.slots, sourceController.paneFootprints, paneColumns]);
	const controller = paneGeometry || slots !== sourceController.slots
		? {
				...sourceController,
				slots,
				columns: paneGeometry?.columns ?? sourceController.columns,
				rowCount: paneGeometry?.rows ?? sourceController.rowCount,
			}
		: sourceController;
	return { controller, paneGeometry, viewport, bankElement, setBankElement };
}
