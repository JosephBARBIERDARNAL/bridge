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

  test("keeps chat histories isolated", async () => {
    const client = new MockBridgeClient();
    const first = await client.createChat();
    const second = await client.createChat();
    const completed = new Promise<void>((resolve, reject) =>
      client.sendMessage(first.id, "Only in the first chat", {
        onStarted() {},
        onDelta() {},
        onError(error) {
          reject(new Error(error.message));
        },
        onCompleted() {
          resolve();
        },
      }),
    );

    await completed;
    expect((await client.getChat(second.id)).messages).toEqual([]);
    expect(
      (await client.getChat(first.id)).messages.every(
        (message) => message.chat_id === first.id,
      ),
    ).toBe(true);
  });
});
