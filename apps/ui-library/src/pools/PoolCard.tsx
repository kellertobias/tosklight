import { type CSSProperties, type ReactNode, useEffect, useRef } from "react";
import { Button, type ButtonProps } from "../controls";

export type PoolCardState =
	| "empty"
	| "selected"
	| "active"
	| "disabled"
	| "store-target"
	| "update-target"
	| "set-target";

export interface PoolCardViewModel {
	number: ReactNode;
	primary: ReactNode;
	secondary?: ReactNode;
	details?: string[];
	icon?: ReactNode;
	image?: { src: string; alt: string };
	status?: ReactNode;
	workflow?: ReactNode;
	color?: string;
	states?: PoolCardState[];
	kind?: "group" | "preset" | "cuelist" | "generic";
	derived?: boolean;
	frozen?: boolean;
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
	const classes = [
		"pool-card",
		"pool-cell",
		`${model.kind ?? "generic"}-card`,
		model.color && "has-color",
		model.derived && "derived",
		model.frozen && "frozen",
		...(model.states ?? []),
		className,
	]
		.filter(Boolean)
		.join(" ");
	return (
		<Button
			{...props}
			className={classes}
			disabled={props.disabled || model.states?.includes("disabled")}
			style={
				{
					...props.style,
					...(model.color ? { "--pool-card-color": model.color } : {}),
				} as CSSProperties
			}
			onPointerDown={(event) => {
				onPointerDown?.(event);
				if (!onPressHold || event.defaultPrevented) return;
				held.current = false;
				clearHold();
				holdTimer.current = window.setTimeout(() => {
					held.current = true;
					onPressHold();
				}, holdDelay);
			}}
			onPointerUp={(event) => {
				clearHold();
				onPointerUp?.(event);
			}}
			onPointerCancel={(event) => {
				clearHold();
				onPointerCancel?.(event);
			}}
			onPointerLeave={(event) => {
				clearHold();
				onPointerLeave?.(event);
			}}
			onClick={(event) => {
				if (held.current) {
					held.current = false;
					return;
				}
				onClick?.(event);
			}}
		>
			<span className="number">{model.number}</span>
			<b>{model.primary}</b>
			{model.secondary != null && <small>{model.secondary}</small>}
			{model.details?.map((detail) => (
				<em key={detail}>{detail}</em>
			))}
			{model.derived && (
				<span
					className="pool-card-state-marker derived"
					role="img"
					aria-label="Derived state"
				>
					Derived
				</span>
			)}
			{model.frozen && (
				<span
					className="pool-card-state-marker frozen"
					role="img"
					aria-label="Frozen state"
				>
					Frozen
				</span>
			)}
			{model.status != null && (
				<span className="pool-card-status">{model.status}</span>
			)}
			{model.workflow != null && (
				<span className="pool-card-workflow">{model.workflow}</span>
			)}
			{model.color && (
				<span
					className="pool-card-color"
					role="img"
					aria-label={`Configured color ${model.color}`}
					title={`Color ${model.color}`}
					style={{ background: model.color }}
				/>
			)}
			{model.image ? (
				<img
					className="pool-card-image"
					src={model.image.src}
					alt={model.image.alt}
				/>
			) : (
				model.icon != null && (
					<span className="pool-card-icon">{model.icon}</span>
				)
			)}
		</Button>
	);
}
