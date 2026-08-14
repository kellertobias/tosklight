import {
	cloneElement,
	createContext,
	isValidElement,
	useContext,
	useEffect,
	useId,
	useMemo,
	useRef,
	useState,
	type CSSProperties,
	type HTMLAttributes,
	type ReactElement,
	type ReactNode,
	type RefAttributes,
} from "react";
import { createPortal } from "react-dom";
import { ModalTitleBar } from "../common/ModalTitleBar";
import type {
	TitleAction,
	TitleActionGroup,
	TitleSearch,
} from "../common/TitleChrome";

export type ModalRole = "modal" | "dialog";

export interface ModalClosePolicy {
	escape?: boolean;
	backdrop?: boolean;
	explicit?: boolean;
}

interface ModalRegistration {
	id: string;
	order: number;
	policy: Required<ModalClosePolicy>;
	requestClose(): void;
}

export interface ModalStackApi {
	close(id: string): boolean;
	updatePolicy(id: string, policy: ModalClosePolicy): boolean;
	isTop(id: string): boolean;
}

interface ModalStackContextValue extends ModalStackApi {
	register(registration: ModalRegistration): void;
	reserve(id: string): number;
	unregister(id: string, restoreFocus?: HTMLElement | null): void;
	indexOf(id: string): number;
}

const ModalStackContext = createContext<ModalStackContextValue | null>(null);

function requiredPolicy(policy: ModalClosePolicy): Required<ModalClosePolicy> {
	return {
		escape: policy.escape ?? true,
		backdrop: policy.backdrop ?? true,
		explicit: policy.explicit ?? true,
	};
}

export function ModalProvider({ children }: { children: ReactNode }) {
	const parent = useContext(ModalStackContext);
	const [entries, setEntries] = useState<ModalRegistration[]>([]);
	const nextOrder = useRef(0);
	const reservedOrders = useRef(new Map<string, number>());
	const entriesRef = useRef(entries);
	const pendingFocusRestore = useRef<HTMLElement | null>(null);
	entriesRef.current = entries;

	const value = useMemo<ModalStackContextValue>(
		() => ({
			register(registration) {
				setEntries((current) =>
					[
						...current.filter((entry) => entry.id !== registration.id),
						registration,
					].sort((left, right) => left.order - right.order),
				);
			},
			reserve(id) {
				const existing = reservedOrders.current.get(id);
				if (existing !== undefined) return existing;
				const order = nextOrder.current++;
				reservedOrders.current.set(id, order);
				return order;
			},
			unregister(id, restoreFocus) {
				reservedOrders.current.delete(id);
				pendingFocusRestore.current = restoreFocus ?? null;
				setEntries((current) => current.filter((entry) => entry.id !== id));
			},
			close(id) {
				const entry = entriesRef.current.find(
					(candidate) => candidate.id === id,
				);
				if (!entry) return false;
				entry.requestClose();
				return true;
			},
			updatePolicy(id, policy) {
				const updated = entriesRef.current.some((entry) => entry.id === id);
				if (!updated) return false;
				setEntries((current) =>
					current.map((entry) => {
						if (entry.id !== id) return entry;
						return { ...entry, policy: { ...entry.policy, ...policy } };
					}),
				);
				return true;
			},
			isTop(id) {
				return entries.at(-1)?.id === id;
			},
			indexOf(id) {
				return entries.findIndex((entry) => entry.id === id);
			},
		}),
		[entries],
	);

	useEffect(() => {
		const handleEscape = (event: KeyboardEvent) => {
			if (event.key !== "Escape") return;
			const target = entriesRef.current.at(-1);
			if (!target?.policy.escape) return;
			event.preventDefault();
			event.stopImmediatePropagation();
			target.requestClose();
		};
		document.addEventListener("keydown", handleEscape, true);
		return () => document.removeEventListener("keydown", handleEscape, true);
	}, []);

	useEffect(() => {
		const target = pendingFocusRestore.current;
		if (!target) return;
		pendingFocusRestore.current = null;
		requestAnimationFrame(() => target.isConnected && target.focus());
	}, [entries]);

	if (parent) return children;
	return (
		<ModalStackContext.Provider value={value}>
			{children}
		</ModalStackContext.Provider>
	);
}

export function useModalStack(): ModalStackApi {
	const context = useContext(ModalStackContext);
	if (!context)
		throw new Error("useModalStack must be used inside ModalProvider");
	return context;
}

export interface ModalLayerProps {
	id?: string;
	role?: ModalRole;
	policy?: ModalClosePolicy;
	ariaLabel: string;
	className?: string;
	dialogClassName?: string;
	children: ReactNode;
	onClose(): void;
}

interface RegisteredModal {
	id: string;
	index: number;
	top: boolean;
	layer: React.RefObject<HTMLElement | null>;
	policy: Required<ModalClosePolicy>;
}

function useRegisteredModal(
	requestedId: string | undefined,
	policy: ModalClosePolicy,
	onClose: () => void,
): RegisteredModal {
	const generatedId = useId();
	const id = requestedId ?? `modal-${generatedId.replaceAll(":", "")}`;
	const stack = useContext(ModalStackContext);
	if (!stack)
		throw new Error("Modal surfaces must be used inside ModalProvider");
	const order = useRef<number | null>(null);
	if (order.current === null) order.current = stack.reserve(id);
	const closeRef = useRef(onClose);
	closeRef.current = onClose;
	const previousFocus = useRef<HTMLElement | null>(null);
	const layer = useRef<HTMLElement>(null);
	const claimedInitialFocus = useRef(false);
	const resolvedPolicy = requiredPolicy(policy);

	useEffect(() => {
		previousFocus.current =
			document.activeElement instanceof HTMLElement
				? document.activeElement
				: null;
		stack.register({
			id,
			order: order.current!,
			policy: resolvedPolicy,
			requestClose: () => closeRef.current(),
		});
		return () => {
			stack.unregister(id, previousFocus.current);
		};
	}, [id]);

	useEffect(() => {
		stack.updatePolicy(id, resolvedPolicy);
	}, [
		id,
		resolvedPolicy.escape,
		resolvedPolicy.backdrop,
		resolvedPolicy.explicit,
	]);

	const index = stack.indexOf(id);
	const top = index >= 0 && stack.isTop(id);
	useEffect(() => {
		if (!top || claimedInitialFocus.current) return;
		claimedInitialFocus.current = true;
		requestAnimationFrame(() => {
			const first =
				layer.current?.querySelector<HTMLElement>(
					"[data-modal-initial-focus]",
				) ??
				layer.current?.querySelector<HTMLElement>(
					"[autofocus], button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex='-1'])",
				);
			(first ?? layer.current)?.focus();
		});
	}, [top]);
	return { id, index, top, layer, policy: resolvedPolicy };
}

export function ModalLayer({
	id: requestedId,
	role = "modal",
	policy = {},
	ariaLabel,
	className = "",
	dialogClassName = "",
	children,
	onClose,
}: ModalLayerProps) {
	const dialog = useRef<HTMLElement>(null);
	const registered = useRegisteredModal(requestedId, policy, onClose);
	const { id, index, top, policy: resolvedPolicy } = registered;
	const layer = (
		<div
			ref={registered.layer as React.RefObject<HTMLDivElement>}
			className={`ui-modal-stack-layer role-${role} ${className}`.trim()}
			data-modal-id={id}
			data-modal-top={top ? "true" : "false"}
			style={
				{
					"--modal-stack-index": Math.max(0, index),
					zIndex: 3000 + Math.max(0, index) * 10,
				} as CSSProperties
			}
			onPointerDown={(event) => {
				if (
					event.target === event.currentTarget &&
					resolvedPolicy.backdrop &&
					top
				)
					onClose();
			}}
		>
			<section
				ref={dialog}
				className={dialogClassName}
				role="dialog"
				aria-modal={top || undefined}
				aria-label={ariaLabel}
				inert={!top}
				tabIndex={-1}
			>
				{children}
			</section>
		</div>
	);
	return createPortal(layer, document.body);
}

export type ModalRegistrationElementProps = HTMLAttributes<HTMLElement> &
	RefAttributes<HTMLElement> & {
		"data-modal-id"?: string;
		"data-modal-top"?: "true" | "false";
		inert?: boolean;
	};

export interface ModalRegistrationProps {
	id?: string;
	policy?: ModalClosePolicy;
	children: ReactElement<ModalRegistrationElementProps>;
	onClose(): void;
}

/**
 * Registers an application-owned workflow modal with the package modal stack
 * without changing its existing layer markup or layout. A bare layer uses the
 * registered close action for backdrop dismissal. A layer with its own pointer
 * handler retains ownership of that behavior so existing guarded close flows
 * are not invoked twice.
 */
export function ModalRegistration({
	id,
	policy = {},
	children,
	onClose,
}: ModalRegistrationProps) {
	if (!isValidElement<ModalRegistrationElementProps>(children)) {
		throw new Error("ModalRegistration requires one modal layer element");
	}
	const registered = useRegisteredModal(id, policy, onClose);
	useEffect(() => {
		const root = registered.layer.current;
		const dialog = root?.matches("[role='dialog'], [role='alertdialog']")
			? root
			: root?.querySelector<HTMLElement>(
					"[role='dialog'], [role='alertdialog']",
				);
		if (!dialog) return;
		if (registered.top) dialog.setAttribute("aria-modal", "true");
		else dialog.removeAttribute("aria-modal");
	}, [registered.top]);
	const childPointerDown = children.props.onPointerDown;
	const style = {
		...children.props.style,
		"--modal-stack-index": Math.max(0, registered.index),
		zIndex: 3000 + Math.max(0, registered.index) * 10,
	} as CSSProperties;
	return createPortal(
		cloneElement(children, {
			ref: registered.layer,
			"data-modal-id": registered.id,
			"data-modal-top": registered.top ? "true" : "false",
			inert: !registered.top || undefined,
			style,
			onPointerDown: (event) => {
				childPointerDown?.(event);
				if (
					!event.defaultPrevented &&
					!childPointerDown &&
					event.target === event.currentTarget &&
					registered.policy.backdrop &&
					registered.top
				)
					onClose();
			},
		}),
		document.body,
	);
}

export type ModalFrameProps = Omit<ModalLayerProps, "children"> & {
	title: ReactNode;
	details?: ReactNode;
	groups?: TitleActionGroup[];
	accept?: TitleAction;
	toolbar?: ReactNode;
	closeDisabled?: boolean;
	closeLabel?: string;
	search?: TitleSearch;
	children: ReactNode;
};

export function ModalFrame({
	title,
	details,
	groups,
	accept,
	toolbar,
	closeDisabled,
	closeLabel,
	children,
	policy,
	onClose,
	...props
}: ModalFrameProps) {
	const { search, ...layerProps } = props;
	const resolvedPolicy = requiredPolicy(policy ?? {});
	return (
		<ModalLayer {...layerProps} policy={resolvedPolicy} onClose={onClose}>
			<ModalTitleBar
				title={title}
				details={details}
				search={search}
				groups={groups}
				accept={accept}
				toolbar={toolbar}
				onClose={resolvedPolicy.explicit ? onClose : undefined}
				closeDisabled={closeDisabled}
				closeLabel={closeLabel}
			/>
			{children}
		</ModalLayer>
	);
}
