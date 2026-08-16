export function SettingsSaveState({
	busy,
	failed,
	restartBound = false,
}: {
	busy: boolean;
	failed: boolean;
	restartBound?: boolean;
}) {
	const suffix = restartBound ? " · Applies on restart" : "";
	return (
		<p className="media-settings-save-state" role="status" aria-live="polite">
			{failed
				? `Not saved · Check the error${suffix}`
				: busy
					? `Saving…${suffix}`
					: `Saved automatically${suffix}`}
		</p>
	);
}
