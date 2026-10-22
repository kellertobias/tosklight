import { fireEvent, render, screen, cleanup } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ExactFrame } from "./TimecodeCueControls";

afterEach(cleanup);
it("submits Enter followed by blur once while awaiting authority", () => {
 const commit = vi.fn();
 render(<ExactFrame label="Trigger" frame={44} onCommit={commit} />);
 const input = screen.getByLabelText("Trigger");
 fireEvent.change(input, {target:{value:"00:00:02.00"}});
 fireEvent.keyDown(input,{key:"Enter"}); fireEvent.blur(input);
 expect(commit).toHaveBeenCalledExactlyOnceWith(88);
});
it("keeps invalid frame values local and allows correction", () => {
 const commit = vi.fn();
 render(<ExactFrame label="Trigger" frame={44} onCommit={commit} />);
 const input = screen.getByLabelText("Trigger");
 fireEvent.change(input,{target:{value:"00:00:02.44"}});fireEvent.blur(input);
 expect(input).toHaveAttribute("aria-invalid","true");expect(commit).not.toHaveBeenCalled();
 fireEvent.change(input,{target:{value:"00:00:02.43"}});fireEvent.blur(input);
 expect(commit).toHaveBeenCalledExactlyOnceWith(131);
});
