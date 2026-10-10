import { useCallback, useEffect, useMemo, useSyncExternalStore } from "react";
import { useCommandLineSurface } from "../../components/control/commandLine/useCommandLineSurface";
import { useProgrammingUpdate } from "../programmingUpdate/ProgrammingUpdateProvider";
import { programmingUpdateSettingsView } from "../programmingUpdate/settingsView";
import { commandLineOption, effectiveOption, targetRecordOption } from "./options";
import { poolRecordLabel, type PoolRecordTarget } from "./poolRecordLabel";

const subscribeEmpty = () => () => undefined;
const snapshotEmpty = () => null;

export function usePoolRecordLabel({ active = true }: { active?: boolean } = {}) {
	const update = useProgrammingUpdate();
	const command = useCommandLineSurface({ enabled: active });
	const view = useMemo(
		() => update ? programmingUpdateSettingsView(update) : null,
		[update],
	);
	const settings = useSyncExternalStore(
		view?.subscribe ?? subscribeEmpty,
		view?.getSnapshot ?? snapshotEmpty,
		snapshotEmpty,
	);
	useEffect(() => {
		if (active && update && view) void view.ensure(() => update.loadSettings());
	}, [active, update, view]);
	const option = effectiveOption(
		command.text,
		"RECORD",
		settings?.record_default ?? "smart",
	);
	return useCallback(
		(target: PoolRecordTarget) => poolRecordLabel(
			target,
			commandLineOption(command.text, "RECORD") ?? targetRecordOption(option, target.kind),
		),
		[option, command.text],
	);
}
