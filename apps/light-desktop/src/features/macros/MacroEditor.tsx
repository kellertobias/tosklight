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
import {
	forwardRef,
	useCallback,
	useEffect,
	useId,
	useRef,
	useState,
} from "react";
import type {
	MacroDefinition,
	MacroLineDiagnostic,
	MacroSuggestion,
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
			{controller.settingsOpen && <MacroSettings controller={controller} />}
			<div
				className={`macro-editor-workspace ${controller.helpOpen ? "has-help" : ""}`}
			>
				<MacroSource controller={controller} />
				{controller.helpOpen && <MacroHelpSidebar />}
			</div>
			<MacroDiagnostics
				diagnostics={controller.visibleDiagnostics}
				settled={controller.validation !== null}
			/>
		</section>
	);
}

function useMacroValidation(
	showId: string | null,
	api: MacrosApiClient,
	source: string,
	cursor: number,
	setNotice: (value: string | null) => void,
) {
	const [validation, setValidation] = useState<MacroValidation | null>(null);
	const [suggestionsOpen, setSuggestionsOpen] = useState(false);
	const [suggestionIndex, setSuggestionIndex] = useState(0);
	const generationRef = useRef(0);
	useEffect(() => {
		if (!showId) return;
		const generation = ++generationRef.current;
		setSuggestionsOpen(false);
		const timer = window.setTimeout(() => {
			void api
				.validate(showId, source, cursor)
				.then((next) => {
					if (generationRef.current !== generation) return;
					setValidation(next);
					setSuggestionsOpen(next.suggestions.length > 0);
					setSuggestionIndex(0);
				})
				.catch((reason) => {
					if (generationRef.current === generation) setNotice(String(reason));
				});
		}, 180);
		return () => {
			window.clearTimeout(timer);
			if (generationRef.current === generation) generationRef.current += 1;
		};
	}, [api, cursor, setNotice, showId, source]);
	return {
		validation,
		setValidation,
		suggestionsOpen,
		setSuggestionsOpen,
		suggestionIndex,
		setSuggestionIndex,
	};
}

function useMacroEditorInput({
	api,
	draft,
	editDraft,
	setCursor,
	setNotice,
	setSuggestionsOpen,
}: {
	api: MacrosApiClient;
	draft: MacroDefinition;
	editDraft(next: MacroDefinition): void;
	setCursor(value: number): void;
	setNotice(value: string | null): void;
	setSuggestionsOpen(value: boolean): void;
}) {
	const editor = useRef<HTMLTextAreaElement | null>(null);
	const pendingCaret = useRef<number | null>(null);
	const instanceId = useRef(`macro-editor:${crypto.randomUUID()}`).current;
	const focusGeneration = useRef(0);
	useEffect(() => {
		if (pendingCaret.current === null || !editor.current) return;
		editor.current.focus();
		editor.current.setSelectionRange(
			pendingCaret.current,
			pendingCaret.current,
		);
		setCursor(pendingCaret.current);
		pendingCaret.current = null;
	}, [draft.source, setCursor]);
	useEffect(
		() => () => {
			focusGeneration.current += 1;
			void api.releaseEditorInput(instanceId).catch(() => undefined);
		},
		[api, instanceId],
	);
	return {
		editor,
		instanceId,
		insertSuggestion(suggestion: MacroSuggestion) {
			const source = draft.source;
			const next =
				source.slice(0, suggestion.replaceStart) +
				suggestion.insertText +
				source.slice(suggestion.replaceEnd);
			pendingCaret.current =
				suggestion.replaceStart + suggestion.insertText.length;
			editDraft({ ...draft, source: next });
			setSuggestionsOpen(false);
		},
		async claim() {
			const generation = ++focusGeneration.current;
			try {
				await api.claimEditorInput(instanceId);
				if (focusGeneration.current !== generation)
					await api.releaseEditorInput(instanceId);
			} catch (reason) {
				if (focusGeneration.current === generation)
					setNotice(
						`Attached desk input remains unavailable: ${String(reason)}`,
					);
			}
		},
		release() {
			focusGeneration.current += 1;
			void api.releaseEditorInput(instanceId).catch(() => undefined);
		},
		apply(action: string) {
			const target = editor.current;
			if (!target || document.activeElement !== target) return;
			const edit = editMacroSourceWithInput(
				draft.source,
				target.selectionStart,
				target.selectionEnd,
				action,
			);
			if (!edit) return;
			pendingCaret.current = edit.caret;
			editDraft({ ...draft, source: edit.source });
		},
	};
}

function useMacroEditorController({
	showId,
	macro,
	api,
	onSaved,
	onClose,
}: MacroEditorProps) {
	const [draft, setDraft] = useState(macro.body);
	const [savedBody, setSavedBody] = useState(macro.body);
	const [revision, setRevision] = useState(macro.revision);
	const [notice, setNotice] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);
	const [settingsOpen, setSettingsOpen] = useState(false);
	const [helpOpen, setHelpOpen] = useState(false);
	const [cursor, setCursor] = useState(0);
	const validationState = useMacroValidation(
		showId,
		api,
		draft.source,
		cursor,
		setNotice,
	);
	const {
		validation,
		setValidation,
		suggestionsOpen,
		setSuggestionsOpen,
		suggestionIndex,
		setSuggestionIndex,
	} = validationState;
	const [runLineUndo, setRunLineUndo] = useState<{
		executionId: string;
		line: number;
	} | null>(null);
	const highlightOverlay = useRef<HTMLPreElement | null>(null);
	const suggestionListId = useId();
	const definitionHelpId = useId();
	const editDraft = (next: MacroDefinition) => {
		if (next.source !== draft.source) setValidation(null);
		setNotice(null);
		setDraft(next);
		setRunLineUndo(null);
	};

	const editorInput = useMacroEditorInput({
		api,
		draft,
		editDraft,
		setCursor,
		setNotice,
		setSuggestionsOpen,
	});
	const editor = editorInput.editor;
	const activeLine = macroLineAtCursor(draft.source, cursor);
	const visibleDiagnostics = (validation?.diagnostics ?? []).filter(
		(diagnostic) =>
			diagnostic.status !== "invalid" || diagnostic.line !== activeLine,
	);
	const activeLineHasInvalidDiagnostic = Boolean(
		validation?.diagnostics.some(
			(diagnostic) =>
				diagnostic.status === "invalid" && diagnostic.line === activeLine,
		),
	);

	const persistence = useMacroPersistence({
		showId,
		macro,
		api,
		onSaved,
		onClose,
		draft,
		savedBody,
		revision,
		validation,
		busy,
		setSavedBody,
		setRevision,
		setNotice,
		setBusy,
	});
	const lineActions = useMacroLineActions({
		showId,
		macroId: persistence.persistedId,
		isNew: persistence.isNew,
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
		status: macroEditorStatus({
			draft,
			savedBody,
			validation,
			notice,
			busy,
			isNew: persistence.isNew,
			visibleDiagnostics,
			activeLineHasInvalidDiagnostic,
		}),
		busy,
		settingsOpen,
		setSettingsOpen,
		helpOpen,
		setHelpOpen,
		visibleDiagnostics,
		cursor,
		setCursor,
		suggestionsOpen,
		setSuggestionsOpen,
		suggestionIndex,
		setSuggestionIndex,
		insertSuggestion: editorInput.insertSuggestion,
		editorInputInstanceId: editorInput.instanceId,
		claimEditorInput: editorInput.claim,
		releaseEditorInput: editorInput.release,
		applyEditorInput: editorInput.apply,
		suggestionListId,
		definitionHelpId,
		runLineUndo,
		editor,
		highlightOverlay,
		editDraft,
		...persistence,
		...lineActions,
	};
}

interface MacroPersistenceOptions extends MacroEditorProps {
	draft: MacroDefinition;
	savedBody: MacroDefinition;
	revision: number;
	validation: MacroValidation | null;
	busy: boolean;
	setSavedBody(value: MacroDefinition): void;
	setRevision(value: number): void;
	setNotice(value: string | null): void;
	setBusy(value: boolean): void;
}

function useMacroPersistence(options: MacroPersistenceOptions) {
	const {
		showId,
		macro,
		api,
		onSaved,
		onClose,
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
	const [isNew, setIsNew] = useState("isNew" in macro);
	const [persistedId, setPersistedId] = useState(macro.id);
	const save = useCallback(async () => {
		if (
			!showId ||
			busy ||
			validation?.valid !== true ||
			sameMacroDefinition(draft, savedBody)
		)
			return;
		setBusy(true);
		setNotice(null);
		try {
			const outcome = isNew
				? await api.create(showId, draft)
				: await api.update(showId, persistedId, revision, {
						number: draft.number,
						name: draft.name,
						source: draft.source,
						presentation: draft.presentation,
					});
			setRevision(outcome.object.revision);
			setPersistedId(outcome.object.id);
			setIsNew(false);
			setSavedBody(draft);
			setNotice("Saved");
			await onSaved();
		} catch (reason) {
			setNotice(
				`Save conflict or failure: ${String(reason)}. Your draft is preserved.`,
			);
		} finally {
			setBusy(false);
		}
	}, [
		api,
		busy,
		draft,
		isNew,
		onSaved,
		persistedId,
		revision,
		savedBody,
		setBusy,
		setNotice,
		setRevision,
		setSavedBody,
		showId,
		validation?.valid,
	]);
	useEffect(() => {
		if (
			busy ||
			validation?.valid !== true ||
			sameMacroDefinition(draft, savedBody)
		)
			return;
		const timer = window.setTimeout(() => void save(), 500);
		return () => window.clearTimeout(timer);
	}, [busy, draft, save, savedBody, validation?.valid]);
	const remove = async () => {
		if (
			!showId ||
			isNew ||
			!window.confirm(`Delete Macro ${draft.number} · ${draft.name}?`)
		)
			return;
		setBusy(true);
		try {
			await api.delete(showId, persistedId, revision);
			await onSaved();
			onClose();
		} catch (reason) {
			setNotice(String(reason));
		} finally {
			setBusy(false);
		}
	};
	return { save, remove, isNew, persistedId };
}

interface MacroLineOptions extends Pick<MacroEditorProps, "showId" | "api"> {
	macroId: string;
	isNew: boolean;
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
		const { showId, macroId, isNew, api, draft, savedBody, revision, busy } =
			options;
		if (!showId || busy || isNew || !sameMacroDefinition(draft, savedBody)) {
			options.setNotice("Save this revision before running the Macro.");
			return;
		}
		options.setBusy(true);
		options.setRunLineUndo(null);
		try {
			const completed = await awaitCompletion(
				await api.run(showId, macroId, {
					source_revision: revision,
					trigger: { type: "editor" },
				}),
			);
			options.setNotice(completed.message ?? `Macro ${completed.state}`);
		} catch (reason) {
			options.setNotice(`Run Macro failed: ${String(reason)}`);
		} finally {
			options.setBusy(false);
		}
	};
	const runLine = async () => {
		const { showId, macroId, isNew, api, draft, savedBody, revision, editor } =
			options;
		if (!showId || isNew || draft.source !== savedBody.source) {
			options.setNotice("Save this revision before running one of its lines.");
			return;
		}
		const line = draft.source
			.slice(0, editor.current?.selectionStart ?? 0)
			.split("\n").length;
		try {
			options.setRunLineUndo(null);
			const started = await api.runLine(showId, macroId, {
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

function macroEditorStatus({
	draft,
	savedBody,
	validation,
	notice,
	busy,
	isNew,
	visibleDiagnostics,
	activeLineHasInvalidDiagnostic,
}: {
	draft: MacroDefinition;
	savedBody: MacroDefinition;
	validation: MacroValidation | null;
	notice: string | null;
	busy: boolean;
	isNew: boolean;
	visibleDiagnostics: MacroLineDiagnostic[];
	activeLineHasInvalidDiagnostic: boolean;
}) {
	if (busy) return { text: "Saving…", error: false };
	if (notice) {
		return {
			text: notice,
			error: /conflict|error|fail|refus|unavailable|invalid/iu.test(notice),
		};
	}
	if (validation === null)
		return { text: "Checking command line…", error: false };
	if (visibleDiagnostics.some((diagnostic) => diagnostic.status !== "valid"))
		return { text: "Command line needs attention", error: true };
	if (activeLineHasInvalidDiagnostic)
		return { text: "Editing current line…", error: false };
	if (!validation.valid)
		return { text: "Command line needs attention", error: true };
	if (!sameMacroDefinition(draft, savedBody))
		return { text: "Autosave pending", error: false };
	if (isNew) return { text: "Edit the Macro to create it", error: false };
	return { text: "Saved", error: false };
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
				secondary: (
					<span
						className="macro-editor-status"
						role={controller.status.error ? "alert" : "status"}
					>
						{controller.status.text}
					</span>
				),
			}}
			groups={[
				{
					id: "macro-navigation",
					actions: [{ id: "back", label: "← Macros", onPress: onClose }],
				},
				{
					id: "macro-run",
					actions: [
						{
							id: "run-macro",
							label: (
								<span>
									<span aria-hidden="true">▶</span> Run Macro
								</span>
							),
							ariaLabel: "Run Macro",
							disabled: controller.isNew || controller.busy,
							onPress: () => void controller.runMacro(),
						},
						{
							id: "run-line",
							label: "Run line",
							onPress: () => void controller.runLine(),
						},
						{
							id: "undo-run-line",
							label: "Undo last run",
							disabled: !controller.runLineUndo || controller.busy,
							onPress: () => void controller.undoLastRun(),
						},
					],
				},
				{
					id: "macro-help",
					actions: [
						{
							id: "toggle-help",
							icon: <span aria-hidden="true">?</span>,
							ariaLabel: "Toggle Macro help",
							active: controller.helpOpen,
							onPress: () => controller.setHelpOpen(!controller.helpOpen),
						},
					],
				},
			]}
			settings
			onSettings={() => controller.setSettingsOpen(true)}
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
						groups={
							!controller.isNew
								? [
										{
											id: "delete",
											actions: [
												{
													id: "delete",
													label: "Delete Macro",
													variant: "danger",
													disabled: controller.busy,
													onPress: () => void controller.remove(),
												},
											],
										},
									]
								: undefined
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

export const MACRO_HELP_COMMANDS = [
	["FIXTURE / GROUP", "Build the target selection."],
	["AT / PRESET", "Apply a value or Preset to the current target."],
	["CUE / PLAYBACK", "Address Cue and Playback operations."],
	["RECORD / UPDATE", "Store or update through the desk command grammar."],
	["DELETE / MOVE / COPY", "Change stored show objects."],
	["SET", "Open or address configuration."],
	["DEFINE _name …", "Define a reusable command expansion."],
	[
		"RESTORE SELECTION",
		"Restore the concrete ordered initiating selection captured when this Macro run began.",
	],
] as const;

function MacroHelpSidebar() {
	return (
		<aside className="macro-help-sidebar" aria-label="Macro Editor help">
			<header>
				<strong>Macro Editor help</strong>
			</header>
			<p>
				Write one desk command per line, or separate commands with a semicolon.
				The last semicolon is optional.
			</p>
			<p>
				Validation follows the caret: an unfinished current line stays neutral
				while you type. Move to another line to see any remaining error.
				Suggestions can complete the command at the caret.
			</p>
			<p>
				Macros autosave only after the complete source validates.{" "}
				<b>Run line</b>
				runs the saved line at the caret; <b>Run Macro</b> runs the saved Macro
				from top to bottom.
			</p>
			<dl>
				{MACRO_HELP_COMMANDS.map(([command, detail]) => (
					<div key={command}>
						<dt>
							<code>{command}</code>
						</dt>
						<dd>{detail}</dd>
					</div>
				))}
			</dl>
		</aside>
	);
}

function macroLineAtCursor(source: string, cursor: number) {
	const boundedCursor = Math.max(0, Math.min(cursor, source.length));
	return source.slice(0, boundedCursor).split("\n").length;
}

function MacroSource({ controller }: { controller: MacroEditorController }) {
	const diagnostics = controller.visibleDiagnostics;
	const suggestions = controller.validation?.suggestions ?? [];
	const updateCursor = (target: HTMLTextAreaElement) =>
		controller.setCursor(target.selectionStart);
	const expansions = diagnostics
		.flatMap((diagnostic) => diagnostic.tokens)
		.flatMap((token) => (token.expansion ? [token.expansion] : []))
		.filter((expansion, index, all) => all.indexOf(expansion) === index);
	useEffect(() => {
		const routeInput = (event: Event) => {
			const payload = (
				event as CustomEvent<{ instance_id?: string; action?: string }>
			).detail;
			if (
				payload?.instance_id === controller.editorInputInstanceId &&
				payload.action
			)
				controller.applyEditorInput(payload.action);
		};
		window.addEventListener("light:macro-editor-input", routeInput);
		return () =>
			window.removeEventListener("light:macro-editor-input", routeInput);
	}, [controller]);
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
					onFocus={() => void controller.claimEditorInput()}
					onBlur={controller.releaseEditorInput}
					value={controller.draft.source}
					spellCheck={false}
					aria-autocomplete="list"
					aria-controls={
						controller.suggestionsOpen ? controller.suggestionListId : undefined
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
									(controller.suggestionIndex +
										direction +
										suggestions.length) %
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

export function editMacroSourceWithInput(
	source: string,
	selectionStart: number,
	selectionEnd: number,
	action: string,
): { source: string; caret: number } | null {
	const start = Math.max(0, Math.min(selectionStart, source.length));
	const end = Math.max(start, Math.min(selectionEnd, source.length));
	if (action === "backspace") {
		const removeStart = start === end ? Math.max(0, start - 1) : start;
		return {
			source: source.slice(0, removeStart) + source.slice(end),
			caret: removeStart,
		};
	}
	const insert = macroInputText(action);
	if (insert === null) return null;
	return {
		source: source.slice(0, start) + insert + source.slice(end),
		caret: start + insert.length,
	};
}

function macroInputText(action: string): string | null {
	const digit = action.match(/^digit-([0-9])$/)?.[1];
	if (digit) return digit;
	const tokens: Record<string, string> = {
		at: " AT ",
		clear: "CLEAR ",
		cpy: "COPY ",
		cue: "CUE ",
		del: "DELETE ",
		delete: "DELETE ",
		diff: "DIFF ",
		div: " DIV ",
		dot: ".",
		enter: "\n",
		group: "GROUP ",
		minus: " - ",
		mov: "MOVE ",
		off: "OFF ",
		playback: "PLAYBACK ",
		plus: " + ",
		preload: "PRELOAD ",
		record: "RECORD ",
		set: "SET ",
		thru: " THRU ",
		time: " TIME ",
		undo: "UNDO ",
	};
	return tokens[action] ?? null;
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
							title={
								token.expansion
									? `${line.slice(start, end)} → ${token.expansion}`
									: undefined
							}
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
	settled,
}: {
	diagnostics: MacroLineDiagnostic[];
	settled: boolean;
}) {
	if (!settled) return null;
	const messages = diagnostics.filter(
		(diagnostic) => diagnostic.status !== "valid",
	);
	if (!messages.length) return null;
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
