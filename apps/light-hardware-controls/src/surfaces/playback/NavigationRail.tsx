import {
	controlSurfaceOscPaths,
	type ProgrammerControlAction,
} from "@tosklight/ui/control-surface-contracts";
import { useRef } from "react";
import { ControlButton } from "../../components/ControlButton";
import { actionRequestId } from "../../controller/actionRequestId";
import type { SendControl } from "../../controller/types";

interface NavigationRailProps {
	page: number;
	send: SendControl;
	nativePageControls?: boolean;
}

export function NavigationRail({
	page,
	send,
	nativePageControls = false,
}: NavigationRailProps) {
	const programmerActionIds = useRef(
		new Map<ProgrammerControlAction, string>(),
	);
	const sendProgrammerAction = (
		action: ProgrammerControlAction,
		down: boolean,
	) => {
		const requestId = down
			? actionRequestId()
			: (programmerActionIds.current.get(action) ?? actionRequestId());
		if (down) programmerActionIds.current.set(action, requestId);
		else programmerActionIds.current.delete(action);
		send(controlSurfaceOscPaths.programmer(action), [down, requestId]);
	};
	const programmerKey = (label: "ESCAPE" | "MENU" | "PROG-PLAYBACK") => {
		const action = label.toLowerCase() as ProgrammerControlAction;
		return (
			<ControlButton
				className={`key-${action}`}
				label={label}
				onDown={() => sendProgrammerAction(action, true)}
				onUp={() => sendProgrammerAction(action, false)}
			/>
		);
	};
	const changePage = (action: "page-up" | "page-down", nextPage: number) => {
		if (!nativePageControls) {
			send(controlSurfaceOscPaths.page, [nextPage]);
			return;
		}
		sendProgrammerAction(action, true);
		sendProgrammerAction(action, false);
	};

	return (
		<aside className="left-rail">
			{programmerKey("ESCAPE")}
			{programmerKey("MENU")}
			{programmerKey("PROG-PLAYBACK")}
			<span className="button-spacer" />
			<ControlButton
				className="key-align"
				label="ALIGN"
				onDown={() => sendProgrammerAction("align", true)}
				onUp={() => sendProgrammerAction("align", false)}
			/>
			<span className="button-spacer" />
			<button
				type="button"
				onClick={() => changePage("page-up", Math.max(1, page - 1))}
			>
				PAGE UP
			</button>
			<strong>{page}</strong>
			<button type="button" onClick={() => changePage("page-down", page + 1)}>
				PAGE DOWN
			</button>
		</aside>
	);
}
