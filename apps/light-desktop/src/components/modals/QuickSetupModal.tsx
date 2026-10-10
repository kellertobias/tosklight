import { formatErrorDetails } from "@tosklight/ui";
import {
	type Dispatch,
	type SetStateAction,
	useEffect,
	useRef,
	useState,
} from "react";
import { useApp } from "../../state/AppContext";
import { useDeskLockActions } from "../../features/deskLock/DeskLockActionsProvider";
import {
	useBootstrapSnapshot,
	useSessionSnapshot,
} from "../../features/deskSnapshot/DeskSnapshotState";
import { useShowLifecycle } from "../../features/showLifecycle/ShowLifecycleContext";
import { useScreens } from "../../features/screens/ScreensContext";
import { Button, ModalRegistration, ModalTitleBar } from "@tosklight/ui";
import type {
	MvrImportPreview,
	ShowEntry,
	ShowRevision,
} from "../../api/types";
import { useShowIndicator } from "../shell/showIndicator";
import { screenForAddAction } from "../setup/screenConfiguration";
import { useDesktopBridge } from "../../platform/desktop";
import { useSelectiveImport } from "../../features/selectiveImport/SelectiveImportContext";
import { QuickSetupDialogs } from "./QuickSetupDialogs";
import { usePatchedFixturesView } from "../../features/patch/PatchState";
import { configuredServerUrl } from "../../api/client/serverLocation";
import { showPatchSummary } from "./showSummary";

function showDate(value?: string | null): string {
	if (!value) return "Unknown";
	const date = new Date(value);
	return Number.isNaN(date.getTime()) ? "Unknown" : date.toLocaleString();
}

interface QuickSetupKeyboardOptions {
	enabled: boolean;
	revisionOpen: boolean;
	saveAsOpen: boolean;
	closeTopLayer: () => void;
	saveNamedRevision: () => Promise<void>;
	saveAs: () => Promise<void>;
	setRevisionName: Dispatch<SetStateAction<string>>;
	setShowName: Dispatch<SetStateAction<string>>;
}

function useQuickSetupKeyboard(options: QuickSetupKeyboardOptions) {
	const optionsRef = useRef(options);
	optionsRef.current = options;
	useEffect(() => {
		if (!options.enabled) return;
		const handle = (event: KeyboardEvent) => {
			if (document.querySelector(".ui-input-modal-layer")) return;
			const current = optionsRef.current;
			if (event.key === "Escape" && document.querySelector(".mvr-modal"))
				return; // The registered MVR modal owns close and inspection cancellation.
			if (event.key === "Escape") {
				event.preventDefault();
				event.stopImmediatePropagation();
				current.closeTopLayer();
				return;
			}
			if (!current.revisionOpen && !current.saveAsOpen) return;
			event.preventDefault();
			event.stopImmediatePropagation();
			if (event.key === "Enter") {
				if (current.revisionOpen) void current.saveNamedRevision();
				else if (current.saveAsOpen) void current.saveAs();
				return;
			}
			const setValue = current.revisionOpen
				? current.setRevisionName
				: current.setShowName;
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
    const openLoadMenu = async () => optionsRef.current.setLoadOpen(true);
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

/** State and workflow for the MVR import flows, kept outside QuickSetupModal for size. */
export function useMvrController(lifecycle: ReturnType<typeof useShowLifecycle>) {
	const [mvrMode, setMvrMode] = useState<"new" | "merge" | null>(
		null,
	);
	const [mvrTarget, setMvrTarget] = useState<ShowEntry | null>(null);
	const [mvrPreview, setMvrPreview] = useState<MvrImportPreview | null>(null);
	const [mvrName, setMvrName] = useState("");
    const [copyConflictingProfiles,setCopyConsentState] = useState(false);
    const copyConsent = useRef(false);
    function setCopyConflictingProfiles(value:boolean) {copyConsent.current=value;setCopyConsentState(value);}
	const [mvrBusy, setMvrBusy] = useState(false);
    const [mvrOperation,setMvrOperation] = useState<"inspect"|"apply"|null>(null);
    const [mvrStartedAt,setMvrStartedAt] = useState<number|null>(null);
    const [mvrInspectionFile,setMvrInspectionFile] = useState<{name:string;size:number}|null>(null);
    const [mvrError,setMvrError] = useState("");
    const inspectionGeneration = useRef(0);
    const inspectionAbort = useRef<AbortController|null>(null);
    const applying = useRef(false);
    const acceptedPreview = useRef<MvrImportPreview|null>(null);
    useEffect(()=>()=>{inspectionGeneration.current+=1;inspectionAbort.current?.abort();acceptedPreview.current=null;},[]);
    function changeMvrMode(mode:"new"|"merge"|null) {
        if(applying.current) return;
        if(mode===null) {
            inspectionGeneration.current+=1;inspectionAbort.current?.abort();inspectionAbort.current=null;acceptedPreview.current=null;
            setMvrPreview(null);setMvrBusy(false);setMvrOperation(null);setMvrError("");
        }
        setMvrMode(mode);
    }

	const mvrFilePickerTrigger = useRef<(() => void) | null>(null);
	const [mvrFilePickerRequested, setMvrFilePickerRequested] = useState(false);
	const [mvrResolutions, setMvrResolutions] = useState<
		Record<string, { action: string; universe?: number; address?: number }>
	>({});
	async function inspectMvr(file: File) {
        if(applying.current) return;
        inspectionAbort.current?.abort();
        const generation=++inspectionGeneration.current;
        acceptedPreview.current=null;
        const abort=new AbortController();inspectionAbort.current=abort;
        setMvrBusy(true);setMvrOperation("inspect");setMvrStartedAt(Date.now());
        setCopyConflictingProfiles(false);
        setMvrInspectionFile({name:file.name,size:file.size});setMvrError("");setMvrPreview(null);
        try {
            if(!lifecycle) throw new Error("The desk connection is unavailable. Reconnect and choose the MVR again.");
            const preview = await lifecycle.previewMvr(
				file,
				mvrMode === "merge" ? mvrTarget?.id : undefined,
                abort.signal,
            );
            if (abort.signal.aborted || inspectionGeneration.current!==generation) return;
			acceptedPreview.current=preview;
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
        } catch(reason) {
            if(!abort.signal.aborted && inspectionGeneration.current===generation) setMvrError(`${formatErrorDetails(reason)} Choose the MVR file again to retry.`);
        } finally {
            if(inspectionGeneration.current===generation) {
                inspectionAbort.current=null;setMvrBusy(false);setMvrOperation(null);
            }
        }
    }

	async function applyMvr() {
        if (!mvrPreview || acceptedPreview.current!==mvrPreview || applying.current || inspectionAbort.current) return;
        if ((mvrPreview.profile_conflicts?.length ?? 0)>0 && !copyConsent.current) return;
        applying.current=true;setMvrOperation("apply");setMvrStartedAt(Date.now());setMvrError("");
        setMvrBusy(true);
		try {
			if(!lifecycle) throw new Error("The desk connection is unavailable. Reconnect before applying the MVR.");
			await lifecycle.applyMvr(
				mvrPreview.token,
				mvrMode === "new"
					? {
							new_show: { name: mvrName.trim(), open_after_import: true },
							resolutions: mvrResolutions,
                            copy_conflicting_profiles: copyConsent.current,
						}
					: { existing_show_id: mvrTarget!.id, resolutions: mvrResolutions, copy_conflicting_profiles: copyConsent.current },
			);
			acceptedPreview.current=null;
			setMvrMode(null);
			setMvrPreview(null);
        } catch(reason) {
            setMvrError(`${formatErrorDetails(reason)} Review the preview and retry applying.`);
        } finally {
            applying.current=false;setMvrOperation(null);
			setMvrBusy(false);
		}
	}
	function openMvrImport(closeSource: () => void) {
        if(applying.current) return;
        inspectionGeneration.current+=1;inspectionAbort.current?.abort();inspectionAbort.current=null;acceptedPreview.current=null;
        setMvrBusy(false);setMvrOperation(null);setMvrError("");
        closeSource();
        setMvrMode("new");
		setMvrTarget(null);
		setMvrPreview(null);
		setMvrFilePickerRequested(true);
	}
	return {
		mvrMode,
        setMvrMode:changeMvrMode,
        mvrOperation,mvrStartedAt,mvrInspectionFile,mvrError,
        mvrTarget,
		setMvrTarget,
		mvrPreview,
		setMvrPreview,
		mvrName,
        copyConflictingProfiles,setCopyConflictingProfiles,
		setMvrName,
		mvrBusy,
		mvrFilePickerTrigger,
		mvrFilePickerRequested,
		setMvrFilePickerRequested,
		mvrResolutions,
		setMvrResolutions,
		inspectMvr,
		applyMvr,
		openMvrImport,
	};
}

function useQuickSetupDialogState() {
	const [showName, setShowName] = useState("");
	const [baseShow, setBaseShow] = useState(false);
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
    const [partialSource, setPartialSource] = useState<ShowEntry | null>(null);
	const selectiveImportClose = useRef<(() => void) | null>(null);
	const usbShowPickerTrigger = useRef<(() => void) | null>(null);
	const osShowPickerInput = useRef<HTMLInputElement | null>(null);
	const [newShowOpen, setNewShowOpen] = useState(false);
	const [confirmShutdown, setConfirmShutdown] = useState(false);
	const saveDestinationSubmit = useRef<{save(): Promise<void>; busy: boolean} | null>(null);
	const [destination, setDestination] = useState<"local" | "flash">("local");
	return {
		saveDestinationSubmit,
		baseShow,
		setBaseShow,
		confirmShutdown,
		copySaveOpen,
		destination,
		loadOpen,
		newShowOpen,
		osShowPickerInput,
		overwriteBusy,
		overwriteTarget,
		revisionName,
		revisionOpen,
		saveAsOpen,
		partialSource,
		setPartialSource,
		selectiveImportClose,
		selectiveImportOpen,
		setConfirmShutdown,
		setCopySaveOpen,
		setDestination,
		setLoadOpen,
		setNewShowOpen,
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
	const mvr = useMvrController(lifecycle);
	const flashDriveConnected = false;
	const showIndicator = useShowIndicator();
	const patchedFixtures = usePatchedFixturesView(state.setupOpen);
	const patchSummary = showPatchSummary(patchedFixtures);
	const activeShow = bootstrap?.active_show;
	const activeShowIsProvisional = /^New Empty Show(?: [1-9]\d*)?$/.test(
		activeShow?.name ?? "",
	);
	const activeShowId = activeShow?.id;
    useEffect(() => {
        if (dialogs.saveAsOpen) dialogs.setBaseShow(activeShow?.is_base_show ?? false);
    }, [dialogs.saveAsOpen, activeShowId, activeShow?.is_base_show]);
    useEffect(() => {
        if (dialogs.saveAsOpen) dialogs.setShowName(activeShow?.name ?? "");
    }, [dialogs.saveAsOpen, activeShowId]);
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
		else if (dialogs.saveAsOpen) { if (!dialogs.saveDestinationSubmit.current?.busy) dialogs.setSaveAsOpen(false); }
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
		closeTopLayer,
		saveNamedRevision,
		saveAs: async () => { if (dialogs.saveDestinationSubmit.current) await dialogs.saveDestinationSubmit.current.save(); else await saveAs(); },
		setRevisionName: dialogs.setRevisionName,
		setShowName: dialogs.setShowName,
	});
	async function saveAs(value = dialogs.showName, latest = false) {
		const name = value.trim();
		if (!name) return false;
		if (!(await lifecycle?.saveShowAs(name, {baseShow: dialogs.baseShow, latest}))) return false;
		if (dialogs.destination === "flash" && bootstrap?.active_show)
			await lifecycle?.downloadShow({ ...bootstrap.active_show, name });
		dialogs.setSaveAsOpen(false);
		dialogs.setShowName("");
        return true;
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
			patchSummary,
		},
	};
}

export type QuickSetupModel = ReturnType<typeof useQuickSetupModel>;

function QuickSetupTitleBar({ model }: { model: QuickSetupModel }) {
	const { close } = model.actions;
	const { dispatch, state } = model.app;
	const { desktop } = model.authorities;
	const screens = useScreens();
	const addScreen = async () => {
		await screens.saveScreen(
			screenForAddAction(screens.screens?.screens ?? [], {
				desks: state.desks,
				activeDeskId: state.activeDeskId,
			}),
		);
		close();
	};
	return (
		<ModalTitleBar
			title="Show"
			closeLabel="Close Show"
			onClose={close}
			groups={[
				{
					id: "show",
					actions: [
						...(desktop.available
							? [
									{
										id: "add-screen",
										label: "Add Screen",
										icon: <span aria-hidden="true">▣</span>,
										onPress: () => void addScreen(),
									},
								]
							: []),
						{
							id: "desk-status",
							label: "Desk Status",
							icon: <span aria-hidden="true">⌁</span>,
							onPress: () =>
								dispatch({
									type: "SET_MODAL",
									modal: "debugOpen",
									value: true,
								}),
						},
					],
				},
			]}
		/>
	);
}

function QuickSetupShowDetails({ model }: { model: QuickSetupModel }) {
	const { activeRevisions, activeShow, revisionCopy, showIndicator, patchSummary } =
		model.view;
	const { bootstrap } = model.authorities;
	const dialogs = model.dialogs;
	const latestRevision = activeRevisions[0];
	const serverAddress = new URL(configuredServerUrl()).hostname;
	return (
		<div className="show-details">
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
				className={`show-status-explanation ${activeShow ? "show-status-connected" : "show-status-warning"}`}
				role="status"
			>
				<span className="show-status-dot" aria-hidden="true">
					●
				</span>
				<span>
					<strong>Current show: {activeShow?.name ?? "None"}</strong>
					<small>Created: {showDate(activeShow?.created_at)}</small>
					<small>Previously loaded: {showDate(activeShow?.last_loaded_at)}</small>
					<small>Last saved: {showDate(activeShow?.updated_at)}</small>
					<small>Last named revision: {latestRevision ? `${latestRevision.name} · ${showDate(latestRevision.created_at)}` : "None"}</small>
				</span>
			</div>
			<div className="show-status-line"><strong>Status</strong><span>Server {showIndicator.connected ? "connected" : "disconnected"} · Hardware {bootstrap?.hardware_connected ? "connected" : "disconnected"}</span></div>
			<div className="show-facts">
				<span>DMX universes <strong>{patchSummary.universes}</strong></span>
				<span>IP address <strong>{serverAddress}</strong></span>
				<span>Parameters sent <strong>{patchSummary.parameters}</strong></span>
				<span>Software build <strong>{__LIGHT_BUILD__}</strong></span>
			</div>
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
	const openBuiltIn = (
		kind: "patch" | "setup" | "scheduler" | "dmx" | "help",
	) => {
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
				<Button onClick={() => openBuiltIn("dmx")}>
					<span className="show-navigation-icon" aria-hidden="true">
						◉
					</span>
					<span>DMX</span>
				</Button>
				<Button onClick={() => openBuiltIn("scheduler")}>
					<span className="show-navigation-icon" aria-hidden="true">
						▣
					</span>
					<span>Scheduler</span>
				</Button>
				<Button onClick={() => openBuiltIn("setup")}>
					<span className="show-navigation-icon" aria-hidden="true">
						⚙
					</span>
					<span>Enter Setup</span>
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
		<ModalRegistration onClose={model.actions.close}>
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
				</section>
			</div>
		</ModalRegistration>
	);
}

export function QuickSetupModal() {
	const model = useQuickSetupModel();
	if (!model.app.state.setupOpen) return null;
	return <QuickSetupModalView model={model} />;
}
