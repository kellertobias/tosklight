import type { Preview } from "@storybook/react-vite";
import { useEffect, type ReactNode } from "react";
import { ModalProvider } from "../../src/modals";
import "../../../light-desktop/src/applicationStyles";
import "./preview.css";

function DocumentationCanvas({ children }: { children: ReactNode }) {
  useEffect(() => {
    document.body.dataset.documentationReady = "true";
    document.body.setAttribute("data-documentation-shot", "");
    return () => {
      delete document.body.dataset.documentationReady;
      document.body.removeAttribute("data-documentation-shot");
    };
  }, []);
  return <div className="storybook-canvas">{children}</div>;
}

const preview: Preview = {
  decorators: [
    (Story) => (
      <DocumentationCanvas>
        <ModalProvider>
          <Story />
        </ModalProvider>
      </DocumentationCanvas>
    ),
  ],
  parameters: {
    controls: { expanded: true },
    options: {
      storySort: {
        order: [
          "Controls",
          "Input",
          "Tables",
          "Faders",
          "Encoders",
          "Playbacks",
          "Pools",
          "Modals",
          "Desktop",
          "Windows",
          "Application",
        ],
      },
    },
  },
  globalTypes: {
    mode: {
      description: "Operator surface mode",
      defaultValue: "software",
      toolbar: {
        icon: "mirror",
        items: ["software", "hardware"],
      },
    },
  },
};

export default preview;
