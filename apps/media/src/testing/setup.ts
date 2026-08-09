import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// Vitest is not running with globals, so React Testing Library's automatic cleanup never
// registers itself. Without this, a mounted page from an earlier test keeps polling and keeps
// answering queries — and a test proves something about the wrong render.
afterEach(cleanup);
