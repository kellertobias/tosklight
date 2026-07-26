import type { Preview } from "@storybook/react-vite";
import { useEffect, useState, type ReactNode } from "react";
import { ModalProvider } from "../../src/modals";
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
      <DocumentationCanvas application={context.title.startsWith("Application/")}>
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
