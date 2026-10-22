import { useState, type ReactNode } from "react";
import "./TimecodeInspectorSection.css";

/** Operator sections retain their expansion while their timing values update. */
export function TimecodeInspectorSection({ title, children, defaultOpen = false }: {
	title: string;
	children: ReactNode;
	defaultOpen?: boolean;
}) {
	const [open, setOpen] = useState(defaultOpen);
	return (
		<details className="timecode-inspector-section" open={open}
			onToggle={(event) => setOpen(event.currentTarget.open)}>
			<summary>{title}</summary>
			<div className="timecode-inspector-section-content">{children}</div>
		</details>
	);
}
