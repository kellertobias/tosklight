import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Button } from "../controls";
import { WindowHeader } from "../window-kit";
import {
  ModalFrame,
  ModalPortal,
  ModalProvider,
  ModalRegistration,
  useModalStack,
  type ModalClosePolicy,
  type ModalRole,
} from "../modals";

const meta = {
  title: "Window System/Modals",
  component: ModalFrame,
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" },
  args: {
    id: "configured-modal",
    role: "modal",
    policy: { escape: true, backdrop: true, explicit: true },
    ariaLabel: "Configured modal",
    title: "Configured modal",
    onClose: () => undefined,
    children: "Configured modal content",
  },
  argTypes: {
    role: { control: "inline-radio", options: ["modal", "dialog"] satisfies ModalRole[] },
    policy: { control: "object" },
    title: { control: "text" },
    ariaLabel: { control: "text" },
  },
} satisfies Meta<typeof ModalFrame>;

export default meta;
type Story = StoryObj<typeof meta>;

function ThreeDeepExample() {
  const [depth, setDepth] = useState(1);
  return (
    <ModalProvider>
      <div style={{ height: 500 }}>
        <Button onClick={() => setDepth(1)}>Open modal</Button>
      </div>
      {depth >= 1 && (
        <ModalFrame id="window-modal" ariaLabel="Window modal" title="Window modal" onClose={() => setDepth(0)}>
          <p>A window opened this modal.</p>
          <Button onClick={() => setDepth(2)}>Open nested modal</Button>
        </ModalFrame>
      )}
      {depth >= 2 && (
        <ModalFrame id="nested-modal" ariaLabel="Nested modal" title="Nested modal" onClose={() => setDepth(1)}>
          <p>The first modal opened this one.</p>
          <Button onClick={() => setDepth(3)}>Open third modal</Button>
        </ModalFrame>
      )}
      {depth >= 3 && (
        <ModalFrame id="third-modal" ariaLabel="Third modal" title="Third modal" policy={{ backdrop: false }} onClose={() => setDepth(2)}>
          <p>Only this top eligible modal handles Escape.</p>
        </ModalFrame>
      )}
    </ModalProvider>
  );
}

export const ThreeDeep: Story = {
  render: () => <ThreeDeepExample />,
};

function PolicyExample({
  role,
  policy,
}: {
  role: ModalRole;
  policy: ModalClosePolicy;
}) {
  const [open, setOpen] = useState(true);
  return (
    <ModalProvider>
      <div style={{ height: 500 }}>
        <Button onClick={() => setOpen(true)}>Reopen policy example</Button>
        <output aria-label="Policy modal state">{open ? "Open" : "Closed"}</output>
      </div>
      {open && (
        <ModalFrame
          id="policy-modal"
          role={role}
          policy={policy}
          ariaLabel="Policy modal"
          title="Close policies"
          details={`${role} · explicit policies`}
          onClose={() => setOpen(false)}
        >
          <p>Escape, backdrop, and title-close behavior follow the configured policy.</p>
        </ModalFrame>
      )}
    </ModalProvider>
  );
}

export const ClosePolicies: Story = {
  args: {
    role: "dialog",
    policy: { escape: false, backdrop: false, explicit: false },
  },
  render: (args) => <PolicyExample role={args.role ?? "modal"} policy={args.policy ?? {}} />,
};

function TitleBarExample({
  keyboardInitiallyOpen = false,
  settingsInitiallyOpen = false,
  nestedInputInitiallyOpen = false,
  minimal = false,
}: {
  keyboardInitiallyOpen?: boolean;
  settingsInitiallyOpen?: boolean;
  nestedInputInitiallyOpen?: boolean;
  minimal?: boolean;
}) {
  const [activeTab, setActiveTab] = useState("general");
  const [search, setSearch] = useState("");
  const [type, setType] = useState("all");
  const [favorites, setFavorites] = useState(false);
  const [presetName, setPresetName] = useState("Tour defaults");
  return (
    <ModalProvider>
      <ModalFrame
        id="configured-title"
        className="ui-modal-wide"
        role="dialog"
        ariaLabel="Configured title bar"
        title="Patch fixtures"
        details="12 selected"
        tabs={minimal ? undefined : [
          { id: "general", label: "General" },
          { id: "advanced", label: "Advanced" },
          { id: "locked", label: "Locked", disabled: true },
        ]}
        activeTab={minimal ? undefined : activeTab}
        onTabChange={minimal ? undefined : setActiveTab}
        search={{
          value: search,
          placeholder: "Search fixtures",
          keyboardInitiallyOpen,
          settingsInitiallyOpen,
          settings: minimal ? [] : [
            {
              kind: "select",
              id: "type",
              label: "Fixture type",
              value: type,
              options: [
                { value: "all", label: "All" },
                { value: "moving", label: "Moving light" },
              ],
            },
            {
              kind: "toggle",
              id: "favorites",
              label: "Favorites only",
              value: favorites,
              offLabel: "All fixtures",
              onLabel: "Favorites",
            },
            {
              kind: "text",
              id: "preset",
              label: "Preset name",
              value: presetName,
              placeholder: "Optional preset",
              keyboardInitiallyOpen: nestedInputInitiallyOpen,
            },
          ],
          onSettingChange: (id, value) => {
            if (id === "type") setType(String(value));
            if (id === "favorites") setFavorites(Boolean(value));
            if (id === "preset") setPresetName(String(value));
          },
          onClearSettings: () => {
            setType("all");
            setFavorites(false);
          },
        }}
        onSearch={setSearch}
        actions={minimal ? undefined : <Button variant="primary">Save</Button>}
        policy={minimal ? { explicit: false } : undefined}
        closeLabel="Close patch fixtures"
        onClose={() => undefined}
      >
        <p>Tabs, search, grouped actions, details, and explicit close share one title bar.</p>
        <output aria-label="Active modal tab">{activeTab}</output>
      </ModalFrame>
    </ModalProvider>
  );
}

export const TitleBarConfiguration: Story = {
  render: () => <TitleBarExample />,
};

export const SearchKeyboardAboveConfigured: Story = {
  render: () => <TitleBarExample keyboardInitiallyOpen />,
};

export const SearchSettingsAboveConfigured: Story = {
  render: () => <TitleBarExample settingsInitiallyOpen />,
};

export const ThirdChildAboveSearchSettings: Story = {
  render: () => <TitleBarExample settingsInitiallyOpen nestedInputInitiallyOpen />,
};

export const SearchWithoutAdjacentButtons: Story = {
  render: () => <TitleBarExample minimal />,
};

function WindowSearchExample() {
  const [search, setSearch] = useState("");
  return <div style={{ height: 240 }}>
    <WindowHeader
      title="Fixture library"
      search={{ value: search, placeholder: "Search fixtures" }}
      onSearch={setSearch}
      actions={[[{ id: "add", label: "Add fixture", onClick: () => undefined }]]}
    />
  </div>;
}

export const WindowTitleBarSearch: Story = {
  render: () => <WindowSearchExample />,
};

function ProgrammaticCloser({ children = "Close target by ID" }: { children?: string }) {
  const stack = useModalStack();
  return <Button onClick={() => stack.close("programmatic-target")}>{children}</Button>;
}

function ProgrammaticExample() {
  const [open, setOpen] = useState(true);
  return (
    <ModalProvider>
      <div style={{ height: 500 }}>
        {!open && <Button onClick={() => setOpen(true)}>Reopen target</Button>}
        <output aria-label="Programmatic modal state">{open ? "Open" : "Closed"}</output>
      </div>
      {open && (
        <ModalFrame
          id="programmatic-target"
          ariaLabel="Programmatic target"
          title="Application-triggered close"
          onClose={() => setOpen(false)}
        >
          <p>Application adapters can close this layer through its stable identifier.</p>
          <ProgrammaticCloser />
        </ModalFrame>
      )}
    </ModalProvider>
  );
}

export const ProgrammaticClose: Story = {
  render: () => <ProgrammaticExample />,
};

export const PortalPrimitive: Story = {
  render: () => (
    <ModalPortal>
      <section
        role="status"
        style={{
          position: "fixed",
          inset: "auto 24px 24px auto",
          padding: 16,
          border: "1px solid var(--cyan)",
          borderRadius: 8,
          background: "var(--panel)",
        }}
      >
        Portal content shares the production document layer.
      </section>
    </ModalPortal>
  ),
};

function ApplicationRegistrationExample() {
  const [open, setOpen] = useState(true);
  return (
    <ModalProvider>
      {!open && <Button onClick={() => setOpen(true)}>Reopen application dialog</Button>}
      {open && (
        <ModalRegistration id="application-registration" onClose={() => setOpen(false)}>
          <div className="stacked-modal-layer">
            <section
              className="nested-modal"
              role="dialog"
              aria-label="Existing application dialog"
            >
              <h2>Application workflow</h2>
              <p>The existing workflow body participates in the authoritative modal stack.</p>
              <Button onClick={() => setOpen(false)}>Close application dialog</Button>
            </section>
          </div>
        </ModalRegistration>
      )}
    </ModalProvider>
  );
}

export const ApplicationRegistration: Story = {
  render: () => <ApplicationRegistrationExample />,
};
