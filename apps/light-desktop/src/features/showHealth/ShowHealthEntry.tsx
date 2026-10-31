import { Button } from "@tosklight/ui";
import { healthCounts, useShowHealth } from "./useShowHealth";
import "./showHealth.css";

export function ShowHealthEntry({ onOpen }: { onOpen: () => void }) {
	const check = useShowHealth();
	return (
		<Button onClick={onOpen} disabled={!check.showId}>
			<span>Show Health</span>
			<small className="show-health-badge">
				{check.loading
					? "Checking…"
					: check.snapshot
						? healthCounts(check.snapshot)
						: "Run check"}
			</small>
		</Button>
	);
}
