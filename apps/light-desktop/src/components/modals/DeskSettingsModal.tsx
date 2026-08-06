import {
	Button,
	FormLayout,
	IconPickerField,
	ModalPortal,
	ModalTitleBar,
	TextField,
} from "@tosklight/ui";
import { useEffect, useState } from "react";
import { useApp } from "../../state/AppContext";

export function DeskSettingsModal() {
	const { state, dispatch } = useApp();
	const desk = state.desks.find((item) => item.id === state.deskSettingsId);
	const [name, setName] = useState("");
	const [confirmDelete, setConfirmDelete] = useState(false);
	useEffect(() => {
		setName(desk?.name ?? "");
		setConfirmDelete(false);
	}, [desk?.id]);
	if (!state.deskSettingsOpen || !desk) return null;
	const close = () => dispatch({ type: "OPEN_DESK_SETTINGS", id: null });
	const clone = () => {
		dispatch({ type: "START_SAVE_DESK" });
		dispatch({ type: "NEW_DESK" });
		close();
	};
	return (
		<ModalPortal onClose={close}>
			<div
				className="stacked-modal-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && close()
				}
			>
				<section
					className="nested-modal desk-settings-modal"
					role="dialog"
					aria-modal="true"
					aria-label="Desktop settings"
				>
					<ModalTitleBar
						title="Desktop"
						actions={
							<Button
								className="danger"
								disabled={state.desks.length <= 1}
								onClick={() => setConfirmDelete(true)}
							>
								Delete desktop
							</Button>
						}
						closeLabel="Close Desktop settings"
						onClose={close}
					/>
					<div className="desk-settings-content">
						<FormLayout labelPlacement="side">
							<TextField
								label="Name"
								clearable
								value={name}
								onChange={(event) => setName(event.target.value)}
								onBlur={() =>
									name.trim() &&
									dispatch({
										type: "UPDATE_DESK",
										id: desk.id,
										name: name.trim(),
									})
								}
							/>
							<IconPickerField
								label="Icon"
								value={desk.icon ?? "⊞"}
								onChange={(icon) =>
									dispatch({ type: "UPDATE_DESK", id: desk.id, icon })
								}
							/>
						</FormLayout>
						<Button className="large-action" onClick={clone}>
							Clone current desktop
						</Button>
					</div>
				</section>
				{confirmDelete && (
					<DeleteDesktopDialog
						name={desk.name}
						onCancel={() => setConfirmDelete(false)}
						onConfirm={() => dispatch({ type: "DELETE_DESK", id: desk.id })}
					/>
				)}
			</div>
		</ModalPortal>
	);
}

/** Deleting a Desktop is destructive, so it owns a dialog instead of a panel inside settings. */
function DeleteDesktopDialog({
	name,
	onCancel,
	onConfirm,
}: {
	name: string;
	onCancel: () => void;
	onConfirm: () => void;
}) {
	return (
		<ModalPortal onClose={onCancel}>
			<div
				className="stacked-modal-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && onCancel()
				}
			>
				<section
					className="nested-modal delete-desktop-dialog"
					role="alertdialog"
					aria-modal="true"
					aria-label="Delete desktop"
				>
					<ModalTitleBar
						title="Delete desktop"
						closeLabel="Cancel deleting this Desktop"
						onClose={onCancel}
					/>
					<div className="delete-confirm">
						<b>Delete desktop “{name}”?</b>
						<Button autoFocus onClick={onCancel}>
							Cancel
						</Button>
						<Button className="danger" onClick={onConfirm}>
							Confirm delete
						</Button>
					</div>
				</section>
			</div>
		</ModalPortal>
	);
}
