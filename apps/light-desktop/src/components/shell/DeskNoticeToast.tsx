import { Button } from "@tosklight/ui";
import { useEffect, useState } from "react";
import {
	DESK_NOTICE_DURATION_MS,
	DESK_NOTICE_EVENT,
} from "../../features/deskNotice/deskNotice";

interface DisplayedNotice {
	id: number;
	message: string;
}

/**
 * A brief, non-blocking lane for harmless no-op actions. It announces politely, expires on its
 * own, and can be dismissed early; it never takes focus or blocks desk controls.
 */
export function DeskNoticeToast() {
	const [notice, setNotice] = useState<DisplayedNotice | null>(null);
	useEffect(() => {
		let next = 0;
		const show = (event: Event) => {
			const message = (event as CustomEvent<string>).detail;
			if (!message) return;
			next += 1;
			setNotice({ id: next, message });
		};
		window.addEventListener(DESK_NOTICE_EVENT, show);
		return () => window.removeEventListener(DESK_NOTICE_EVENT, show);
	}, []);
	useEffect(() => {
		if (!notice) return;
		const timer = globalThis.setTimeout(
			() =>
				setNotice((current) => (current?.id === notice.id ? null : current)),
			DESK_NOTICE_DURATION_MS,
		);
		return () => globalThis.clearTimeout(timer);
	}, [notice]);
	return (
		<div className="desk-notice-lane" role="status" aria-live="polite">
			{notice && (
				<aside className="desk-notice-toast" aria-label="Desk notice">
					<span>{notice.message}</span>
					<Button aria-label="Dismiss notice" onClick={() => setNotice(null)}>
						OK
					</Button>
				</aside>
			)}
		</div>
	);
}
