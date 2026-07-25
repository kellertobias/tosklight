import { useState } from "react";
import { useDeskLockActions } from "../../features/deskLock/DeskLockActionsProvider";
import { useDeskLock } from "../../features/deskLock/DeskLockState";
import { Button, TextField } from "../common";
import "./DeskLockOverlay.css";

export function DeskLockOverlay() {
  const deskLock = useDeskLock();
  const deskLockActions = useDeskLockActions();
  const [pin, setPin] = useState("");
  const [incorrect, setIncorrect] = useState(false);
  if (!deskLock?.locked) return null;
  const unlock = async () => {
    const ok = await deskLockActions?.unlockDesk(deskLock?.unlock_mode === "pin" ? pin : undefined);
    setIncorrect(!ok);
    if (ok) setPin("");
  };
  return (
    <div
      className="desk-lock-overlay"
      role="dialog"
      aria-label="Desk locked"
      style={
        deskLock.wallpaper
          ? {
              backgroundImage: `linear-gradient(#0008,#0008),url(${JSON.stringify(deskLock.wallpaper)})`,
            }
          : undefined
      }
    >
      <section>
        <h1>Desk locked</h1>
        <p>{deskLock.message || "This desk is locked."}</p>
        {deskLock.unlock_mode === "pin" && (
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
        <Button onClick={() => void unlock()} disabled={deskLock.unlock_mode === "pin" && !pin}>
          Unlock Desk
        </Button>
      </section>
    </div>
  );
}
