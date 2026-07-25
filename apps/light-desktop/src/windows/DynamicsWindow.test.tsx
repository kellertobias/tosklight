import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { DynamicsWindow } from "./DynamicsWindow";

describe("DynamicsWindow", () => {
  it("shows the future-feature empty state without the conceptual editor", () => {
    render(<DynamicsWindow compact={false} />);

    expect(screen.getByRole("status")).toHaveTextContent("Dynamics is a future feature");
    expect(screen.getByText("Dynamics is currently being conceptualized.")).toBeInTheDocument();
    expect(screen.queryByText("Attribute Dynamics")).not.toBeInTheDocument();
    expect(screen.queryByText("Dynamic properties")).not.toBeInTheDocument();
  });
});
