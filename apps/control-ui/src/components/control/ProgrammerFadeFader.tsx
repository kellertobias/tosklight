import { useProgrammerFadeMillis } from "../../features/configuration/ConfigurationState";
import { useServer } from "../../api/ServerContext";
import { TouchValueButton, VerticalTouchFader } from "./VerticalTouchFader";

export function ProgrammerFadeFader({ compact = false }: { compact?: boolean }) {
  const server = useServer();
  const value = (useProgrammerFadeMillis() ?? 3_000) / 1_000;
  const onChange = (next: number) => void server.setControlTiming({ programmer_fade_millis: Math.round(next * 1_000) });
  return <div className={`programmer-fade-fader ${compact ? "compact" : "full"}`}>{compact
    ? <TouchValueButton label="Prog. Fade" value={value} maximum={20} display={`${value.toFixed(1)} s`} onChange={onChange}/>
    : <VerticalTouchFader label="Prog. Fade" value={value} maximum={20} display={`${value.toFixed(1)} s`} directInput onChange={onChange}/>
  }</div>;
}
