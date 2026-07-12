import { useServer } from "../../api/ServerContext";
import { VerticalTouchFader } from "./VerticalTouchFader";

export function ProgrammerFadeFader() {
  const server = useServer();
  const value = (server.configuration?.programmer_fade_millis ?? 3_000) / 1_000;
  return <div className="programmer-fade-fader"><VerticalTouchFader label="Prog. Fade" value={value} maximum={20} display={`${value.toFixed(1)} s`} onChange={(next) => void server.setControlTiming({ programmer_fade_millis: Math.round(next * 1_000) })} /></div>;
}
