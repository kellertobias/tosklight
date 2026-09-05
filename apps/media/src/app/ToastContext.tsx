import { Button } from "@tosklight/ui/controls";
import { newIdentity } from "../shared/api/identity";
import {
	createContext,
	type ReactNode,
	useCallback,
	useContext,
	useEffect,
	useRef,
	useState,
} from "react";

interface ToastApi {
	error(message: string): void;
}

const ToastContext = createContext<ToastApi>({ error: () => undefined });

/** The error callback remains usable after a page unmounts while an accepted save finishes. */
export function useToast(): ToastApi {
	return useContext(ToastContext);
}

export function ToastProvider({ children }: { children: ReactNode }) {
	const [toasts, setToasts] = useState<Array<{ id: string; message: string }>>(
		[],
	);
	const error = useCallback((message: string) => {
		setToasts((current) => {
			if (current.some((toast) => toast.message === message)) return current;
			return [...current, { id: newIdentity(), message }];
		});
	}, []);
	return (
		<ToastContext.Provider value={{ error }}>
			{children}
			<div className="media-toast-region" aria-label="Notifications">
				{toasts.map((toast) => (
					<div className="media-toast is-error" role="alert" key={toast.id}>
						<span>{toast.message}</span>
						<Button
							size="compact"
							onClick={() =>
								setToasts((current) =>
									current.filter((candidate) => candidate.id !== toast.id),
								)
							}
						>
							Dismiss
						</Button>
					</div>
				))}
			</div>
		</ToastContext.Provider>
	);
}

export function useFailureToast(
	failure: { message: string } | undefined,
	onShown?: () => void,
) {
	const toasts = useContext(ToastContext);
	const shown = useRef<string | undefined>(undefined);
	useEffect(() => {
		if (!failure || shown.current === failure.message) return;
		shown.current = failure.message;
		toasts.error(failure.message);
		onShown?.();
	}, [failure, onShown, toasts]);
}

/** A page-local toast for features that can also be rendered outside the application provider. */
export function MediaErrorToast({
	message,
	onDismiss,
}: {
	message: string;
	onDismiss: () => void;
}) {
	return (
		<div className="media-toast-region" aria-label="Notifications">
			<div className="media-toast is-error" role="alert">
				<span>{message}</span>
				<Button size="compact" onClick={onDismiss}>
					Dismiss
				</Button>
			</div>
		</div>
	);
}
