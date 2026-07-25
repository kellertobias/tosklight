import {
	useConnectionStatus,
	useServerError,
} from "../../features/shellStatus/ShellStatusState";
import {
	type Dispatch,
	type SetStateAction,
	useEffect,
	useRef,
	useState,
} from "react";
import { useApp } from "../../state/AppContext";
import { ServerErrorNotice } from "../shell/ServerErrorNotice";
import { useDeskLockActions } from "../../features/deskLock/DeskLockActionsProvider";
import {
	useBootstrapSnapshot,
	useSessionSnapshot,
} from "../../features/deskSnapshot/DeskSnapshotState";
import { useShowLifecycle } from "../../features/showLifecycle/ShowLifecycleContext";
import { useScreens } from "../../features/screens/ScreensContext";
import { Button, ModalTitleBar } from "../common";
import type {
	MvrExportPreview,
	MvrImportPreview,
	ShowEntry,
	ShowRevision,
} from "../../api/types";
import { useShowIndicator } from "../shell/showIndicator";
import { screenForAddAction } from "../setup/screenConfiguration";
import { useDesktopBridge } from "../../platform/desktop";
import { useSelectiveImport } from "../../features/selectiveImport/SelectiveImportContext";
import { QuickSetupDialogs } from "./QuickSetupDialogs";

interface QuickSetupKeyboardOptions {
	enabled: boolean;
	revisionOpen: boolean;
	saveAsOpen: boolean;
	changeUserOpen: boolean;
	newUserName: string;
	closeTopLayer: () => void;
	saveNamedRevision: () => Promise<void>;
	saveAs: () => Promise<void>;
	createUser: (name: string) => Promise<void>;
	setRevisionName: Dispatch<SetStateAction<string>>;
	setShowName: Dispatch<SetStateAction<string>>;
	setNewUserName: Dispatch<SetStateAction<string>>;
}

function useQuickSetupKeyboard(options: QuickSetupKeyboardOptions) {
	const optionsRef = useRef(options);
	optionsRef.current = options;
	useEffect(() => {
		if (!options.enabled) return;
		const handle = (event: KeyboardEvent) => {
			if (document.querySelector(".ui-input-modal-layer")) return;
			const current = optionsRef.current;
			if (event.key === "Escape") {
				event.preventDefault();
				event.stopImmediatePropagation();
				current.closeTopLayer();
				return;
			}
			if (
				!current.revisionOpen &&
				!current.saveAsOpen &&
				!current.changeUserOpen
			)
				return;
			event.preventDefault();
			event.stopImmediatePropagation();
			if (event.key === "Enter") {
				if (current.revisionOpen) void current.saveNamedRevision();
				else if (current.saveAsOpen) void current.saveAs();
				else if (current.newUserName.trim())
					void current.createUser(current.newUserName.trim());
				return;
			}
			const setValue = current.revisionOpen
				? current.setRevisionName
				: current.saveAsOpen
					? current.setShowName
					: current.setNewUserName;
			if (event.key === "Backspace") setValue((value) => value.slice(0, -1));
			else if (event.key.length === 1) setValue((value) => value + event.key);
		};
		window.addEventListener("keydown", handle, true);
		return () => window.removeEventListener("keydown", handle, true);
	}, [options.enabled, optionsRef]);
}

interface ShowRevisionControllerOptions {
	enabled: boolean;
	activeShowId?: string;
	revisionName: string;
	shows: ShowEntry[];
	listShowRevisions: (showId: string) => Promise<ShowRevision[]>;
	saveShowRevision: (name: string) => Promise<ShowRevision | null>;
	openShowRevision: (showId: string, revision: number) => Promise<boolean>;
	setRevisionName: Dispatch<SetStateAction<string>>;
	setRevisionOpen: Dispatch<SetStateAction<boolean>>;
	setLoadOpen: Dispatch<SetStateAction<boolean>>;
}

function useShowRevisionController(options: ShowRevisionControllerOptions) {
	const optionsRef = useRef(options);
	optionsRef.current = options;
	const [byShow, setByShow] = useState<Record<string, ShowRevision[]>>({});
	useEffect(() => {
		if (!options.enabled || !options.activeShowId) return;
		const showId = options.activeShowId;
		void optionsRef.current
			.listShowRevisions(showId)
			.then((revisions) =>
				setByShow((current) => ({ ...current, [showId]: revisions })),
			);
	}, [options.enabled, options.activeShowId, optionsRef]);
	const saveNamed = async (value = optionsRef.current.revisionName) => {
		const current = optionsRef.current;
		const name = value.trim();
		if (!name || !current.activeShowId) return;
		const showId = current.activeShowId;
		const revision = await current.saveShowRevision(name);
		if (!revision) return;
		setByShow((loaded) => ({
			...loaded,
			[showId]: [revision, ...(loaded[showId] ?? [])],
		}));
		current.setRevisionName("");
		current.setRevisionOpen(false);
	};
	const openLoadMenu = async () => {
		const current = optionsRef.current;
		current.setLoadOpen(true);
		const entries = await Promise.all(
			current.shows.map(
				async (show) =>
					[show.id, await current.listShowRevisions(show.id)] as const,
			),
		);
		setByShow(Object.fromEntries(entries));
	};
	const loadNamed = async (showId: string, revision: number) => {
		const current = optionsRef.current;
		if (await current.openShowRevision(showId, revision))
			current.setLoadOpen(false);
	};
	return {
		activeRevisions: options.activeShowId
			? (byShow[options.activeShowId] ?? [])
			: [],
		byShow,
		loadNamed,
		openLoadMenu,
		saveNamed,
	};
}

function AddScreenAction({
	layout,
	onAdded,
}: {
	layout: Parameters<typeof screenForAddAction>[1];
	onAdded: () => void;
}) {
	const screens = useScreens();
	const add = async () => {
		await screens.saveScreen(
			screenForAddAction(screens.screens?.screens ?? [], layout),
		);
		onAdded();
	};
	return (
		<Button onClick={() => void add()}>
			<span aria-hidden="true">▣</span> Add Screen
		</Button>
	);
}

/** State and workflow for the MVR import/export flows, kept outside QuickSetupModal for size. */
function useMvrController(
	lifecycle: ReturnType<typeof useShowLifecycle>,
	bootstrap: ReturnType<typeof useBootstrapSnapshot>,
	closeSaveAs: () => void,
) {
	const [mvrMode, setMvrMode] = useState<"new" | "merge" | "export" | null>(
		null,
	);
	const [mvrTarget, setMvrTarget] = useState<ShowEntry | null>(null);
	const [mvrPreview, setMvrPreview] = useState<MvrImportPreview | null>(null);
	const [mvrExportPreview, setMvrExportPreview] =
		useState<MvrExportPreview | null>(null);
	const [mvrName, setMvrName] = useState("");
	const [mvrBusy, setMvrBusy] = useState(false);
	const [mvrResolutions, setMvrResolutions] = useState<
		Record<string, { action: string; universe?: number; address?: number }>
	>({});
	async function inspectMvr(file: File) {
		setMvrBusy(true);
		try {
			const preview = await lifecycle?.previewMvr(
				file,
				mvrMode === "merge" ? mvrTarget?.id : undefined,
			);
			if (!preview) return;
			setMvrPreview(preview);
			setMvrName(file.name.replace(/\.mvr$/i, ""));
			const conflicted = new Set(
				preview.address_conflicts
					.map(
						(message) =>
							preview.fixtures.find((fixture) =>
								message.startsWith(fixture.name),
							)?.uuid,
					)
					.filter(Boolean),
			);
			setMvrResolutions(
				Object.fromEntries(
					[...conflicted].map((uuid) => [
						uuid!,
						{ action: "import_unpatched" },
					]),
				),
			);
		} finally {
			setMvrBusy(false);
		}
	}
	async function applyMvr() {
		if (!mvrPreview) return;
		setMvrBusy(true);
		try {
			await lifecycle?.applyMvr(
				mvrPreview.token,
				mvrMode === "new"
					? {
							new_show: { name: mvrName.trim(), open_after_import: true },
							resolutions: mvrResolutions,
						}
					: { existing_show_id: mvrTarget!.id, resolutions: mvrResolutions },
			);
			setMvrMode(null);
			setMvrPreview(null);
		} finally {
			setMvrBusy(false);
		}
	}
	async function inspectExport(show: ShowEntry) {
		setMvrTarget(show);
		setMvrBusy(true);
		try {
			setMvrExportPreview((await lifecycle?.previewMvrExport(show.id)) ?? null);
		} finally {
			setMvrBusy(false);
		}
	}
	function openMvrImport(closeSource: () => void) {
		closeSource();
		setMvrMode("new");
		setMvrTarget(null);
		setMvrPreview(null);
	}
	function openMvrExport() {
		closeSaveAs();
		setMvrMode("export");
		setMvrExportPreview(null);
		const active = bootstrap?.active_show;
		if (active) void inspectExport(active);
		else setMvrTarget(null);
	}
	return {
		mvrMode,
		setMvrMode,
		mvrTarget,
		setMvrTarget,
		mvrPreview,
		setMvrPreview,
		mvrExportPreview,
		mvrName,
		setMvrName,
		mvrBusy,
		mvrResolutions,
		setMvrResolutions,
		inspectMvr,
		applyMvr,
		inspectExport,
		openMvrImport,
		openMvrExport,
	};
}

function useQuickSetupDialogState() {
	const [showName, setShowName] = useState("");
	const [revisionOpen, setRevisionOpen] = useState(false);
	const [revisionName, setRevisionName] = useState("");
	const [saveAsOpen, setSaveAsOpen] = useState(false);
	const [copySaveOpen, setCopySaveOpen] = useState(false);
	const [overwriteTarget, setOverwriteTarget] = useState<ShowEntry | null>(
		null,
	);
	const [overwriteBusy, setOverwriteBusy] = useState(false);
	const [loadOpen, setLoadOpen] = useState(false);
	const [selectiveImportOpen, setSelectiveImportOpen] = useState(false);
	const selectiveImportClose = useRef<(() => void) | null>(null);
	const usbShowPickerTrigger = useRef<(() => void) | null>(null);
	const osShowPickerInput = useRef<HTMLInputElement | null>(null);
	const [newShowOpen, setNewShowOpen] = useState(false);
	const [confirmShutdown, setConfirmShutdown] = useState(false);
	const [changeUserOpen, setChangeUserOpen] = useState(false);
	const [newUserName, setNewUserName] = useState("");
	const [destination, setDestination] = useState<"local" | "flash">("local");
	return {
		changeUserOpen,
		confirmShutdown,
		copySaveOpen,
		destination,
		loadOpen,
		newShowOpen,
		newUserName,
		osShowPickerInput,
		overwriteBusy,
		overwriteTarget,
		revisionName,
		revisionOpen,
		saveAsOpen,
		selectiveImportClose,
		selectiveImportOpen,
		setChangeUserOpen,
		setConfirmShutdown,
		setCopySaveOpen,
		setDestination,
		setLoadOpen,
		setNewShowOpen,
		setNewUserName,
		setOverwriteBusy,
		setOverwriteTarget,
		setRevisionName,
		setRevisionOpen,
		setSaveAsOpen,
		setSelectiveImportOpen,
		setShowName,
		showName,
		usbShowPickerTrigger,
	};
}

function useQuickSetupModel() {
	const { state, dispatch } = useApp();
	const lifecycle = useShowLifecycle();
	const bootstrap = useBootstrapSnapshot();
	const session = useSessionSnapshot();
	const deskLockActions = useDeskLockActions();
	const selectiveImport = useSelectiveImport();
	const desktop = useDesktopBridge();
	const dialogs = useQuickSetupDialogState();
	const mvr = useMvrController(lifecycle, bootstrap, () =>
		dialogs.setSaveAsOpen(false),
	);
	const flashDriveConnected = false;
	const showIndicator = useShowIndicator();
	const activeShow = bootstrap?.active_show;
	const activeShowIsProvisional = /^New Empty Show(?: [1-9]\d*)?$/.test(
		activeShow?.name ?? "",
	);
	const activeShowId = activeShow?.id;
	const revisionCopy = activeShow?.revision_copy;
	const originalShow = revisionCopy
		? (lifecycle?.shows ?? []).find((show) => show.id === revisionCopy.show_id)
		: undefined;
	const close = () =>
		dispatch({ type: "SET_MODAL", modal: "setupOpen", value: false });
	const {
		activeRevisions,
		byShow: revisionsByShow,
		loadNamed: loadNamedRevision,
		openLoadMenu,
		saveNamed: saveNamedRevision,
	} = useShowRevisionController({
		enabled: state.setupOpen,
		activeShowId,
		revisionName: dialogs.revisionName,
		shows: lifecycle?.shows ?? [],
		listShowRevisions: lifecycle?.listShowRevisions ?? (async () => []),
		saveShowRevision: lifecycle?.saveShowRevision ?? (async () => null),
		openShowRevision: lifecycle?.openShowRevision ?? (async () => false),
		setRevisionName: dialogs.setRevisionName,
		setRevisionOpen: dialogs.setRevisionOpen,
		setLoadOpen: dialogs.setLoadOpen,
	});
	function closeTopLayer() {
		if (dialogs.overwriteTarget && !dialogs.overwriteBusy)
			dialogs.setOverwriteTarget(null);
		else if (dialogs.copySaveOpen) dialogs.setCopySaveOpen(false);
		else if (dialogs.revisionOpen) dialogs.setRevisionOpen(false);
		else if (dialogs.saveAsOpen) dialogs.setSaveAsOpen(false);
		else if (dialogs.changeUserOpen) dialogs.setChangeUserOpen(false);
		else if (dialogs.selectiveImportOpen)
			dialogs.selectiveImportClose.current?.();
		else if (dialogs.loadOpen) dialogs.setLoadOpen(false);
		else if (dialogs.newShowOpen) dialogs.setNewShowOpen(false);
		else if (dialogs.confirmShutdown) dialogs.setConfirmShutdown(false);
		else close();
	}
	useQuickSetupKeyboard({
		enabled: state.setupOpen,
		revisionOpen: dialogs.revisionOpen,
		saveAsOpen: dialogs.saveAsOpen,
		changeUserOpen: dialogs.changeUserOpen,
		newUserName: dialogs.newUserName,
		closeTopLayer,
		saveNamedRevision,
		saveAs: () => saveAs(),
		createUser: lifecycle?.createUser ?? (async () => undefined),
		setRevisionName: dialogs.setRevisionName,
		setShowName: dialogs.setShowName,
		setNewUserName: dialogs.setNewUserName,
	});
	async function saveAs(value = dialogs.showName) {
		const name = value.trim();
		if (!name) return;
		if (!(await lifecycle?.saveShowAs(name))) return;
		if (dialogs.destination === "flash" && bootstrap?.active_show)
			await lifecycle?.downloadShow({ ...bootstrap.active_show, name });
		dialogs.setSaveAsOpen(false);
		dialogs.setShowName("");
	}
	function requestOverwrite(show: ShowEntry) {
		dialogs.setSaveAsOpen(false);
		dialogs.setCopySaveOpen(false);
		dialogs.setOverwriteTarget(show);
	}
	async function confirmOverwrite() {
		if (!dialogs.overwriteTarget) return;
		dialogs.setOverwriteBusy(true);
		try {
			if (!(await lifecycle?.overwriteShow(dialogs.overwriteTarget.id))) return;
			dialogs.setOverwriteTarget(null);
		} finally {
			dialogs.setOverwriteBusy(false);
		}
	}
	async function shutDownDesk() {
		if (!(await lifecycle?.shutdownServer())) return;
		if (desktop.available) await desktop.exitApplication();
	}
	async function lockDesk() {
		close();
		await deskLockActions?.lockDesk();
	}
	return {
		actions: {
			close,
			confirmOverwrite,
			loadNamedRevision,
			lockDesk,
			openLoadMenu,
			requestOverwrite,
			saveAs,
			saveNamedRevision,
			shutDownDesk,
		},
		app: { dispatch, state },
		authorities: {
			bootstrap,
			desktop,
			lifecycle,
			selectiveImport,
			session,
		},
		dialogs,
		mvr,
		view: {
			activeRevisions,
			activeShow,
			activeShowId,
			activeShowIsProvisional,
			flashDriveConnected,
			originalShow,
			revisionCopy,
			revisionsByShow,
			showIndicator,
		},
	};
}

export type QuickSetupModel = ReturnType<typeof useQuickSetupModel>;

function QuickSetupTitleBar({ model }: { model: QuickSetupModel }) {
	const { close } = model.actions;
	const { dispatch, state } = model.app;
	const { desktop } = model.authorities;
	return (
		<ModalTitleBar
			title="Show"
			closeLabel="Close Show"
			onClose={close}
			actions={
				<>
					<Button onClick={() => model.dialogs.setChangeUserOpen(true)}>
						<span aria-hidden="true">♙</span> Change User
					</Button>
					{desktop.available && (
						<AddScreenAction
							layout={{
								desks: state.desks,
								activeDeskId: state.activeDeskId,
							}}
							onAdded={close}
						/>
					)}
					<Button
						onClick={() =>
							dispatch({
								type: "SET_MODAL",
								modal: "debugOpen",
								value: true,
							})
						}
					>
						<span aria-hidden="true">⌁</span> Desk Status
					</Button>
				</>
			}
		/>
	);
}

function QuickSetupShowDetails({ model }: { model: QuickSetupModel }) {
	const { activeRevisions, activeShow, revisionCopy, showIndicator } =
		model.view;
	const { session } = model.authorities;
	const dialogs = model.dialogs;
	return (
		<div className="show-details">
			<b>{activeShow?.name ?? "No active show"}</b>
			{revisionCopy && (
				<div className="revision-copy-notice" role="status">
					<strong>Separate revision copy</strong>
					<span>
						Created from <b>{revisionCopy.show_name}</b>, Revision{" "}
						{revisionCopy.revision} · {revisionCopy.revision_name}
					</span>
					<small>
						Created {new Date(revisionCopy.copied_at).toLocaleString()}. Current
						changes are autosaved to this copy, not to {revisionCopy.show_name}.
					</small>
				</div>
			)}
			<div
				className={`show-status-explanation ${showIndicator.className}`}
				role="status"
			>
				<span className="show-status-dot" aria-hidden="true">
					●
				</span>
				<span>
					<strong>{showIndicator.label}</strong>
					<small>{showIndicator.detail}</small>
				</span>
			</div>
			<span>
				Server connected{" "}
				<strong>{showIndicator.connected ? "Yes" : "No"}</strong>
			</span>
			<span>
				Latest named revision{" "}
				<strong>
					{activeRevisions[0]
						? `${activeRevisions[0].revision} · ${activeRevisions[0].name}`
						: "None"}
				</strong>
			</span>
			<span>
				Operator <strong>{session?.user.name ?? "—"}</strong>
			</span>
			<div className="show-primary-actions">
				<Button onClick={() => dialogs.setRevisionOpen(true)}>
					<span aria-hidden="true">💾</span> Save Named Revision
				</Button>
				{revisionCopy && (
					<Button onClick={() => dialogs.setCopySaveOpen(true)}>
						<span aria-hidden="true">✓</span> Save
					</Button>
				)}
				<Button onClick={() => dialogs.setSaveAsOpen(true)}>
					<span aria-hidden="true">✎</span> Save As
				</Button>
				<Button onClick={() => void model.actions.openLoadMenu()}>
					<span aria-hidden="true">↥</span> Load
				</Button>
				<Button onClick={() => dialogs.setNewShowOpen(true)}>
					<span aria-hidden="true">＋</span> New Show
				</Button>
			</div>
		</div>
	);
}

function QuickSetupNavigation({ model }: { model: QuickSetupModel }) {
	const { close, lockDesk } = model.actions;
	const { dispatch } = model.app;
	const openBuiltIn = (kind: "patch" | "setup" | "dmx" | "help") => {
		dispatch({ type: "OPEN_BUILTIN", kind });
		close();
	};
	return (
		<>
			<div className="show-navigation-primary">
				<Button onClick={() => openBuiltIn("patch")}>
					<span className="show-navigation-icon" aria-hidden="true">
						▦
					</span>
					<span>Show Patch</span>
				</Button>
				<Button onClick={() => openBuiltIn("setup")}>
					<span className="show-navigation-icon" aria-hidden="true">
						⚙
					</span>
					<span>Enter Setup</span>
				</Button>
				<Button onClick={() => openBuiltIn("dmx")}>
					<span className="show-navigation-icon" aria-hidden="true">
						◉
					</span>
					<span>DMX</span>
				</Button>
			</div>
			<div className="modal-actions show-secondary-actions">
				<Button className="help-action" onClick={() => openBuiltIn("help")}>
					<span aria-hidden="true">?</span> Help
				</Button>
				<Button
					variant="warning"
					className="lock-action"
					onClick={() => void lockDesk()}
				>
					<span aria-hidden="true">🔒</span> Lock Desk
				</Button>
				<Button
					className="danger shutdown-action"
					onClick={() => model.dialogs.setConfirmShutdown(true)}
				>
					<span aria-hidden="true">⏻</span> Shut Down Desk
				</Button>
			</div>
		</>
	);
}

function QuickSetupModalView({ model }: { model: QuickSetupModel }) {
	return (
		<div
			className="modal-backdrop"
			onPointerDown={(event) => {
				if (event.currentTarget === event.target) model.actions.close();
			}}
		>
			<section
				className="modal-card show-modal"
				role="dialog"
				aria-modal="true"
				aria-label="Show"
			>
				<QuickSetupTitleBar model={model} />
				<QuickSetupShowDetails model={model} />
				<QuickSetupNavigation model={model} />
				<QuickSetupDialogs model={model} />
				<ServerErrorNotice />
			</section>
		</div>
	);
}

export function QuickSetupModal() {
	const model = useQuickSetupModel();
	if (!model.app.state.setupOpen) return null;
	return <QuickSetupModalView model={model} />;
}
