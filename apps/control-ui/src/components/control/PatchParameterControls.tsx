import { useServer } from "../../api/ServerContext";
import { Button } from "../common";

export function PatchParameterControls() {
  const server = useServer();
  const fixture = server.patch?.fixtures.find((item) => server.selectedFixtures.includes(item.fixture_id)) ?? null;
  const updateVector = (kind: "location" | "rotation", axis: "x" | "y" | "z", delta: number) => {
    if (!fixture) return;
    const current = fixture[kind] ?? { x: 0, y: 0, z: 0 };
    void server.updatePatchedFixture(fixture.fixture_id, { [kind]: { ...current, [axis]: current[axis] + delta } });
  };
  const slots = (["x","y","z"] as const).flatMap((axis) => [{ kind: "location" as const, axis }, { kind: "rotation" as const, axis }]).sort((a,b) => a.kind.localeCompare(b.kind));
  return <div className="parameter-controls patch-parameter-controls"><div className="family-tabs"><b>Fixture position</b><span className="family-spacer"/><small>{fixture ? fixture.name || fixture.definition.name : "Select a patched fixture"}</small></div><div className="parameter-surfaces">{slots.map(({ kind, axis }) => { const stored = fixture?.[kind]?.[axis] ?? 0; const display = kind === "location" ? `${(stored / 1000).toFixed(3)} m` : `${stored.toFixed(0)}°`; return <div className="patch-vector-control" key={`${kind}-${axis}`}><span>{kind === "location" ? "Location" : "Rotation"} {axis.toUpperCase()}</span><strong>{display}</strong><div><Button onClick={() => updateVector(kind, axis, kind === "location" ? -10 : -1)}>−</Button><Button onClick={() => updateVector(kind, axis, kind === "location" ? 10 : 1)}>+</Button></div></div>; })}</div></div>;
}
