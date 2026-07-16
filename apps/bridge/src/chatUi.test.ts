import { describe, expect, test } from "bun:test";
import {
  isNearBottom,
  isHistorySwipeStart,
  sanitizeMarkdownImages,
  shouldClaimHistorySwipe,
  shouldOpenHistoryDrawer,
  shouldPauseAutoFollow,
} from "./chatUi";

describe("chat auto-follow", () => {
  test("follows while the viewport is near the bottom", () => {
    expect(
      isNearBottom({
        contentHeight: 1000,
        viewportHeight: 400,
        offsetY: 540,
      }),
    ).toBe(true);
  });

  test("stops following after the user scrolls away", () => {
    expect(
      isNearBottom({
        contentHeight: 1000,
        viewportHeight: 400,
        offsetY: 300,
      }),
    ).toBe(false);
  });

  test("pauses after even a tiny upward scroll", () => {
    expect(shouldPauseAutoFollow(600, 599)).toBe(true);
    expect(shouldPauseAutoFollow(600, 600)).toBe(false);
    expect(shouldPauseAutoFollow(600, 601)).toBe(false);
  });
});

describe("history swipe", () => {
  test("allows a swipe to begin around the middle of the viewport", () => {
    expect(isHistorySwipeStart(0, 400)).toBe(true);
    expect(isHistorySwipeStart(200, 400)).toBe(true);
    expect(isHistorySwipeStart(280, 400)).toBe(false);
  });

  test("claims a horizontal rightward gesture from the left or middle", () => {
    expect(shouldClaimHistorySwipe({ startX: 200, dx: 20, dy: 4 }, 400)).toBe(
      true,
    );
    expect(shouldOpenHistoryDrawer({ startX: 200, dx: 50, dy: 8 }, 400)).toBe(
      true,
    );
  });

  test("rejects gestures that would conflict with ordinary scrolling", () => {
    expect(shouldClaimHistorySwipe({ startX: 280, dx: 70, dy: 2 }, 400)).toBe(
      false,
    );
    expect(shouldClaimHistorySwipe({ startX: 200, dx: -70, dy: 2 }, 400)).toBe(
      false,
    );
    expect(shouldClaimHistorySwipe({ startX: 200, dx: 20, dy: 18 }, 400)).toBe(
      false,
    );
    expect(shouldOpenHistoryDrawer({ startX: 200, dx: 40, dy: 2 }, 400)).toBe(
      false,
    );
  });
});

describe("markdown image privacy", () => {
  test("turns inline and reference images into explicit links", () => {
    expect(
      sanitizeMarkdownImages(
        "![Bridge](https://example.com/logo.png)\n![Chart][chart]\n[chart]: https://example.com/chart.png",
      ),
    ).toBe(
      "[Image: Bridge](https://example.com/logo.png)\n[Image: Chart][chart]\n[chart]: https://example.com/chart.png",
    );
  });

  test("does not rewrite image syntax inside inline or fenced code", () => {
    const markdown =
      "`![inline](https://example.com/a.png)`\n```\n![block](https://example.com/b.png)\n```";
    expect(sanitizeMarkdownImages(markdown)).toBe(markdown);
  });

  test("leaves tables and incomplete streaming fragments intact", () => {
    const markdown =
      "| Name | Value |\n| --- | --- |\n| Image | ![partial](https://example.com";
    expect(sanitizeMarkdownImages(markdown)).toBe(markdown);
  });

  test("turns HTML images into explicit links", () => {
    expect(
      sanitizeMarkdownImages(
        '<img src="https://example.com/a.png" alt="Example">',
      ),
    ).toBe("[Image: Example](https://example.com/a.png)");
  });
});
