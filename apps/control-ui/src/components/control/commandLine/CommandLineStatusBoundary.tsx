import { useServer } from "../../../api/ServerContext";
import { CommandLineStatus } from "./CommandLineStatus";

/** Keeps broad connection/bootstrap updates at the status leaf. */
export function CommandLineStatusBoundary({ onOpen }: { onOpen: () => void }) {
	const server = useServer();
	return (
		<CommandLineStatus
			status={server.status}
			frequency={server.bootstrap?.frame_rate_hz ?? "—"}
			timecode={server.bootstrap?.active_timecode ?? null}
			onOpen={onOpen}
		/>
	);
}
