import { createContext, useContext, type ReactNode } from "react";

/**
 * Where the operator is in the fixture editor: fixture, then mode, then channel, then functions.
 *
 * Each of those is its own stacked window, and a window only knows how to close itself. So every
 * level passes the path down, and a level wraps each crumb above it so that choosing the crumb
 * closes this level first and then whatever the crumb itself closes. Choosing the fixture from the
 * functions window therefore closes functions, the channel, and the mode, in that order.
 */
export type EditorCrumb = { label: string; onSelect?: () => void };

const TrailContext = createContext<readonly EditorCrumb[]>([]);

/** The path to this window, given its own crumbs and how it closes. */
export function useEditorTrail(own: readonly string[], onClose?: () => void) {
	const above = useContext(TrailContext);
	return [
		...above.map((crumb) => ({
			label: crumb.label,
			onSelect: onClose
				? () => {
						onClose();
						crumb.onSelect?.();
					}
				: crumb.onSelect,
		})),
		...own.map((label) => ({ label })),
	];
}

export function EditorTrailProvider({
	trail,
	children,
}: {
	trail: readonly EditorCrumb[];
	children: ReactNode;
}) {
	return (
		<TrailContext.Provider value={trail}>{children}</TrailContext.Provider>
	);
}

export function EditorBreadcrumbs({ trail }: { trail: readonly EditorCrumb[] }) {
	return (
		<nav className="fixture-editor-breadcrumbs" aria-label="Editing path">
			<ol>
				{trail.map((crumb, index) => {
					const current = index === trail.length - 1;
					return (
						// Labels repeat (a mode and a channel can share a name), so position is the key.
						// biome-ignore lint/suspicious/noArrayIndexKey: the trail is positional
						<li key={index}>
							{crumb.onSelect && !current ? (
								<button type="button" onClick={crumb.onSelect}>
									{crumb.label}
								</button>
							) : (
								<span aria-current={current ? "location" : undefined}>
									{crumb.label}
								</span>
							)}
						</li>
					);
				})}
			</ol>
		</nav>
	);
}
