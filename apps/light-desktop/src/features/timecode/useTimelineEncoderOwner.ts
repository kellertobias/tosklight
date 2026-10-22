import { useEffect, useRef } from "react";
import { clearTimecodeEncoderDeck } from "./timecodeEncoderBridge";

/** Relinquish only this editor's encoder deck when its window unmounts. */
export function useTimelineEncoderOwner(): symbol {
	const owner = useRef(Symbol("timecode-editor")).current;
	useEffect(() => () => clearTimecodeEncoderDeck(owner), [owner]);
	return owner;
}
