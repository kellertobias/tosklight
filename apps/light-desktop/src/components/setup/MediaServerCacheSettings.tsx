import { Button } from "@tosklight/ui";
import { useState } from "react";
import { useMediaServers } from "../../features/mediaServers/MediaServersContext";

type Outcome = { tone: "status" | "alert"; text: string };

/** Show Patch Settings › Media Servers: the desk's cached server artwork. */
export function MediaServerCacheSettings() {
	const server = useMediaServers();
	const [busy, setBusy] = useState(false);
	const [outcome, setOutcome] = useState<Outcome | null>(null);
	const clear = async () => {
		if (!server) {
			setOutcome({
				tone: "alert",
				text: "The desk connection is unavailable. Reconnect, then Clear Thumbnail Cache again.",
			});
			return;
		}
		setBusy(true);
		setOutcome(null);
		try {
			const cleared = await server.clearMediaThumbnailCache();
			setOutcome({
				tone: "status",
				text:
					cleared === 1
						? "Cleared 1 cached thumbnail. Refresh Thumbnails on a server row to fetch it again."
						: `Cleared ${cleared} cached thumbnails. Refresh Thumbnails on a server row to fetch them again.`,
			});
		} catch (reason) {
			setOutcome({
				tone: "alert",
				text: `${reason instanceof Error ? reason.message : "The desk did not clear the cache."} Check the desk connection, then Clear Thumbnail Cache again.`,
			});
		} finally {
			setBusy(false);
		}
	};
	return (
		<section className="media-server-cache-settings">
			<h3>Thumbnail cache</h3>
			<p>
				The desk keeps Media Server thumbnails so media pickers open instantly.
				Clearing drops every cached thumbnail; live previews and the patch stay
				as they are.
			</p>
			<Button disabled={busy} onClick={() => void clear()}>
				{busy ? "Clearing…" : "Clear Thumbnail Cache"}
			</Button>
			{outcome ? <p role={outcome.tone}>{outcome.text}</p> : null}
		</section>
	);
}
