import { TextField } from "@tosklight/ui";
import { useEffect, useRef, useState } from "react";
import { formatFrame, parseTimelineFrame } from "./timecodeEditorShared";

/** Keeps incomplete keyboard input local while valid complete positions autosave. */
export function TimecodeFrameField({
	label,
	value,
	fps,
	minimum = 0,
	maximum,
	onChange,
}: {
	label: string;
	value: number;
	fps: number;
	minimum?: number;
	maximum?: number;
	onChange(frame: number): void;
}) {
	const [text, setText] = useState(() => formatFrame(value, fps));
	const [error, setError] = useState<string>();
	const focused = useRef(false);
	useEffect(() => {
		if (!focused.current) {
			setText(formatFrame(value, fps));
			setError(undefined);
		}
	}, [value, fps]);
	const parse = (candidate: string) => {
		const parsed = parseTimelineFrame(candidate, fps);
		return parsed !== null &&
			Number.isSafeInteger(parsed) &&
			parsed >= minimum &&
			(maximum === undefined || parsed <= maximum)
			? parsed
			: null;
	};
	const validate = () => {
		const frame = parse(text);
		if (frame === null) {
			setError(
				`Enter HH:MM:SS.FF (${fps} fps), ${
					maximum === undefined
						? `at least ${formatFrame(minimum, fps)}`
						: `between ${formatFrame(minimum, fps)} and ${formatFrame(maximum, fps)}`
				}.`,
			);
			return;
		}
		setError(undefined);
		setText(formatFrame(frame, fps));
	};
	return (
		<TextField
			label={label}
			value={text}
			error={error}
			onFocus={() => {
				focused.current = true;
			}}
			onChange={(event) => {
				const candidate = event.currentTarget.value;
				setText(candidate);
				setError(undefined);
				const frame = parse(candidate);
				if (frame !== null && frame !== value) onChange(frame);
			}}
			onBlur={() => {
				focused.current = false;
				validate();
			}}
			onKeyDown={(event) => {
				if (event.key === "Escape") {
					event.preventDefault();
					event.stopPropagation();
					setText(formatFrame(value, fps));
					setError(undefined);
				} else if (event.key === "Enter") {
					event.preventDefault();
					validate();
				}
			}}
		/>
	);
}
