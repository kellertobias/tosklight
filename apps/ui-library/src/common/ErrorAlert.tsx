import { createElement, useEffect, useState, type HTMLAttributes } from "react";
import { Button } from "./controls/foundation";

/** Keep diagnostic data intact when an exception crosses into string-based UI state. */
export function formatErrorDetails(error: unknown): string {
	const seen = new Set<object>();
	function format(value: unknown): string {
		if (value === null || typeof value !== "object") return String(value);
		if (seen.has(value)) return "[Circular error]";
		seen.add(value);
		if (value instanceof Error) {
			const message = [value.message, value.stack].filter(Boolean).join("\n");
			const details = Object.entries(value)
				.filter(([key]) => !["name", "message", "stack", "cause"].includes(key))
				.map(([key, detail]) => `${key}: ${format(detail)}`);
			if (value.cause !== undefined) details.push(`Caused by: ${format(value.cause)}`);
			if (value instanceof AggregateError) {
				for (const [index, nested] of Array.from(value.errors).entries()) details.push(`Error ${index + 1}: ${format(nested)}`);
			}
			return [message, ...details].join("\n");
		}
		return Object.entries(value).map(([key, detail]) => `${key}: ${format(detail)}`).join("\n");
	}
	return format(error);
}

/** Read the entire diagnostic, including collapsed details, without action labels. */
export function errorAlertText(element: HTMLElement): string {
	const clone = element.cloneNode(true) as HTMLElement;
	clone.querySelectorAll("button, input, select, textarea, [data-error-copy-exclude]").forEach((node) => node.remove());
	clone.querySelectorAll("p, pre, div, li, h1, h2, h3, strong, small, summary, br").forEach((node) => node.append("\n"));
	return clone.textContent?.trim() ?? "";
}

async function copyText(text: string) {
	try {
		if (navigator.clipboard?.writeText) {
			await navigator.clipboard.writeText(text);
			return;
		}
	} catch {
		// Local HTTP desk screens may require the user-gesture clipboard fallback.
	}
	const selection = document.getSelection();
	const ranges = selection ? Array.from({ length: selection.rangeCount }, (_, i) => selection.getRangeAt(i).cloneRange()) : [];
	const focused = document.activeElement as HTMLElement | null;
	const field = document.createElement("textarea");
	field.value = text;
	field.style.cssText = "position:fixed;opacity:0;pointer-events:none";
	document.body.append(field);
	field.select();
	try {
		if (!document.execCommand?.("copy")) throw new Error("Clipboard access failed. Select the error text and copy it manually.");
	} finally {
		field.remove();
		focused?.focus({ preventScroll: true });
		selection?.removeAllRanges();
		for (const range of ranges) selection?.addRange(range);
	}
}

export function CopyErrorButton({ text }: { text?: string }) {
	const [state, setState] = useState<"idle" | "copied" | "failed">("idle");
	useEffect(() => setState("idle"), [text]);
	const label = state === "failed" ? "Copy failed; retry copying error" : state === "copied" ? "Error copied; copy again" : "Copy error";
	return <Button iconOnly className="ui-error-copy" aria-label={label} title={label}
		onClick={(event) => {
			event.stopPropagation();
			const alert = event.currentTarget.closest<HTMLElement>("[data-error-alert]");
			void copyText(text ?? (alert ? errorAlertText(alert) : ""))
				.then(() => setState("copied"), () => setState("failed"));
		}}>
		<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
			{state === "copied" ? <path d="m5 12 4 4L19 6" /> : state === "failed" ? <><path d="M12 5v9" /><circle cx="12" cy="18" r="1" /></> : <><rect x="8" y="8" width="12" height="13" rx="2" /><path d="M16 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h3" /></>}
		</svg>
	</Button>;
}

type ErrorAlertProps = HTMLAttributes<HTMLElement> & {
	as?: "p" | "div" | "span" | "small" | "aside" | "output" | "pre" | "article" | "li" | "section";
	copyText?: string;
};

/** Preserve each surface's markup and layout while guaranteeing a copy action. */
export function ErrorAlert({ as = "div", children, copyText, ...props }: ErrorAlertProps) {
	return createElement(as, { role: "alert", ...props, "data-error-alert": true }, children, ("role" in props && props.role !== "alert" && props.role !== "alertdialog") ? null : <CopyErrorButton text={copyText ?? (typeof children === "string" ? children : undefined)} />);
}
