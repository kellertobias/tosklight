import type { RefObject } from "react";
import { createPortal } from "react-dom";
import { useServer } from "../../../api/ServerContext";
import { Button } from "../../common";

export function CommandLineHistoryPanel({
	open,
	panel,
	onClose,
	onReuse,
}: {
	open: boolean;
	panel: RefObject<HTMLElement | null>;
	onClose: () => void;
	onReuse: (command: string) => void;
}) {
	const history = useServer().commandHistory;
	if (!open) return null;
	return createPortal(
		<section
			className="command-history-panel"
			role="dialog"
			aria-modal="false"
			aria-label="Command line history"
			ref={panel}
		>
			<header>
				<div>
					<h2>Command Line History</h2>
					<small>Newest first · this desk · last 50 results</small>
				</div>
				<Button aria-label="Close command line history" onClick={onClose}>
					×
				</Button>
			</header>
			<div className="command-history-list">
				{history.length === 0 ? (
					<p className="command-history-empty">
						No accepted or rejected commands yet.
					</p>
				) : (
					history.map((entry) => (
						<article
							className={`command-history-entry ${entry.status}`}
							key={entry.id}
						>
							<div className="command-history-entry-main">
								<span className="command-history-status">
									{entry.status === "accepted" ? "Accepted" : "Rejected"}
								</span>
								<code>{entry.command}</code>
								<small>
									{new Date(entry.at).toLocaleTimeString([], {
										hour: "2-digit",
										minute: "2-digit",
										second: "2-digit",
									})}{" "}
									· {entry.source === "osc" ? "attached hardware" : "desk"}
								</small>
							</div>
							<p>{entry.feedback}</p>
							<Button onClick={() => onReuse(entry.command)}>Reuse</Button>
						</article>
					))
				)}
			</div>
		</section>,
		document.body,
	);
}
