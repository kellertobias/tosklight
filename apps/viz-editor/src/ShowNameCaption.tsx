import { useRef, useState } from "react";

/**
 * The show's name over the rig overview, renamed in place from the pencil beside it.
 *
 * This is the name a desk's menu offers, not the file's name: Enter or leaving the field keeps
 * the edit, Escape drops it, and an empty or unchanged name changes nothing.
 */
export function ShowNameCaption({
	name,
	onRename,
}: {
	name: string;
	onRename: (name: string) => Promise<void>;
}) {
	const [draft, setDraft] = useState<string | null>(null);
	// A field that closes on Enter also blurs; only the first of the two may settle the edit.
	const editing = useRef(false);

	const settle = (keep: boolean) => {
		if (!editing.current) return;
		editing.current = false;
		const next = (draft ?? "").trim();
		setDraft(null);
		if (keep && next && next !== name) void onRename(next);
	};

	return (
		<figcaption>
			<span>Rig overview</span>
			{draft === null ? (
				<div className="viz-show-name">
					<strong>{name}</strong>
					<button
						type="button"
						className="viz-show-name-edit"
						aria-label="Rename show"
						title="Rename show"
						onClick={() => {
							editing.current = true;
							setDraft(name);
						}}
					>
						<svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
							<path
								d="M11.2 2.2a1.5 1.5 0 0 1 2.1 0l.5.5a1.5 1.5 0 0 1 0 2.1L6 12.6 2.5 13.5l.9-3.5z M10 3.4l2.6 2.6"
								fill="none"
								stroke="currentColor"
								strokeWidth="1.4"
								strokeLinejoin="round"
							/>
						</svg>
					</button>
				</div>
			) : (
				<input
					className="viz-show-name-input"
					aria-label="Show name"
					value={draft}
					// biome-ignore lint/a11y/noAutofocus: the field opens only from its own pencil.
					autoFocus
					onFocus={(event) => event.currentTarget.select()}
					onChange={(event) => setDraft(event.target.value)}
					onBlur={() => settle(true)}
					onKeyDown={(event) => {
						event.stopPropagation();
						if (event.key === "Enter") settle(true);
						if (event.key === "Escape") settle(false);
					}}
				/>
			)}
		</figcaption>
	);
}
