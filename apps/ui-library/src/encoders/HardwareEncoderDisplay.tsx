import { forwardRef, useCallback, useImperativeHandle, useState } from "react";
import { Button } from "../common";
import { ModalNumberEditor } from "../input/ModalNumberEditor";
import { submitNumericExpression } from "../input/numericExpression";

export type HardwareEncoderTarget = {
	label: string;
	value: string;
	role?: string;
};

export interface HardwareEncoderDisplayHandle {
	activate(): void;
}

export interface HardwareEncoderDisplayProps {
	slot: number;
	target?: HardwareEncoderTarget;
	secondary?: HardwareEncoderTarget;
	editValue?: number;
	onEdit?: (value: number) => void;
	onEditRange?: (points: number[]) => void;
	canRelease?: boolean;
	onRelease?: () => void;
}

export const HardwareEncoderDisplayView = forwardRef<
	HardwareEncoderDisplayHandle,
	HardwareEncoderDisplayProps
>(function HardwareEncoderDisplayView(
	{
		slot,
		target,
		secondary,
		editValue,
		onEdit,
		onEditRange,
		canRelease = false,
		onRelease,
	},
	ref,
) {
	const [editing, setEditing] = useState(false);
	const [inputValue, setInputValue] = useState("");
	const openEditor = useCallback(() => {
		const next = String(Number((editValue ?? 0).toFixed(1)));
		setInputValue(next);
		setEditing(true);
	}, [editValue]);
	useImperativeHandle(ref, () => ({ activate: openEditor }), [openEditor]);
	const submit = () => {
		if (submitNumericExpression(inputValue, onEdit, onEditRange))
			setEditing(false);
	};
	if (!target)
		return (
			<section
				className="hardware-encoder-display unassigned"
				aria-label={`Encoder ${slot} unassigned`}
			>
				<header>
					<b>Unassigned</b>
					<small>Enc {slot}</small>
				</header>
			</section>
		);
	const content = (
		<>
			<header>
				<b title={target.label}>{target.label}</b>
				<small>Enc {slot}</small>
			</header>
			<div className="hardware-encoder-target">
				<strong>{target.value}</strong>
				{target.role && <span>{target.role}</span>}
			</div>
			{secondary && (
				<div className="hardware-encoder-target secondary">
					<b title={secondary.label}>{secondary.label}</b>
					<strong>{secondary.value}</strong>
					{secondary.role && <span>{secondary.role}</span>}
				</div>
			)}
		</>
	);
	const displayClassName = `hardware-encoder-display ${secondary ? "dual-target" : "single-target"}`;
	return (
		<>
			{onEdit ? (
				<Button
					className={displayClassName}
					aria-label={`Encoder ${slot}: ${target.label}, ${target.value}`}
					onClick={openEditor}
				>
					{content}
				</Button>
			) : (
				<section
					className={displayClassName}
					aria-label={`Encoder ${slot}: ${target.label}, ${target.value}`}
				>
					{content}
				</section>
			)}
			{editing && (
				<ModalNumberEditor
					ariaLabel={`Encoder ${slot} value`}
					dialogClassName="direct-value-modal hardware-encoder-modal"
					title={target.label}
					value={inputValue}
					onChange={setInputValue}
					onSubmit={submit}
					onClose={() => setEditing(false)}
					allowThrough={Boolean(onEditRange)}
					onRelease={
						canRelease && onRelease
							? () => {
									onRelease();
									setEditing(false);
								}
							: undefined
					}
					releaseLabel={`Release ${target.label}`}
				/>
			)}
		</>
	);
});
