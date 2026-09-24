/**
 * An error the operator has to see: its words can be selected and copied — into a report, a
 * message, a search — and it stays until the operator closes it with ×.
 */
import { Button } from "@tosklight/ui";
import { useState } from "react";

export function ErrorMessage({
	message,
	onDismiss,
	className,
}: {
	message: string;
	onDismiss(): void;
	/** Where the message sits: the CAD corner or the window's toast. */
	className: string;
}) {
	const [copied, setCopied] = useState(false);
	const copy = () =>
		navigator.clipboard
			?.writeText(message)
			.then(() => setCopied(true))
			.catch(() => setCopied(false));
	return (
		<output className={`viz-error-message ${className}`} role="alert">
			<span className="viz-error-message-text">{message}</span>
			<span className="viz-error-message-actions">
				<Button aria-label="Copy error" title="Copy the message" onClick={() => void copy()}>
					{copied ? "Copied" : "Copy"}
				</Button>
				<Button aria-label="Dismiss error" title="Close" onClick={onDismiss}>
					×
				</Button>
			</span>
		</output>
	);
}
