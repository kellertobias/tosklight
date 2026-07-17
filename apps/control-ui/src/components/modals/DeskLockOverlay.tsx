import { useState } from "react";
import { useServer } from "../../api/ServerContext";
import { Button, TextField } from "../common";
import "./DeskLockOverlay.css";

export function DeskLockOverlay() {
  const server = useServer();
  const [pin, setPin] = useState("");
  const [incorrect, setIncorrect] = useState(false);
  if (!server.deskLock?.locked) return null;
  const unlock = async () => {
    const ok = await server.unlockDesk(server.deskLock?.unlock_mode === "pin" ? pin : undefined);
    setIncorrect(!ok);
    if (ok) setPin("");
  };
  return (
    <div
      className="desk-lock-overlay"
      role="dialog"
      aria-label="Desk locked"
      style={
        server.deskLock.wallpaper
          ? {
              backgroundImage: `linear-gradient(#0008,#0008),url(${JSON.stringify(server.deskLock.wallpaper)})`,
            }
          : undefined
      }
    >
      <section>
        <h1>Desk locked</h1>
        <p>{server.deskLock.message || "This desk is locked."}</p>
        {server.deskLock.unlock_mode === "pin" && (
          <TextField
            label="PIN"
            secure
            inputMode="numeric"
            autoComplete="off"
            value={pin}
            error={incorrect ? "Incorrect PIN" : undefined}
            onChange={(event) => {
              setPin(event.target.value.replace(/\D/g, ""));
              setIncorrect(false);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") void unlock();
            }}
          />
        )}
        <Button onClick={() => void unlock()} disabled={server.deskLock.unlock_mode === "pin" && !pin}>
          Unlock Desk
        </Button>
      </section>
    </div>
  );
}
