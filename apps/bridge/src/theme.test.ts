import { expect, test } from "bun:test";
import { DEFAULT_DARK_MODE } from "./theme";

test("defaults to light mode", () => {
  expect(DEFAULT_DARK_MODE).toBe(false);
});
