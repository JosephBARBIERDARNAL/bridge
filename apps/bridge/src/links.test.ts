import { describe, expect, test } from "bun:test";
import { normalizeHttpsUrl } from "./urlPolicy";

describe("HTTPS link policy", () => {
  test("normalizes HTTPS URLs and removes fragments", () => {
    expect(normalizeHttpsUrl("https://example.com/path#section")).toBe(
      "https://example.com/path",
    );
  });

  test("rejects HTTP, credentials, relative URLs, and custom schemes", () => {
    for (const value of [
      "http://example.com",
      "https://user:password@example.com",
      "/relative",
      "intent://settings",
      "tel:+33123456789",
      "javascript:alert(1)",
    ])
      expect(normalizeHttpsUrl(value)).toBeUndefined();
  });
});
