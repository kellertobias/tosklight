import { useEffect, useRef } from "react";
import {
	type ControlSurfaceTarget,
	registerControlSurfaceTarget,
} from "./registry";

export function useControlSurfaceTarget(target: ControlSurfaceTarget | null) {
	const current = useRef(target);
	current.current = target;
	useEffect(() => {
		if (!target) return;
		return registerControlSurfaceTarget({
			id: target.id,
			priority: target.priority,
			accepts: (intent) => current.current?.accepts(intent) ?? false,
			handle: (intent) => current.current?.handle(intent),
		});
	}, [target?.id, target?.priority]);
}
