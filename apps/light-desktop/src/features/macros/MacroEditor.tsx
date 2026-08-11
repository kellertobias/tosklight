import {
	Button,
	FormLayout,
	IconPickerField,
	ModalPortal,
	ModalTitleBar,
	TextArea,
	TextField,
} from "@tosklight/ui";
import { WindowHeader } from "@tosklight/ui/window-kit";
import { forwardRef, useEffect, useId, useRef, useState } from "react";
import type {
	MacroDefinition,
	MacroLineDiagnostic,
	MacrosApiClient,
	MacroSuggestion,
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
	const noticeIsError = controller.notice
		? /conflict|error|fail|refus|unavailable|invalid/iu.test(controller.notice)
		: false;
	return (
		<section
			className="macro-editor"
			aria-label={`Edit Macro ${controller.draft.number}`}
		>
			<MacroEditorHeader {...props} controller={controller} />
			{controller.settingsOpen && (
				<MacroSettings controller={controller} />
			)}
			<MacroSource controller={controller} />
			<MacroDiagnostics
				diagnostics={controller.validation?.diagnostics ?? []}
			/>
			{controller.notice && (
				<p
					role={noticeIsError ? "alert" : "status"}
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
	const [settingsOpen, setSettingsOpen] = useState(false);
	const [cursor, setCursor] = useState(0);
	const [suggestionsOpen, setSuggestionsOpen] = useState(false);
	const [suggestionIndex, setSuggestionIndex] = useState(0);
	const [runLineUndo, setRunLineUndo] = useState<{
		executionId: string;
		line: number;
	} | null>(null);
	const editor = useRef<HTMLTextAreaElement | null>(null);
	const highlightOverlay = useRef<HTMLPreElement | null>(null);
	const pendingCaret = useRef<number | null>(null);
	const validationGeneration = useRef(0);
	const suggestionListId = useId();
	const definitionHelpId = useId();
	const isNew = "isNew" in macro;
	const editDraft = (next: MacroDefinition) => {
		setDraft(next);
		setRunLineUndo(null);
	};

	useEffect(() => {
		if (!showId) return;
		const generation = ++validationGeneration.current;
		setSuggestionsOpen(false);
		const timer = window.setTimeout(() => {
			void api
				.validate(showId, draft.source, cursor)
				.then((next) => {
					if (validationGeneration.current !== generation) return;
					setValidation(next);
					setSuggestionsOpen(next.suggestions.length > 0);
					setSuggestionIndex(0);
				})
				.catch((reason) => {
					if (validationGeneration.current === generation)
						setNotice(String(reason));
				});
		}, 180);
		return () => {
			window.clearTimeout(timer);
			if (validationGeneration.current === generation)
				validationGeneration.current += 1;
		};
	}, [api, cursor, draft.source, showId]);

	useEffect(() => {
		if (pendingCaret.current === null || !editor.current) return;
		editor.current.focus();
		editor.current.setSelectionRange(pendingCaret.current, pendingCaret.current);
		setCursor(pendingCaret.current);
		pendingCaret.current = null;
	}, [draft.source]);

	const insertSuggestion = (suggestion: MacroSuggestion) => {
		const source = draft.source;
		const next =
			source.slice(0, suggestion.replaceStart) +
			suggestion.insertText +
			source.slice(suggestion.replaceEnd);
		pendingCaret.current =
			suggestion.replaceStart + suggestion.insertText.length;
		editDraft({ ...draft, source: next });
		setSuggestionsOpen(false);
	};

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
		setDraft,
		setSavedBody,
		setRevision,
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
		settingsOpen,
		setSettingsOpen,
		cursor,
		setCursor,
		suggestionsOpen,
		setSuggestionsOpen,
		suggestionIndex,
		setSuggestionIndex,
		insertSuggestion,
		suggestionListId,
		definitionHelpId,
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
	setDraft(value: MacroDefinition): void;
	setSavedBody(value: MacroDefinition): void;
	setRevision(value: number): void;
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
		setSavedBody,
		setRevision,
		setNotice,
		setBusy,
	} = options;
	const isNew = "isNew" in macro;
	const save = async () => {
		if (!showId || busy || validation?.valid === false) return;
		setBusy(true);
		setNotice("");
		try {
			const outcome = isNew
				? await api.create(showId, draft)
				: await api.update(showId, macro.id, revision, {
						number: draft.number,
						name: draft.name,
						source: draft.source,
						presentation: draft.presentation,
					});
			setRevision(outcome.object.revision);
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
	return { save, remove };
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
	const awaitCompletion = async (
		started: Awaited<ReturnType<MacrosApiClient["run"]>>,
	) => {
		let completed = started;
		while (["queued", "validating", "running"].includes(completed.state)) {
			await new Promise((resolve) => window.setTimeout(resolve, 25));
			completed = await options.api.execution(
				options.showId ?? "",
				started.execution_id,
			);
		}
		return completed;
	};
	const runMacro = async () => {
		const { showId, macro, api, draft, savedBody, revision, busy } = options;
		if (
			!showId ||
			busy ||
			"isNew" in macro ||
			!sameMacroDefinition(draft, savedBody)
		) {
			options.setNotice("Save this revision before running the Macro.");
			return;
		}
		options.setBusy(true);
		options.setRunLineUndo(null);
		try {
			const completed = await awaitCompletion(
				await api.run(showId, macro.id, {
					source_revision: revision,
					trigger: { type: "editor" },
				}),
			);
			options.setNotice(
				completed.message ?? `Macro ${completed.state}`,
			);
		} catch (reason) {
			options.setNotice(`Run Macro failed: ${String(reason)}`);
		} finally {
			options.setBusy(false);
		}
	};
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
			const completed = await awaitCompletion(started);
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
	return { runMacro, runLine, undoLastRun };
}

function sameMacroDefinition(left: MacroDefinition, right: MacroDefinition) {
	return (
		left.number === right.number &&
		left.name === right.name &&
		left.source === right.source &&
		left.presentation.color === right.presentation.color &&
		left.presentation.icon === right.presentation.icon
	);
}

type MacroEditorController = ReturnType<typeof useMacroEditorController>;

function MacroEditorHeader({
	onClose,
	controller,
}: Pick<MacroEditorProps, "onClose"> & { controller: MacroEditorController }) {
	return (
		<WindowHeader
			title="Macro"
			info={{
				primary: controller.draft.name,
				secondary:
					controller.validation?.valid === false ? "Invalid" : "Command Macro",
			}}
			actions={[
				[
					{ id: "back", label: "← Macros", onClick: onClose },
				],
				[
					{
						id: "run-macro",
						label: (
							<span>
								<span aria-hidden="true">▶</span> Run Macro
							</span>
						),
						ariaLabel: "Run Macro",
						disabled: controller.isNew || controller.busy,
						onClick: () => void controller.runMacro(),
					},
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
				],
				[
					{
						id: "settings",
						label: "Settings",
						onClick: () => controller.setSettingsOpen(true),
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

function MacroSettings({ controller }: { controller: MacroEditorController }) {
	const { draft, editDraft } = controller;
	const close = () => controller.setSettingsOpen(false);
	return (
		<ModalPortal onClose={close}>
			<div
				className="stacked-modal-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && close()
				}
			>
				<section
					className="nested-modal macro-settings-modal"
					role="dialog"
					aria-modal="true"
					aria-label="Macro Settings"
				>
					<ModalTitleBar
						title="Macro Settings"
						actions={
							!controller.isNew ? (
								<Button
									className="danger"
									disabled={controller.busy}
									onClick={() => void controller.remove()}
								>
									Delete Macro
								</Button>
							) : undefined
						}
						closeLabel="Close Macro Settings"
						onClose={close}
					/>
					<div className="macro-settings-content">
						<FormLayout labelPlacement="side">
							<TextField
								label="Name"
								value={draft.name}
								onChange={(event) =>
									editDraft({ ...draft, name: event.target.value })
								}
							/>
							<IconPickerField
								label="Icon"
								value={draft.presentation.icon ?? ""}
								onChange={(icon) =>
									editDraft({
										...draft,
										presentation: {
											...draft.presentation,
											icon: icon || undefined,
										},
									})
								}
							/>
						</FormLayout>
					</div>
				</section>
			</div>
		</ModalPortal>
	);
}

function MacroSource({ controller }: { controller: MacroEditorController }) {
	const diagnostics = controller.validation?.diagnostics ?? [];
	const suggestions = controller.validation?.suggestions ?? [];
	const updateCursor = (target: HTMLTextAreaElement) =>
		controller.setCursor(target.selectionStart);
	const expansions = diagnostics
		.flatMap((diagnostic) => diagnostic.tokens)
		.flatMap((token) => (token.expansion ? [token.expansion] : []))
		.filter((expansion, index, all) => all.indexOf(expansion) === index);
	return (
		<div className="macro-source-editor">
			<LineNumbers source={controller.draft.source} diagnostics={diagnostics} />
			<div className="macro-source-stack">
				<HighlightedSource
					ref={controller.highlightOverlay}
					source={controller.draft.source}
					diagnostics={diagnostics}
					onDefinitionPointerDown={(offset) => {
						controller.editor.current?.focus();
						controller.editor.current?.setSelectionRange(offset, offset);
						controller.setCursor(offset);
					}}
				/>
				<TextArea
					ref={controller.editor}
					aria-label="Macro command lines"
					value={controller.draft.source}
					spellCheck={false}
					aria-autocomplete="list"
					aria-controls={
						controller.suggestionsOpen
							? controller.suggestionListId
							: undefined
					}
					aria-describedby={
						expansions.length ? controller.definitionHelpId : undefined
					}
					aria-expanded={controller.suggestionsOpen}
					aria-activedescendant={
						controller.suggestionsOpen && suggestions.length
							? `${controller.suggestionListId}-${controller.suggestionIndex}`
							: undefined
					}
					onSelect={(event) => updateCursor(event.currentTarget)}
					onClick={(event) => updateCursor(event.currentTarget)}
					onKeyUp={(event) => updateCursor(event.currentTarget)}
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
						if (controller.suggestionsOpen && suggestions.length) {
							if (event.key === "ArrowDown" || event.key === "ArrowUp") {
								event.preventDefault();
								const direction = event.key === "ArrowDown" ? 1 : -1;
								controller.setSuggestionIndex(
									(controller.suggestionIndex + direction + suggestions.length) %
										suggestions.length,
								);
								return;
							}
							if (event.key === "Enter") {
								event.preventDefault();
								controller.insertSuggestion(
									suggestions[controller.suggestionIndex]!,
								);
								return;
							}
							if (event.key === "Escape") {
								event.preventDefault();
								controller.setSuggestionsOpen(false);
								return;
							}
						}
						if (
							(event.metaKey || event.ctrlKey) &&
							event.key.toLowerCase() === "s"
						) {
							event.preventDefault();
							void controller.save();
						}
					}}
				/>
				{controller.suggestionsOpen && suggestions.length > 0 && (
					<div
						id={controller.suggestionListId}
						className="macro-suggestions"
						role="listbox"
						aria-label="Macro command suggestions"
					>
						{suggestions.map((suggestion, index) => (
							<Button
								key={`${suggestion.label}:${suggestion.replaceStart}`}
								id={`${controller.suggestionListId}-${index}`}
								role="option"
								aria-selected={index === controller.suggestionIndex}
								onPointerDown={(event) => event.preventDefault()}
								onClick={() => controller.insertSuggestion(suggestion)}
							>
								<b>{suggestion.label}</b>
								<small>{suggestion.detail}</small>
							</Button>
						))}
					</div>
				)}
				{expansions.length > 0 && (
					<p id={controller.definitionHelpId} className="sr-only">
						Defined command expansions: {expansions.join("; ")}
					</p>
				)}
			</div>
		</div>
	);
}

const HighlightedSource = forwardRef<
	HTMLPreElement,
	{
		source: string;
		diagnostics: MacroLineDiagnostic[];
		onDefinitionPointerDown(offset: number): void;
	}
>(function HighlightedSource(
	{ source, diagnostics, onDefinitionPointerDown },
	ref,
) {
	const lines = source.split("\n");
	const byLine = new Map(
		diagnostics.map((diagnostic) => [diagnostic.line, diagnostic.tokens]),
	);
	const lineOffsets: number[] = [];
	let documentOffset = 0;
	for (const line of lines) {
		lineOffsets.push(documentOffset);
		documentOffset += line.length + 1;
	}
	return (
		<pre ref={ref} className="macro-source-highlight" aria-hidden="true">
			{lines.map((line, lineIndex) => {
				const lineOffset = lineOffsets[lineIndex] ?? 0;
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
							title={token.expansion ? `${line.slice(start, end)} → ${token.expansion}` : undefined}
							onPointerDown={
								token.expansion
									? (event) => {
											event.preventDefault();
											onDefinitionPointerDown(lineOffset + end);
										}
									: undefined
							}
						>
							{line.slice(start, end)}
						</span>,
					);
					cursor = end;
				}
				if (cursor < line.length) fragments.push(line.slice(cursor));
				return (
					<span
						key={`line:${lineIndex}`}
						className={`macro-source-line ${lineIndex % 2 ? "alternate" : ""}`}
					>
						{fragments}
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
