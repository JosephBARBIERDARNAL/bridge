import { describe, expect, test } from "bun:test";
import { MockBridgeClient } from "./mock";

describe("MockBridgeClient", () => {
  test("creates and renames a chat", async () => {
    const client = new MockBridgeClient();
    const chat = await client.createChat();
    expect(chat.title).toBe("New chat");
    expect((await client.renameChat(chat.id, "Private notes")).title).toBe(
      "Private notes",
    );
  });

  test("streams a response", async () => {
    const client = new MockBridgeClient();
    const chat = await client.createChat();
    const completed = new Promise<string>((resolve) =>
      client.sendMessage(chat.id, "Hello", {
        onStarted() {},
        onDelta() {},
        onError(error) {
          throw new Error(error.message);
        },
        onCompleted(message) {
          resolve(message.content);
        },
      }),
    );
    expect(await completed).toContain("local preview response");
  });
});
