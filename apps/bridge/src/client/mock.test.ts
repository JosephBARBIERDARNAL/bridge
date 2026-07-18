import { describe, expect, test } from "bun:test";
import { MockBridgeClient } from "./mock";
import { parseToolCalls } from "../types";

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
    let thinking = "";
    const completed = new Promise<string>((resolve) =>
      client.sendMessage(chat.id, "Hello", false, {
        onStarted() {},
        onThinkingDelta(_messageId, text) {
          thinking += text;
        },
        onDelta() {},
        onToolCall() {},
        onToolResult() {},
        onError(error) {
          throw new Error(error.message);
        },
        onCompleted(message) {
          resolve(message.content);
        },
      }),
    );
    expect(await completed).toContain("local preview response");
    expect(thinking).toContain("Preparing");
    expect((await client.getChat(chat.id)).messages.at(-1)?.thinking).toBe(
      thinking,
    );
  });

  test("simulates a web search round when the toggle is on", async () => {
    const client = new MockBridgeClient();
    const chat = await client.createChat();
    const toolCalls: string[] = [];
    const toolResults: string[] = [];
    const completed = new Promise<string>((resolve, reject) =>
      client.sendMessage(chat.id, "What is new today?", true, {
        onStarted() {},
        onThinkingDelta() {},
        onDelta() {},
        onToolCall(_messageId, callIndex, name, argumentsJson) {
          toolCalls.push(`${callIndex}:${name}:${argumentsJson}`);
        },
        onToolResult(_messageId, _callIndex, _name, recordJson) {
          toolResults.push(recordJson);
        },
        onError(error) {
          reject(new Error(error.message));
        },
        onCompleted(message) {
          resolve(message.tool_calls);
        },
      }),
    );
    const persisted = await completed;
    expect(toolCalls).toHaveLength(1);
    expect(toolCalls[0]).toContain("0:web_search");
    expect(toolResults).toHaveLength(1);
    const records = parseToolCalls(persisted);
    expect(records).toHaveLength(1);
    expect(records[0].name).toBe("web_search");
    expect(records[0].status).toBe("ok");
    expect(records[0].sources?.length).toBe(2);
    expect(JSON.parse(toolResults[0])).toEqual(records[0]);
  });

  test("keeps chat histories isolated", async () => {
    const client = new MockBridgeClient();
    const first = await client.createChat();
    const second = await client.createChat();
    const completed = new Promise<void>((resolve, reject) =>
      client.sendMessage(first.id, "Only in the first chat", false, {
        onStarted() {},
        onThinkingDelta() {},
        onDelta() {},
        onToolCall() {},
        onToolResult() {},
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

describe("parseToolCalls", () => {
  test("is safe on empty and malformed input", () => {
    expect(parseToolCalls("")).toEqual([]);
    expect(parseToolCalls(undefined)).toEqual([]);
    expect(parseToolCalls("not json")).toEqual([]);
    expect(parseToolCalls('{"an":"object"}')).toEqual([]);
    expect(parseToolCalls('[{"name":"web_search"}]')).toHaveLength(1);
  });
});
