import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import type { PlaybackPage } from "../../api/types";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import { Button, TextInput } from "../common";

export const MAX_PLAYBACK_PAGES = 127;

export function nextPlaybackPageNumber(pages: PlaybackPage[]): number | null {
  const lastNumber = pages.reduce((maximum, page) => Math.max(maximum, page.number), 0);
  return lastNumber < MAX_PLAYBACK_PAGES ? lastNumber + 1 : null;
}

export function canAdvancePlaybackPage(pages: PlaybackPage[], currentPage: number): boolean {
  if (pages.some((page) => page.number === currentPage + 1)) return true;
  const lastPage = pages.reduce<PlaybackPage | undefined>((last, page) => !last || page.number > last.number ? page : last, undefined);
  return Boolean(lastPage && currentPage === lastPage.number && Object.keys(lastPage.slots ?? {}).length > 0 && lastPage.number < MAX_PLAYBACK_PAGES);
}

export function PlaybackPageMenu({ open, onClose }: { open: boolean; onClose: () => void }) {
  const server = useServer();
  const { dispatch } = useApp();
  if (!open) return null;
  const pages = [...(server.playbacks?.pages ?? [])].sort((left, right) => left.number - right.number);
  const nextNumber = nextPlaybackPageNumber(pages);
  const select = (number: number) => {
    dispatch({ type: "SET_PLAYBACK_PAGE", page: number - 1 });
    void server.setPlaybackPage(number);
    onClose();
  };
  const add = async () => {
    if (nextNumber == null) return;
    const saved = await server.savePlaybackPage({ number: nextNumber, name: `Page ${nextNumber}`, slots: {} });
    if (saved) select(nextNumber);
  };
  return createPortal(<div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && onClose()}>
    <section className="nested-modal playback-page-modal" role="dialog" aria-modal="true" aria-label="Playback pages">
      <Button className="modal-close" onClick={onClose}>×</Button>
      <h3>Playback pages</h3>
      <div>{pages.map((item) => <Button className={item.number === (server.playbacks?.active_page ?? 1) ? "active" : ""} key={item.number} onClick={() => select(item.number)}><strong>{item.number}</strong><span>{item.name}</span></Button>)}</div>
      <footer><Button variant="primary" disabled={nextNumber == null} onClick={() => void add()}>Add new page</Button></footer>
    </section>
  </div>, document.body);
}

export function PlaybackPageRenameDialog({ page, onClose }: { page: PlaybackPage | null; onClose: () => void }) {
  const server = useServer();
  const [name, setName] = useState(page?.name ?? "");
  useEffect(() => setName(page?.name ?? ""), [page]);
  if (!page) return null;
  const save = async (value = name) => {
    const trimmed = value.trim();
    if (!trimmed) return;
    if (await server.savePlaybackPage({ ...page, name: trimmed })) onClose();
  };
  return createPortal(<div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && onClose()}>
    <section className="nested-modal playback-page-name-modal" role="dialog" aria-modal="true" aria-label={`Rename playback page ${page.number}`}>
      <Button className="modal-close" onClick={onClose}>×</Button>
      <h3>Rename Playback Page {page.number}</h3>
      <TextInput autoFocus clearable aria-label="Playback page name" value={name} onChange={(event) => setName(event.target.value)} onKeyboardCommit={(value) => void save(value)}/>
      <footer><Button onClick={onClose}>Cancel</Button><Button variant="primary" disabled={!name.trim()} onClick={() => void save()}>Rename Page</Button></footer>
    </section>
  </div>, document.body);
}
