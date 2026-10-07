/**
 * An error the operator has to see: its words can be selected and copied — into a report, a
 * message, a search — and it stays until the operator closes it with ×.
 */
import { Button, CopyErrorButton } from "@tosklight/ui";

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
	return (
		<output className={`viz-error-message ${className}`} role="alert">
			<span className="viz-error-message-text">{message}</span>
			<span className="viz-error-message-actions">
				<CopyErrorButton text={message} />
				<Button aria-label="Dismiss error" title="Close" onClick={onDismiss}>
					×
				</Button>
			</span>
		</output>
	);
}
