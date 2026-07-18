import { afterEach, describe, expect, test } from "bun:test";
import type { Message, StreamListener } from "../types";
import { WebGatewayClient } from "./web";

const originalFetch = globalThis.fetch;

afterEach(() => {
  globalThis.fetch = originalFetch;
});

function responseFrom(chunks: Uint8Array[]) {
  return new Response(
    new ReadableStream({
      start(controller) {
        for (const chunk of chunks) controller.enqueue(chunk);
        controller.close();
      },
    }),
    { status: 200, headers: { "content-type": "text/event-stream" } },
  );
}

function listener(overrides: Partial<StreamListener> = {}): StreamListener {
  return {
    onStarted() {},
    onThinkingDelta() {},
    onDelta() {},
    onToolCall() {},
    onToolResult() {},
    onCompleted() {},
    onError() {},
    ...overrides,
  };
}

describe("WebGatewayClient streaming", () => {
  test("decodes split UTF-8 and completes on a terminal event", async () => {
    const completed: Message = {
      id: "assistant-1",
      chat_id: "chat-1",
      role: "assistant",
      content: "café 🦀",
      thinking: "",
      tool_calls: "",
      status: "complete",
      created_at: "2026-07-18T00:00:00Z",
    };
    const text =
      `event: delta\r\ndata: {"message_id":"assistant-1","text":"café 🦀"}\r\n\r\n` +
      `event: message_completed\r\ndata: ${JSON.stringify(completed)}\r\n\r\n`;
    const bytes = new TextEncoder().encode(text);
    globalThis.fetch = Object.assign(
      async () =>
        responseFrom([...bytes].map((byte) => new Uint8Array([byte]))),
      { preconnect: originalFetch.preconnect },
    );
    const client = new WebGatewayClient();
    let content = "";
    const done = new Promise<Message>((resolve, reject) =>
      client.sendMessage(
        "chat-1",
        "hello",
        false,
        listener({
          onDelta: (_id, value) => (content += value),
          onCompleted: resolve,
          onError: (error) => reject(new Error(error.message)),
        }),
      ),
    );

    expect((await done).content).toBe("café 🦀");
    expect(content).toBe("café 🦀");
  });

  test("reports clean EOF without a terminal event", async () => {
    const bytes = new TextEncoder().encode(
      `event: delta\ndata: {"message_id":"assistant-1","text":"partial"}\n\n`,
    );
    globalThis.fetch = Object.assign(async () => responseFrom([bytes]), {
      preconnect: originalFetch.preconnect,
    });
    const client = new WebGatewayClient();
    const failure = new Promise<string>((resolve) =>
      client.sendMessage(
        "chat-1",
        "hello",
        false,
        listener({ onError: (error) => resolve(error.message) }),
      ),
    );

    expect(await failure).toContain("ended unexpectedly");
  });
});
