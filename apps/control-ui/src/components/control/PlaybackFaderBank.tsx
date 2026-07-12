import { useServer } from "../../api/ServerContext";

export function PlaybackFaderBank() {
  const server = useServer();
  const cueLists = server.playbacks?.cue_lists ?? [];
  const slots = Array.from({ length: 8 }, (_, index) => ({ cue: cueLists[index] ?? null, group: cueLists[index] ? null : server.groups.find((group) => group.body.playback_fader === index + 1) ?? null }));
  return <div className="playback-fader-bank">{slots.map(({ cue, group }, index) => {
    const active = cue ? server.playbacks?.active.find((item) => item.cue_list_id === cue.id) : undefined;
    const level = Math.round((group?.body.master ?? 0) * 100);
    return <article className={`${active ? "running" : ""} ${group ? "group-master-playback" : ""}`} key={cue?.id ?? group?.id ?? `empty-${index}`}><b>{index + 1}. {cue?.name ?? group?.body.name ?? "Unassigned"}</b><div>{group ? <><strong>Group master · {level}%</strong><input aria-label={`${group.body.name ?? group.id} playback master`} type="range" min="0" max="100" value={level} onChange={(event) => void server.setGroupMaster(group.id, Number(event.target.value) / 100)}/></> : <><strong>{active ? `Cue ${active.cue_index + 1}` : cue ? "Ready" : "Empty"}</strong><i style={{ height: `${active ? 72 : 25 + (index * 7) % 60}%` }}/></>}</div><footer><button disabled={!cue && !group} onClick={() => cue ? void server.playbackAction(cue.id, "go") : group && void server.selectGroup(group.id)}>{group ? "SELECT" : "GO"}</button><button disabled={!group} onPointerDown={() => group && void server.setGroupMaster(group.id, 1)} onPointerUp={() => group && void server.setGroupMaster(group.id, level / 100)}>FLASH</button></footer></article>;
  })}</div>;
}
