import { useConnectionStatus } from "../../../features/shellStatus/ShellStatusState";
import {
	useActiveTimecode,
	useFrameRateHz,
} from "../../../features/deskSnapshot/DeskSnapshotState";
import { CommandLineStatus } from "./CommandLineStatus";

/** Keeps broad connection/bootstrap updates at the status leaf. */
export function CommandLineStatusBoundary({ onOpen }: { onOpen: () => void }) {
	const connectionStatus = useConnectionStatus();
	const frameRateHz = useFrameRateHz();
	const timecode = useActiveTimecode();
	return (
		<CommandLineStatus
			status={connectionStatus}
			frequency={frameRateHz ?? "—"}
			timecode={timecode}
			onOpen={onOpen}
		/>
	);
}
