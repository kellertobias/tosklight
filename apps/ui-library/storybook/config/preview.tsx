import type { Preview } from "@storybook/react-vite";
import { useEffect, useState, type ReactNode } from "react";
import { ModalProvider } from "../../src/modals";
import { actualSourceForStory } from "./actualSource";
import "../../src/styles.css";
import "./preview.css";

function DocumentationCanvas({
	application,
	children,
}: {
	application: boolean;
	children: ReactNode;
}) {
	const [stylesReady, setStylesReady] = useState(!application);
	useEffect(() => {
		let active = true;
		if (!application) {
			setStylesReady(true);
			return () => {
				active = false;
			};
		}
		void import("../../../light-desktop/src/applicationStyles").then(() => {
			if (active) setStylesReady(true);
		});
		return () => {
			active = false;
		};
	}, [application]);
  useEffect(() => {
    if (!stylesReady) return;
    document.body.dataset.documentationReady = "true";
    document.body.setAttribute("data-documentation-shot", "");
    return () => {
      delete document.body.dataset.documentationReady;
      document.body.removeAttribute("data-documentation-shot");
    };
  }, [stylesReady]);
  if (!stylesReady) return null;
  return <div className="storybook-canvas">{children}</div>;
}

const preview: Preview = {
  decorators: [
    (Story, context) => (
      <DocumentationCanvas application={context.title.startsWith("ToskLight/")}>
        <ModalProvider>
          <Story />
        </ModalProvider>
      </DocumentationCanvas>
    ),
  ],
  parameters: {
    controls: { expanded: true },
    docs: {
      codePanel: true,
      source: {
        language: "tsx",
        transform: actualSourceForStory,
      },
    },
    options: {
      storySort: {
        order: [
          "Controls",
          [
            "Buttons",
            "Forms",
            "Keyboard and numpad",
            "Faders",
            "Encoders",
            "Playbacks",
          ],
          "Tables and Grids",
          [
            "Generic table",
            "Fixture grid",
            "Pools",
            "Virtual playback grid",
            "DMX patch grid",
          ],
          "Window System",
          ["Modals", "Production window kit", "Desktop"],
          "ToskLight",
          [
            "Windows",
            "Shell and control",
            "Virtual Playbacks",
            "Modal workflows",
            "Command line",
            "Command section",
            "Marketing",
          ],
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
