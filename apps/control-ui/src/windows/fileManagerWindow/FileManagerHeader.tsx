import type { MouseEvent as ReactMouseEvent } from "react";
import { createPortal } from "react-dom";
import { Button } from "../../components/common";
import { usePaneChromeTargets } from "../../components/shell/PaneChromeContext";
import { WindowHeader } from "../../components/window-kit";
import { FileMenuIcon } from "./FileMenuIcon";
import type { FileHeaderMenuKind } from "./types";
import type { FileManagerController } from "./useFileManagerController";

type MenuAction = (action: () => void | Promise<void>) => void;

function LocationMenuItems({
	controller,
	act,
}: {
	controller: FileManagerController;
	act: MenuAction;
}) {
	const { navigation } = controller;
	const choices = [
		{ label: navigation.currentRoot?.label ?? "Location", path: "" },
		...controller.breadcrumbs.map((_, index) => ({
			label: `/${controller.breadcrumbs.slice(0, index + 1).join("/")}`,
			path: controller.breadcrumbs.slice(0, index + 1).join("/"),
		})),
	];
	return choices.map((choice) => (
		<Button
			key={choice.path}
			role="menuitem"
			className="file-menu-location"
			active={choice.path === navigation.currentPath}
			onClick={() =>
				act(() =>
					navigation.navigate({ rootId: navigation.rootId, path: choice.path }),
				)
			}
		>
			<FileMenuIcon name="folder" />
			<span>{choice.label}</span>
		</Button>
	));
}

function EditMenuItems({
	controller,
	act,
}: {
	controller: FileManagerController;
	act: MenuAction;
}) {
	const { state, operations } = controller;
	return (
		<>
			<Button
				className="file-menu-rename"
				role="menuitem"
				disabled={state.selected.length !== 1}
				onClick={() => act(() => operations.beginOperation("rename"))}
			>
				<FileMenuIcon name="rename" />
				<span>Rename</span>
			</Button>
			<Button
				className="file-menu-copy"
				role="menuitem"
				disabled={!state.selected.length}
				onClick={() => act(() => operations.beginOperation("copy"))}
			>
				<FileMenuIcon name="copy" />
				<span>Copy</span>
			</Button>
			<Button
				className="file-menu-move"
				role="menuitem"
				disabled={!state.selected.length}
				onClick={() => act(() => operations.beginOperation("move"))}
			>
				<FileMenuIcon name="move" />
				<span>Move</span>
			</Button>
			<Button
				className="file-menu-delete"
				role="menuitem"
				disabled={!state.selected.length}
				onClick={() => act(() => operations.beginOperation("delete"))}
			>
				<FileMenuIcon name="delete" />
				<span>Delete</span>
			</Button>
		</>
	);
}

function CreateMenuItems({
	controller,
	act,
}: {
	controller: FileManagerController;
	act: MenuAction;
}) {
	return (
		<>
			<Button
				className="file-menu-new-file"
				role="menuitem"
				onClick={() => act(() => controller.operations.create(false))}
			>
				<FileMenuIcon name="file-new" />
				<span>New File</span>
			</Button>
			<Button
				className="file-menu-new-folder"
				role="menuitem"
				onClick={() => act(() => controller.operations.create(true))}
			>
				<FileMenuIcon name="folder-new" />
				<span>New Folder</span>
			</Button>
		</>
	);
}

function ViewMenuItems({
	controller,
	act,
}: {
	controller: FileManagerController;
	act: MenuAction;
}) {
	const { state } = controller;
	return (
		<>
			<Button
				role="menuitemradio"
				aria-checked={state.view === "list"}
				onClick={() => act(() => state.setView("list"))}
			>
				<span className="file-menu-check" aria-hidden="true">
					{state.view === "list" ? "✓" : ""}
				</span>
				<FileMenuIcon name="list" />
				<span>List</span>
			</Button>
			<Button
				role="menuitemradio"
				aria-checked={state.view === "grid"}
				onClick={() => act(() => state.setView("grid"))}
			>
				<span className="file-menu-check" aria-hidden="true">
					{state.view === "grid" ? "✓" : ""}
				</span>
				<FileMenuIcon name="grid" />
				<span>Grid</span>
			</Button>
			{/* biome-ignore lint/a11y: This established menu separator is intentionally visual and non-interactive. */}
			<div className="file-menu-divider" role="separator" />
			<Button
				role="menuitemcheckbox"
				aria-checked={controller.hidden}
				onClick={() => act(() => controller.setHidden(!controller.hidden))}
			>
				<span className="file-menu-checkbox" aria-hidden="true">
					{controller.hidden ? "✓" : ""}
				</span>
				<span>Show Hidden Files</span>
			</Button>
			<Button
				role="menuitemcheckbox"
				aria-checked={state.propertiesVisible}
				onClick={() =>
					act(() => {
						state.setPropertiesVisible((value) => !value);
						state.setSidePanel("none");
					})
				}
			>
				<span className="file-menu-checkbox" aria-hidden="true">
					{state.propertiesVisible ? "✓" : ""}
				</span>
				<span>Show Properties Sidebar</span>
			</Button>
		</>
	);
}

function HeaderMenuItems({
	controller,
	act,
}: {
	controller: FileManagerController;
	act: MenuAction;
}) {
	switch (controller.state.headerMenu?.kind) {
		case "location":
			return <LocationMenuItems controller={controller} act={act} />;
		case "edit":
			return <EditMenuItems controller={controller} act={act} />;
		case "create":
			return <CreateMenuItems controller={controller} act={act} />;
		case "view":
			return <ViewMenuItems controller={controller} act={act} />;
		default:
			return null;
	}
}

function HeaderMenu({ controller }: { controller: FileManagerController }) {
	const { state } = controller;
	const menu = state.headerMenu;
	if (!menu) return null;
	const close = () => state.setHeaderMenu(null);
	const act: MenuAction = (action) => {
		close();
		void action();
	};
	const title =
		menu.kind === "create"
			? "New"
			: `${menu.kind[0].toUpperCase()}${menu.kind.slice(1)}`;
	return createPortal(
		<div
			className="file-header-menu-layer"
			onPointerDown={(event) => event.target === event.currentTarget && close()}
		>
			<div
				className="file-header-menu"
				role="menu"
				aria-label={`${title} menu`}
				style={{
					top: menu.anchor.bottom + 3,
					left: Math.max(
						3,
						Math.min(menu.anchor.left, window.innerWidth - 230),
					),
				}}
			>
				<HeaderMenuItems controller={controller} act={act} />
			</div>
		</div>,
		document.body,
	);
}

export function FileManagerHeader({
	controller,
}: {
	controller: FileManagerController;
}) {
	const paneChrome = usePaneChromeTargets();
	const { state, navigation } = controller;
	const headerPath = `/${navigation.currentPath}`;
	const openMenu = (
		kind: FileHeaderMenuKind,
		event: ReactMouseEvent<HTMLButtonElement>,
	) => {
		const anchor = event.currentTarget.getBoundingClientRect();
		state.setHeaderMenu((current) =>
			current?.kind === kind ? null : { kind, anchor },
		);
	};
	const pathControl = (
		<Button
			variant="ghost"
			className="file-manager-header-path"
			aria-label={`Current path ${headerPath}`}
			aria-haspopup="menu"
			aria-expanded={state.headerMenu?.kind === "location"}
			title={headerPath}
			onClick={(event) => openMenu("location", event)}
		>
			<span>{headerPath}</span>
			<FileMenuIcon name="chevron" />
		</Button>
	);
	const actions = (
		<div className="file-manager-header-actions">
			<Button
				aria-label="Edit"
				aria-haspopup="menu"
				aria-expanded={state.headerMenu?.kind === "edit"}
				onClick={(event) => openMenu("edit", event)}
			>
				<span>Edit</span>
				<FileMenuIcon name="chevron" />
			</Button>
			<Button
				aria-label="New"
				aria-haspopup="menu"
				aria-expanded={state.headerMenu?.kind === "create"}
				onClick={(event) => openMenu("create", event)}
			>
				<span>New</span>
				<FileMenuIcon name="chevron" />
			</Button>
			<Button
				aria-label="View"
				aria-haspopup="menu"
				aria-expanded={state.headerMenu?.kind === "view"}
				onClick={(event) => openMenu("view", event)}
			>
				<span>View</span>
				<FileMenuIcon name="chevron" />
			</Button>
			<Button
				aria-label="Back"
				disabled={state.historyIndex <= 0}
				onClick={() => {
					state.setHistoryIndex((value) => value - 1);
					state.setSelected([]);
				}}
			>
				←
			</Button>
			<Button
				aria-label="Forward"
				disabled={
					state.historyIndex < 0 ||
					state.historyIndex >= state.history.length - 1
				}
				onClick={() => {
					state.setHistoryIndex((value) => value + 1);
					state.setSelected([]);
				}}
			>
				→
			</Button>
		</div>
	);
	return (
		<>
			{!paneChrome && !controller.picker && (
				<WindowHeader
					title="File Manager"
					info={{ primary: controller.purpose, secondary: pathControl }}
					toolbar={actions}
					actions={
						controller.closeable && controller.app
							? [
									[
										{
											id: "close",
											label: "×",
											ariaLabel: "Close File Manager",
											onClick: () =>
												controller.app?.dispatch({
													type: "CLOSE_FILE_MANAGER",
												}),
										},
									],
								]
							: []
					}
				/>
			)}
			{paneChrome?.info && createPortal(pathControl, paneChrome.info)}
			{paneChrome?.toolbar && createPortal(actions, paneChrome.toolbar)}
			<HeaderMenu controller={controller} />
		</>
	);
}
