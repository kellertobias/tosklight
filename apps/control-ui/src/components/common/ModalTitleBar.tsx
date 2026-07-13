import type { ReactNode } from "react";

export interface ModalTitleTab {
  id: string;
  label: ReactNode;
  disabled?: boolean;
}

export function ModalTitleBar({
  title,
  tabs,
  activeTab,
  onTabChange,
  actions,
  onClose,
  closeLabel = "Close modal",
  className = "",
}: {
  title: ReactNode;
  tabs?: ModalTitleTab[];
  activeTab?: string;
  onTabChange?: (id: string) => void;
  actions?: ReactNode;
  onClose?: () => void;
  closeLabel?: string;
  className?: string;
}) {
  const hasTabs = Boolean(tabs?.length);
  return <header className={`ui-modal-titlebar ${className}`.trim()}>
    <div className="ui-modal-title-tabs" role={hasTabs ? "tablist" : undefined}>
      {hasTabs ? tabs!.map((tab) => <button
        type="button"
        key={tab.id}
        role="tab"
        aria-selected={tab.id === activeTab}
        className={`ui-button ui-secondary ui-default ${tab.id === activeTab ? "active" : ""}`.trim()}
        disabled={tab.disabled}
        onClick={() => onTabChange?.(tab.id)}
      >{tab.label}</button>) : <h2>{title}</h2>}
    </div>
    <span className="ui-modal-title-spacer" />
    {actions && <div className="ui-modal-title-actions">{actions}</div>}
    {onClose && <button type="button" className="ui-button ui-secondary ui-default ui-modal-title-close" aria-label={closeLabel} title={closeLabel} onClick={onClose}>×</button>}
  </header>;
}
