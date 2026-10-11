import { ModalRegistration } from "@tosklight/ui/modals";
import { useEffect, useState } from "react";
import { useDesktopBridge } from "../../platform/desktop";

export function QuitConfirmOverlay() {
	const desktop = useDesktopBridge();
	const [visible, setVisible] = useState(false);
	const dismiss = () => {
		setVisible(false);
		void desktop.cancelQuit();
	};
	useEffect(() => {
		if (!desktop.available) return;
		const cleanups: (() => void)[] = [];
		let cancelled = false;
		const register = (subscription: Promise<() => void>) => {
			void subscription.then((unlisten) => {
				if (cancelled) unlisten();
				else cleanups.push(unlisten);
			});
		};
		register(desktop.onQuitRequested(() => setVisible(true)));
		register(desktop.onQuitCancelled(() => setVisible(false)));
		return () => {
			cancelled = true;
			for (const cleanup of cleanups) cleanup();
		};
	}, [desktop]);
	if (!visible) return null;
	return (
		<ModalRegistration onClose={dismiss}>
			<div className="quit-confirm-cover" onPointerDown={dismiss}>
				<div>
					<h1>Really quit?</h1>
					<p>Press <kbd>⌘Q</kbd> again to quit ToskLight.</p>
					<small>Press Esc or click anywhere to keep it running.</small>
				</div>
			</div>
		</ModalRegistration>
	);
}
