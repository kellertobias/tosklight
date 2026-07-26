import {
	type CSSProperties,
	type MouseEvent,
	type PointerEvent,
	type ReactNode,
	useEffect,
	useRef,
} from "react";
import { iconCatalogItem } from "../common/controls/iconCatalog";
import { Button, type ButtonProps } from "../controls";
import type { PoolPresentationState } from "./poolColors";

export type PoolCardState = PoolPresentationState;

export interface PoolCardViewModel {
	number: ReactNode;
	primary: ReactNode;
	secondary?: ReactNode;
	details?: string[];
	icon?: ReactNode;
	iconColor?: string;
	iconBackgroundColor?: string;
	image?: { src: string; alt: string };
	status?: ReactNode;
	workflow?: ReactNode;
	color?: string;
	states?: PoolCardState[];
	kind?: "group" | "preset" | "cuelist" | "generic";
	derived?: boolean;
	derivedLabel?: ReactNode;
	frozen?: boolean;
	frozenLabel?: ReactNode;
}

export interface PoolCardProps extends Omit<ButtonProps, "children"> {
	model: PoolCardViewModel;
	onPressHold?: () => void;
	holdDelay?: number;
}

export function PoolCard({
	model,
	className = "",
	onPressHold,
	holdDelay = 650,
	onClick,
	onPointerDown,
	onPointerUp,
	onPointerCancel,
	onPointerLeave,
	...props
}: PoolCardProps) {
	const workflow = resolveWorkflow(model);
	const empty = model.states?.includes("empty") ?? false;
	const color = empty ? undefined : model.color;
	const hasStatus = Boolean(
		workflow || model.status != null || model.derived || model.frozen,
	);
	const hasMedia = Boolean(model.image || model.icon != null || color);
	const interactions = usePoolCardPressHold({
		onPressHold,
		holdDelay,
		onClick,
		onPointerDown,
		onPointerUp,
		onPointerCancel,
		onPointerLeave,
	});
	return (
		<Button
			{...props}
			className={poolCardClassName(
				model,
				className,
				hasStatus,
				hasMedia,
				Boolean(color),
			)}
			disabled={props.disabled || model.states?.includes("disabled")}
			style={poolCardStyle(model, props.style, empty)}
			{...interactions}
		>
			<PoolCardContents
				model={model}
				workflow={workflow}
				hasStatus={hasStatus}
				hasMedia={hasMedia}
				color={color}
			/>
		</Button>
	);
}

function usePoolCardPressHold({
	onPressHold,
	holdDelay,
	onClick,
	onPointerDown,
	onPointerUp,
	onPointerCancel,
	onPointerLeave,
}: Pick<
	PoolCardProps,
	| "onPressHold"
	| "holdDelay"
	| "onClick"
	| "onPointerDown"
	| "onPointerUp"
	| "onPointerCancel"
	| "onPointerLeave"
>) {
	const holdTimer = useRef<number | null>(null);
	const held = useRef(false);
	const clearHold = () => {
		if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
		holdTimer.current = null;
	};
	useEffect(
		() => () => {
			if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
		},
		[],
	);
	return {
		onPointerDown(event: PointerEvent<HTMLButtonElement>) {
			onPointerDown?.(event);
			if (!onPressHold || event.defaultPrevented) return;
			held.current = false;
			clearHold();
			holdTimer.current = window.setTimeout(() => {
				held.current = true;
				onPressHold();
			}, holdDelay);
		},
		onPointerUp(event: PointerEvent<HTMLButtonElement>) {
			clearHold();
			onPointerUp?.(event);
		},
		onPointerCancel(event: PointerEvent<HTMLButtonElement>) {
			clearHold();
			onPointerCancel?.(event);
		},
		onPointerLeave(event: PointerEvent<HTMLButtonElement>) {
			clearHold();
			onPointerLeave?.(event);
		},
		onClick(event: MouseEvent<HTMLButtonElement>) {
			if (held.current) {
				held.current = false;
				return;
			}
			onClick?.(event);
		},
	};
}

function poolCardClassName(
	model: PoolCardViewModel,
	className: string,
	hasStatus: boolean,
	hasMedia: boolean,
	hasColor: boolean,
) {
	return [
		"pool-card",
		"pool-cell",
		`${model.kind ?? "generic"}-card`,
		hasColor && "has-color",
		hasStatus && "has-status",
		hasMedia && "has-media",
		model.derived && "derived",
		model.frozen && "frozen",
		...(model.states ?? []),
		className,
	]
		.filter(Boolean)
		.join(" ");
}

function poolCardStyle(
	model: PoolCardViewModel,
	style: CSSProperties | undefined,
	empty: boolean,
) {
	const resolved = {
		...(!empty && model.color ? { "--pool-card-color": model.color } : {}),
		...(!empty && model.iconColor
			? { "--pool-card-icon-color": model.iconColor }
			: {}),
		...(!empty && model.iconBackgroundColor
			? { "--pool-card-icon-background": model.iconBackgroundColor }
			: {}),
		...style,
	} as CSSProperties & Record<string, unknown>;
	if (empty) {
		delete resolved["--pool-card-color"];
		delete resolved["--pool-card-icon-color"];
		delete resolved["--pool-card-icon-background"];
	}
	return resolved;
}

function PoolCardContents({
	model,
	workflow,
	hasStatus,
	hasMedia,
	color,
}: {
	model: PoolCardViewModel;
	workflow: ReturnType<typeof resolveWorkflow>;
	hasStatus: boolean;
	hasMedia: boolean;
	color: string | undefined;
}) {
	const catalogIcon =
		typeof model.icon === "string" ? iconCatalogItem(model.icon) : undefined;
	return (
		<>
			<span className="number">{model.number}</span>
			<b className="pool-card-name">{model.primary}</b>
			{(model.secondary != null || model.details?.length) && (
				<span className="pool-card-information">
					{model.secondary != null && <small>{model.secondary}</small>}
					{model.details?.map((detail) => (
						<em key={detail}>{detail}</em>
					))}
				</span>
			)}
			{hasStatus && (
				<span className="pool-card-status-row">
					{workflow && (
						<span
							className={`pool-card-workflow ${workflow.kind}`}
							data-pool-workflow={workflow.kind}
						>
							{workflow.label}
						</span>
					)}
					{model.status != null && (
						<span className="pool-card-status">{model.status}</span>
					)}
					{model.derived && (
						<span
							className="pool-card-state-marker derived"
							role="img"
							aria-label="Derived state"
						>
							{model.derivedLabel ?? "Derived"}
						</span>
					)}
					{model.frozen && (
						<span
							className="pool-card-state-marker frozen"
							role="img"
							aria-label="Frozen state"
						>
							{model.frozenLabel ?? "Frozen"}
						</span>
					)}
				</span>
			)}
			{hasMedia && (
				<span className="pool-card-media">
					{!model.image && color && (
						<span
							className="pool-card-color-label"
							role="img"
							aria-label={`Configured color ${color}`}
							title={`Color ${color}`}
						/>
					)}
					{model.image ? (
						<img
							className="pool-card-image"
							src={model.image.src}
							alt={model.image.alt}
						/>
					) : catalogIcon?.source === "catalog" ? (
						<img
							className="pool-card-icon-image"
							src={catalogIcon.value}
							alt=""
						/>
					) : model.icon != null ? (
						<span className="pool-card-icon">{model.icon}</span>
					) : (
						<span className="pool-card-color" style={{ background: color }} />
					)}
				</span>
			)}
		</>
	);
}

function resolveWorkflow(model: PoolCardViewModel) {
	const states = new Set(model.states ?? []);
	if (states.has("update-target"))
		return { kind: "update", label: model.workflow ?? "Update" } as const;
	if (states.has("record-target") || states.has("store-target"))
		return { kind: "record", label: model.workflow ?? "Record" } as const;
	if (states.has("set-target"))
		return { kind: "set", label: model.workflow ?? "Set" } as const;
	if (model.workflow != null)
		return { kind: "custom", label: model.workflow } as const;
	return null;
}
