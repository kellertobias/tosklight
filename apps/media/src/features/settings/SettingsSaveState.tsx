export function SettingsSaveState({
	busy,
	failed,
	restartBound = false,
	note,
}: {
	busy: boolean;
	failed: boolean;
	restartBound?: boolean;
	note?: string;
}) {
	const suffix = note ? ` · ${note}` : restartBound ? " · Applies on restart" : "";
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
