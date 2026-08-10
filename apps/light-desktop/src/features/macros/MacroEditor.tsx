import { Button, TextArea, TextField } from "@tosklight/ui";
import { WindowHeader } from "@tosklight/ui/window-kit";
import { forwardRef, useEffect, useRef, useState } from "react";
import type {
	MacroDefinition,
	MacroLineDiagnostic,
	MacrosApiClient,
	MacroValidation,
} from "../../api/client/macros";
import type { VersionedObject } from "../../api/types";

export interface NewMacro {
	id: string;
	revision: 0;
	body: MacroDefinition;
	isNew: true;
}

interface MacroEditorProps {
	showId: string | null;
	macro: VersionedObject<MacroDefinition> | NewMacro;
	api: MacrosApiClient;
	onClose(): void;
	onSaved(): Promise<void>;
}

export function MacroEditor(props: MacroEditorProps) {
	const controller = useMacroEditorController(props);
	return (
		<section
			className="macro-editor"
			aria-label={`Edit Macro ${controller.draft.number}`}
		>
			<MacroEditorHeader {...props} controller={controller} />
			<MacroIdentity controller={controller} />
			<MacroSource controller={controller} />
			<MacroDiagnostics
				diagnostics={controller.validation?.diagnostics ?? []}
			/>
			{controller.notice && (
				<p
					role={
						controller.notice.includes("failure") ||
						controller.notice.includes("refused")
							? "alert"
							: "status"
					}
				>
					{controller.notice}
				</p>
			)}
		</section>
	);
}

function useMacroEditorController({
	showId,
	macro,
	api,
	onSaved,
}: MacroEditorProps) {
	const [draft, setDraft] = useState(macro.body);
	const [savedBody, setSavedBody] = useState(macro.body);
	const [revision, setRevision] = useState(macro.revision);
	const [validation, setValidation] = useState<MacroValidation | null>(null);
	const [notice, setNotice] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);
	const [undo, setUndo] = useState<{
		body: MacroDefinition;
		revision: number;
	} | null>(null);
	const [runLineUndo, setRunLineUndo] = useState<{
		executionId: string;
		line: number;
	} | null>(null);
	const editor = useRef<HTMLTextAreaElement | null>(null);
	const highlightOverlay = useRef<HTMLPreElement | null>(null);
	const isNew = "isNew" in macro;
	const editDraft = (next: MacroDefinition) => {
		setDraft(next);
		setRunLineUndo(null);
	};

	useEffect(() => {
		if (!showId) return;
		const timer = window.setTimeout(() => {
			void api
				.validate(showId, draft.source)
				.then(setValidation)
				.catch((reason) => setNotice(String(reason)));
		}, 180);
		return () => window.clearTimeout(timer);
	}, [api, draft.source, showId]);

	const persistence = useMacroPersistence({
		showId,
		macro,
		api,
		onSaved,
		draft,
		savedBody,
		revision,
		validation,
		busy,
		undo,
		setDraft,
		setSavedBody,
		setRevision,
		setUndo,
		setRunLineUndo,
		setNotice,
		setBusy,
	});
	const lineActions = useMacroLineActions({
		showId,
		macro,
		api,
		draft,
		savedBody,
		revision,
		busy,
		runLineUndo,
		editor,
		setRunLineUndo,
		setNotice,
		setBusy,
	});

	return {
		draft,
		validation,
		notice,
		busy,
		undo,
		runLineUndo,
		isNew,
		editor,
		highlightOverlay,
		editDraft,
		...persistence,
		...lineActions,
	};
}

interface MacroPersistenceOptions extends Omit<MacroEditorProps, "onClose"> {
	draft: MacroDefinition;
	savedBody: MacroDefinition;
	revision: number;
	validation: MacroValidation | null;
	busy: boolean;
	undo: { body: MacroDefinition; revision: number } | null;
	setDraft(value: MacroDefinition): void;
	setSavedBody(value: MacroDefinition): void;
	setRevision(value: number): void;
	setUndo(value: { body: MacroDefinition; revision: number } | null): void;
	setRunLineUndo(value: null): void;
	setNotice(value: string): void;
	setBusy(value: boolean): void;
}

function useMacroPersistence(options: MacroPersistenceOptions) {
	const {
		showId,
		macro,
		api,
		onSaved,
		draft,
		savedBody,
		revision,
		validation,
		busy,
		undo,
		setDraft,
		setSavedBody,
		setRevision,
		setUndo,
		setRunLineUndo,
		setNotice,
		setBusy,
	} = options;
	const isNew = "isNew" in macro;
	const save = async () => {
		if (!showId || busy || validation?.valid === false) return;
		setBusy(true);
		setNotice("");
		try {
			const before = savedBody;
			const outcome = isNew
				? await api.create(showId, draft)
				: await api.update(showId, macro.id, revision, {
						number: draft.number,
						name: draft.name,
						source: draft.source,
						presentation: draft.presentation,
					});
			setRevision(outcome.object.revision);
			setUndo({ body: before, revision: outcome.object.revision });
			setSavedBody(draft);
			setNotice("Saved");
			if (isNew) await onSaved();
		} catch (reason) {
			setNotice(
				`Save conflict or failure: ${String(reason)}. Your draft is preserved.`,
			);
		} finally {
			setBusy(false);
		}
	};
	const undoSave = async () => {
		if (!showId || !undo || busy) return;
		setBusy(true);
		try {
			const outcome = await api.update(showId, macro.id, undo.revision, {
				number: undo.body.number,
				name: undo.body.name,
				source: undo.body.source,
				presentation: undo.body.presentation,
			});
			setDraft(undo.body);
			setRunLineUndo(null);
			setSavedBody(undo.body);
			setRevision(outcome.object.revision);
			setUndo(null);
			setNotice("Last save undone as a new guarded revision");
		} catch (reason) {
			setNotice(
				`Undo refused because the saved Macro changed: ${String(reason)}`,
			);
		} finally {
			setBusy(false);
		}
	};
	const copy = async () => {
		if (!showId || isNew || busy) return;
		const requested = window.prompt(
			"Copy to Macro number",
			String(draft.number + 1),
		);
		if (requested == null) return;
		const number = Number(requested);
		if (!Number.isInteger(number) || number < 1 || number > 65_535) {
			setNotice("Enter a Macro number from 1 to 65535.");
			return;
		}
		setBusy(true);
		try {
			await api.create(showId, {
				...savedBody,
				id: crypto.randomUUID(),
				number,
				name: `${savedBody.name} Copy`,
			});
			setNotice(`Copied to Macro ${number}`);
		} catch (reason) {
			setNotice(`Copy failed: ${String(reason)}`);
		} finally {
			setBusy(false);
		}
	};
	const remove = async () => {
		if (
			!showId ||
			isNew ||
			!window.confirm(`Delete Macro ${draft.number} · ${draft.name}?`)
		)
			return;
		setBusy(true);
		try {
			await api.delete(showId, macro.id, revision);
			await onSaved();
		} catch (reason) {
			setNotice(String(reason));
		} finally {
			setBusy(false);
		}
	};
	return { save, undoSave, copy, remove };
}

interface MacroLineOptions
	extends Pick<MacroEditorProps, "showId" | "macro" | "api"> {
	draft: MacroDefinition;
	savedBody: MacroDefinition;
	revision: number;
	busy: boolean;
	runLineUndo: { executionId: string; line: number } | null;
	editor: React.RefObject<HTMLTextAreaElement | null>;
	setRunLineUndo(value: { executionId: string; line: number } | null): void;
	setNotice(value: string): void;
	setBusy(value: boolean): void;
}

function useMacroLineActions(options: MacroLineOptions) {
	const runLine = async () => {
		const { showId, macro, api, draft, savedBody, revision, editor } = options;
		if (!showId || "isNew" in macro || draft.source !== savedBody.source) {
			options.setNotice("Save this revision before running one of its lines.");
			return;
		}
		const line = draft.source
			.slice(0, editor.current?.selectionStart ?? 0)
			.split("\n").length;
		try {
			options.setRunLineUndo(null);
			const started = await api.runLine(showId, macro.id, {
				source_revision: revision,
				line,
			});
			let completed = started;
			while (["queued", "validating", "running"].includes(completed.state)) {
				await new Promise((resolve) => window.setTimeout(resolve, 25));
				completed = await api.execution(showId, started.execution_id);
			}
			if (completed.state === "succeeded") {
				options.setRunLineUndo({ executionId: completed.execution_id, line });
				options.setNotice(
					`Ran line ${line}; Undo last run is available until another change`,
				);
			} else {
				options.setNotice(
					completed.message ?? `Line ${line} ${completed.state}`,
				);
			}
		} catch (reason) {
			options.setNotice(String(reason));
		}
	};
	const undoLastRun = async () => {
		const { showId, api, runLineUndo, busy } = options;
		if (!showId || !runLineUndo || busy) return;
		options.setBusy(true);
		try {
			const outcome = await api.undoRunLine(showId, runLineUndo.executionId);
			options.setRunLineUndo(null);
			options.setNotice(outcome.message);
		} catch (reason) {
			options.setRunLineUndo(null);
			options.setNotice(`Undo last run is unavailable: ${String(reason)}`);
		} finally {
			options.setBusy(false);
		}
	};
	return { runLine, undoLastRun };
}

type MacroEditorController = ReturnType<typeof useMacroEditorController>;

function MacroEditorHeader({
	onClose,
	controller,
}: Pick<MacroEditorProps, "onClose"> & { controller: MacroEditorController }) {
	return (
		<WindowHeader
			title={`Macro ${controller.draft.number} · ${controller.draft.name}`}
			info={{
				primary:
					controller.validation?.valid === false ? "Invalid" : "Command Macro",
			}}
			actions={[
				[
					{ id: "back", label: "Back", onClick: onClose },
					{
						id: "run-line",
						label: "Run line",
						onClick: () => void controller.runLine(),
					},
					{
						id: "undo-run-line",
						label: "Undo last run",
						disabled: !controller.runLineUndo || controller.busy,
						onClick: () => void controller.undoLastRun(),
					},
					{
						id: "copy",
						label: "Copy",
						disabled: controller.isNew || controller.busy,
						onClick: () => void controller.copy(),
					},
					{
						id: "undo",
						label: "Undo save",
						disabled: !controller.undo,
						onClick: () => void controller.undoSave(),
					},
					{
						id: "save",
						label: controller.busy ? "Saving…" : "Save",
						disabled: controller.busy || controller.validation?.valid === false,
						onClick: () => void controller.save(),
					},
				],
			]}
		/>
	);
}

function MacroIdentity({ controller }: { controller: MacroEditorController }) {
	const { draft, editDraft } = controller;
	return (
		<div className="macro-editor-identity">
			<TextField
				label="Number"
				value={String(draft.number)}
				onChange={(event) =>
					editDraft({ ...draft, number: Number(event.target.value) })
				}
			/>
			<TextField
				label="Name"
				value={draft.name}
				onChange={(event) => editDraft({ ...draft, name: event.target.value })}
			/>
			<TextField
				label="Icon"
				value={draft.presentation.icon ?? ""}
				onChange={(event) =>
					editDraft({
						...draft,
						presentation: {
							...draft.presentation,
							icon: event.target.value || undefined,
						},
					})
				}
			/>
			{!controller.isNew && (
				<Button
					className="danger"
					disabled={controller.busy}
					onClick={() => void controller.remove()}
				>
					Delete
				</Button>
			)}
		</div>
	);
}

function MacroSource({ controller }: { controller: MacroEditorController }) {
	const diagnostics = controller.validation?.diagnostics ?? [];
	return (
		<div className="macro-source-editor">
			<LineNumbers source={controller.draft.source} diagnostics={diagnostics} />
			<div className="macro-source-stack">
				<HighlightedSource
					ref={controller.highlightOverlay}
					source={controller.draft.source}
					diagnostics={diagnostics}
				/>
				<TextArea
					ref={controller.editor}
					aria-label="Macro command lines"
					value={controller.draft.source}
					spellCheck={false}
					onScroll={(event) => {
						if (!controller.highlightOverlay.current) return;
						controller.highlightOverlay.current.scrollTop =
							event.currentTarget.scrollTop;
						controller.highlightOverlay.current.scrollLeft =
							event.currentTarget.scrollLeft;
					}}
					onChange={(event) =>
						controller.editDraft({
							...controller.draft,
							source: event.target.value,
						})
					}
					onKeyDown={(event) => {
						if (
							(event.metaKey || event.ctrlKey) &&
							event.key.toLowerCase() === "s"
						) {
							event.preventDefault();
							void controller.save();
						}
					}}
				/>
			</div>
		</div>
	);
}

const HighlightedSource = forwardRef<
	HTMLPreElement,
	{ source: string; diagnostics: MacroLineDiagnostic[] }
>(function HighlightedSource({ source, diagnostics }, ref) {
	const lines = source.split("\n");
	const byLine = new Map(
		diagnostics.map((diagnostic) => [diagnostic.line, diagnostic.tokens]),
	);
	return (
		<pre ref={ref} className="macro-source-highlight" aria-hidden="true">
			{lines.map((line, lineIndex) => {
				const tokens = [...(byLine.get(lineIndex + 1) ?? [])].sort(
					(left, right) => left.start - right.start,
				);
				const fragments: React.ReactNode[] = [];
				let cursor = 0;
				for (const [tokenIndex, token] of tokens.entries()) {
					const start = Math.max(cursor, Math.min(line.length, token.start));
					const end = Math.max(start, Math.min(line.length, token.end));
					if (start > cursor) fragments.push(line.slice(cursor, start));
					fragments.push(
						<span
							key={`${lineIndex}:${tokenIndex}:${start}`}
							className={`macro-token-${token.kind}`}
						>
							{line.slice(start, end)}
						</span>,
					);
					cursor = end;
				}
				if (cursor < line.length) fragments.push(line.slice(cursor));
				return (
					<span key={`line:${lineIndex}`}>
						{fragments}
						{lineIndex < lines.length - 1 ? "\n" : null}
					</span>
				);
			})}
		</pre>
	);
});

function LineNumbers({
	source,
	diagnostics,
}: {
	source: string;
	diagnostics: MacroLineDiagnostic[];
}) {
	const byLine = new Map(
		diagnostics.map((diagnostic) => [diagnostic.line, diagnostic]),
	);
	return (
		<ol className="macro-line-numbers" aria-hidden="true">
			{source.split("\n").map((_, index) => {
				const line = index + 1;
				return (
					<li key={line} className={byLine.get(line)?.status ?? "blank"}>
						{line}
					</li>
				);
			})}
		</ol>
	);
}

function MacroDiagnostics({
	diagnostics,
}: {
	diagnostics: MacroLineDiagnostic[];
}) {
	const messages = diagnostics.filter(
		(diagnostic) => diagnostic.status !== "valid",
	);
	if (!messages.length)
		return (
			<p className="macro-validation-ok" role="status">
				All command lines are valid.
			</p>
		);
	return (
		<ul className="macro-diagnostics">
			{messages.map((diagnostic) => (
				<li key={diagnostic.line}>
					<b>Line {diagnostic.line}</b> · {diagnostic.message}
				</li>
			))}
		</ul>
	);
}
