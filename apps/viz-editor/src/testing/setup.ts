import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// Vitest globals are off here, so React Testing Library's automatic cleanup does not register
// itself; without this, each test would mount another window on top of the last.
afterEach(cleanup);
