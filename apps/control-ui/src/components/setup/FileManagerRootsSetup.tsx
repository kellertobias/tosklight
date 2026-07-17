import type { DeskConfiguration } from "../../api/types";
import { Button, FormLayout, SelectField, SwitchField, TextField } from "../common";

export type FileManagerRootConfiguration = DeskConfiguration["file_manager_roots"][number];

const iconOptions = [
  { value: "folder", label: "Folder" },
  { value: "shows", label: "Shows" },
  { value: "drive", label: "Drive" },
  { value: "network", label: "Network" },
  { value: "archive", label: "Archive" },
] as const;

export function looksLikeAbsoluteServerPath(path: string) {
  const value = path.trim();
  return value.startsWith("/") || /^[A-Za-z]:[\\/]/.test(value) || value.startsWith("\\\\");
}

export function fileManagerRootsValidationError(roots: FileManagerRootConfiguration[]) {
  const ids = new Set<string>();
  for (const root of roots) {
    if (!root.id.trim()) return "Every File Manager root requires a stable ID.";
    if (ids.has(root.id)) return `File Manager root ID “${root.id}” is duplicated.`;
    ids.add(root.id);
    if (!root.label.trim()) return `File Manager root “${root.id}” requires an operator-facing label.`;
    if (!looksLikeAbsoluteServerPath(root.path)) return `File Manager root “${root.label}” requires an absolute path on the server.`;
  }
  return null;
}

function nextRootId(roots: FileManagerRootConfiguration[]) {
  const used = new Set(roots.map((root) => root.id));
  let sequence = roots.length + 1;
  let candidate = `location-${sequence}`;
  while (used.has(candidate)) {
    sequence += 1;
    candidate = `location-${sequence}`;
  }
  return candidate;
}

export function FileManagerRootsSetup({
  roots,
  systemPickerFallback,
  onChange,
  onSystemPickerFallbackChange,
  onOpen,
}: {
  roots: FileManagerRootConfiguration[];
  systemPickerFallback: boolean;
  onChange: (roots: FileManagerRootConfiguration[]) => void;
  onSystemPickerFallbackChange: (enabled: boolean) => void;
  onOpen: () => void;
}) {
  const update = (index: number, patch: Partial<FileManagerRootConfiguration>) => {
    onChange(roots.map((root, candidate) => candidate === index ? { ...root, ...patch } : root));
  };
  const validation = fileManagerRootsValidationError(roots);

  return <section className="file-root-setup" aria-label="File Manager root configuration">
    <p>Configured locations are exposed to every authenticated File Manager on this desk. Paths are absolute on the Light server; operators only see the label and root-relative paths.</p>
    {!roots.length && <article className="file-root-default">
      <div><b>Shows</b><small>Built-in default · Desk Shows directory · ID: shows</small></div>
      <span>Used while no custom roots are configured.</span>
    </article>}
    <div className="file-root-list">
      {roots.map((root, index) => <article key={root.id} aria-label={`Configured root ${index + 1}`}>
        <header><b>{root.label || "Unnamed location"}</b><code>{root.id}</code><Button className="danger" onClick={() => onChange(roots.filter((_, candidate) => candidate !== index))}>Remove</Button></header>
        <FormLayout columns={3} minColumnWidth={180}>
          <TextField
            label="Label"
            required
            value={root.label}
            error={!root.label.trim() ? "Enter the label shown to operators." : undefined}
            onChange={(event) => update(index, { label: event.target.value })}
          />
          <TextField
            label="Absolute server path"
            required
            value={root.path}
            error={!looksLikeAbsoluteServerPath(root.path) ? "Use an absolute Unix, Windows drive, or UNC path." : undefined}
            onChange={(event) => update(index, { path: event.target.value })}
          />
          <SelectField
            label="Icon"
            value={root.icon ?? "folder"}
            options={[...iconOptions]}
            onChange={(icon) => update(index, { icon })}
          />
        </FormLayout>
        <small>The stable ID is generated once and is not changed when the label or path is edited.</small>
      </article>)}
    </div>
    {validation && <p className="modal-error" role="alert">{validation}</p>}
    <SwitchField
      label="Allow Open system file picker fallback"
      checked={systemPickerFallback}
      description="Disabled by default. When enabled, forms still open the root-confined ToskLight picker first and offer the operating-system picker only as a secondary action."
      onChange={(event) => onSystemPickerFallbackChange(event.target.checked)}
    />
    <div className="modal-actions">
      <Button onClick={() => onChange([...roots, { id: nextRootId(roots), label: "New location", path: "", icon: "folder" }])}>Add configured root</Button>
      <Button variant="primary" onClick={onOpen}>Open File Manager Workspace</Button>
    </div>
    <small>Connected removable drives are discovered at runtime and are never added to this saved list.</small>
  </section>;
}
