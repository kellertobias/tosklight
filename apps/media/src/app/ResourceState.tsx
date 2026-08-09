// Loading, empty, error, retry, and disconnected — decided once.
//
// The shared package owns how these look; Media owns *when* they occur. That split is why this
// component takes a resource and not a design decision.

import { Button } from "@tosklight/ui/controls";
import type { ReactNode } from "react";
import type { Resource } from "../shared/api/resource";

export interface ResourceStateProps<T> {
	resource: Resource<T>;
	/** What this panel is called, so a message can name it. */
	subject: string;
	children: (data: T) => ReactNode;
	/** Shown when the load succeeded but there is nothing to show. */
	empty?: ReactNode;
	isEmpty?: (data: T) => boolean;
}

export function ResourceState<T>({
	resource,
	subject,
	children,
	empty,
	isEmpty,
}: ResourceStateProps<T>) {
	const { data, failure, loading, stale, reload } = resource;

	if (data === undefined) {
		if (failure) return <Failed subject={subject} message={failure.message} onRetry={reload} />;
		return (
			<p className="media-state" role="status" aria-live="polite">
				{loading ? `Loading ${subject}…` : `${capitalize(subject)} not loaded yet.`}
			</p>
		);
	}

	return (
		<>
			{stale && failure && (
				<p className="media-state is-stale" role="status" aria-live="polite">
					Showing the last known {subject}. {failure.message}{" "}
					<Button size="compact" onClick={reload}>
						Retry
					</Button>
				</p>
			)}
			{isEmpty?.(data) && empty ? (
				<p className="media-state is-empty">{empty}</p>
			) : (
				children(data)
			)}
		</>
	);
}

function Failed({
	subject,
	message,
	onRetry,
}: {
	subject: string;
	message: string;
	onRetry: () => void;
}) {
	return (
		<div className="media-state is-error" role="alert">
			<p>
				{capitalize(subject)} could not be loaded. {message}
			</p>
			<Button variant="primary" onClick={onRetry}>
				Try again
			</Button>
		</div>
	);
}

function capitalize(text: string): string {
	return text.charAt(0).toUpperCase() + text.slice(1);
}
