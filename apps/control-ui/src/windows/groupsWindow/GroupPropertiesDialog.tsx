import { useState } from "react";
import { useServer } from "../../api/ServerContext";
import {
	Button,
	ColorPickerField,
	FormLayout,
	IconPickerField,
	ModalPortal,
	TextField,
} from "../../components/common";
import type { Group } from "./model";

export function GroupPropertiesDialog({
	group,
	onClose,
}: {
	group: Group;
	onClose: () => void;
}) {
	const server = useServer();
	const [name, setName] = useState(group.body.name ?? `Group ${group.id}`);
	const [color, setColor] = useState(group.body.color ?? "#718596");
	const [icon, setIcon] = useState(group.body.icon ?? "◇");
	const [saving, setSaving] = useState(false);
	const save = async () => {
		if (!name.trim() || saving) return;
		setSaving(true);
		if (
			await server.updateGroup(group.id, {
				name: name.trim(),
				color,
				icon,
			})
		) {
			onClose();
			return;
		}
		setSaving(false);
	};

	return (
		<ModalPortal>
			<div
				className="stacked-modal-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && onClose()
				}
			>
				<section
					className="nested-modal group-properties-modal"
					role="dialog"
					aria-modal="true"
					aria-label="Group properties"
				>
					<Button className="modal-close" onClick={onClose}>
						×
					</Button>
					<h3>Group {group.id} properties</h3>
					<FormLayout labelPlacement="side">
						<TextField
							label="Group name"
							clearable
							autoFocus
							value={name}
							onChange={(event) => setName(event.target.value)}
						/>
						<ColorPickerField label="Color" value={color} onChange={setColor} />
						<IconPickerField label="Icon" value={icon} onChange={setIcon} />
					</FormLayout>
					<footer>
						<Button onClick={onClose}>Cancel</Button>
						<Button
							variant="primary"
							disabled={!name.trim() || saving}
							onClick={() => void save()}
						>
							{saving ? "Saving…" : "Save group"}
						</Button>
					</footer>
				</section>
			</div>
		</ModalPortal>
	);
}
