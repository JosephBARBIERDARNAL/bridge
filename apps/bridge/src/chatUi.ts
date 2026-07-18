export const AUTO_FOLLOW_THRESHOLD = 72;
export const AUTO_FOLLOW_RESUME_THRESHOLD = 4;
export const HISTORY_SWIPE_START_RATIO = 0.65;
export const HISTORY_SWIPE_CLAIM_DISTANCE = 12;
export const HISTORY_SWIPE_OPEN_DISTANCE = 48;

type ScrollMetrics = {
  contentHeight: number;
  viewportHeight: number;
  offsetY: number;
};

type SwipeMetrics = {
  startX: number;
  dx: number;
  dy: number;
};

export function isNearBottom(
  metrics: ScrollMetrics,
  threshold = AUTO_FOLLOW_THRESHOLD,
) {
  const distance =
    metrics.contentHeight - metrics.viewportHeight - metrics.offsetY;
  return distance <= threshold;
}

export function shouldPauseAutoFollow(
  previousOffsetY: number,
  offsetY: number,
) {
  return offsetY < previousOffsetY - 0.5;
}

export function isHistorySwipeStart(startX: number, viewportWidth: number) {
  return (
    startX >= 0 &&
    viewportWidth > 0 &&
    startX <= viewportWidth * HISTORY_SWIPE_START_RATIO
  );
}

export function shouldClaimHistorySwipe(
  { startX, dx, dy }: SwipeMetrics,
  viewportWidth: number,
) {
  return (
    isHistorySwipeStart(startX, viewportWidth) &&
    dx >= HISTORY_SWIPE_CLAIM_DISTANCE &&
    dx > Math.abs(dy) * 1.5
  );
}

export function shouldOpenHistoryDrawer(
  metrics: SwipeMetrics,
  viewportWidth: number,
) {
  return (
    shouldClaimHistorySwipe(metrics, viewportWidth) &&
    metrics.dx >= HISTORY_SWIPE_OPEN_DISTANCE
  );
}

type ResponseMessage = {
  role: "user" | "assistant";
  content: string;
  thinking: string;
  status: "complete" | "streaming" | "failed";
};

export function shouldShowResponseWaiting(
  busy: boolean,
  messages: ResponseMessage[],
) {
  if (!busy) return false;
  const streamingAssistant = [...messages]
    .reverse()
    .find(
      (message) =>
        message.role === "assistant" && message.status === "streaming",
    );
  return (
    !streamingAssistant ||
    (!streamingAssistant.content && !streamingAssistant.thinking)
  );
}

export function sanitizeMarkdownImages(markdown: string) {
  let result = "";
  let offset = 0;
  let fence: { marker: string; length: number } | undefined;
  let inlineTicks = 0;

  while (offset < markdown.length) {
    const lineEnd = markdown.indexOf("\n", offset);
    const end = lineEnd === -1 ? markdown.length : lineEnd;
    const line = markdown.slice(offset, end);
    const fenceMatch = line.match(/^ {0,3}(`{3,}|~{3,})/);

    if (fence) {
      result += line;
      if (
        fenceMatch &&
        fenceMatch[1][0] === fence.marker &&
        fenceMatch[1].length >= fence.length &&
        line.slice(fenceMatch[0].length).trim() === ""
      )
        fence = undefined;
    } else if (inlineTicks === 0 && fenceMatch) {
      fence = {
        marker: fenceMatch[1][0],
        length: fenceMatch[1].length,
      };
      result += line;
    } else {
      const sanitized = sanitizeMarkdownLine(line, inlineTicks);
      result += sanitized.value;
      inlineTicks = sanitized.inlineTicks;
    }

    if (lineEnd !== -1) result += "\n";
    offset = lineEnd === -1 ? markdown.length : lineEnd + 1;
  }

  return result;
}

function sanitizeMarkdownLine(line: string, initialInlineTicks: number) {
  let value = "";
  let index = 0;
  let inlineTicks = initialInlineTicks;

  while (index < line.length) {
    if (line[index] === "`") {
      const length = runLength(line, index, "`");
      if (inlineTicks === 0) inlineTicks = length;
      else if (inlineTicks === length) inlineTicks = 0;
      value += line.slice(index, index + length);
      index += length;
      continue;
    }

    if (
      inlineTicks === 0 &&
      line.startsWith("![", index) &&
      !isEscaped(line, index)
    ) {
      const image = readMarkdownImage(line, index);
      if (image) {
        value += image.replacement;
        index = image.end;
        continue;
      }
    }

    if (
      inlineTicks === 0 &&
      line.slice(index, index + 4).toLowerCase() === "<img"
    ) {
      const image = readHtmlImage(line, index);
      if (image) {
        value += image.replacement;
        index = image.end;
        continue;
      }
    }

    value += line[index];
    index += 1;
  }

  return { value, inlineTicks };
}

function readMarkdownImage(value: string, start: number) {
  const labelEnd = findClosing(value, start + 1, "[", "]");
  if (labelEnd === -1) return undefined;

  const alt = value.slice(start + 2, labelEnd).trim();
  const label = alt ? `Image: ${alt}` : "Image";
  const next = labelEnd + 1;

  if (value[next] === "(") {
    const destinationEnd = findClosing(value, next, "(", ")");
    if (destinationEnd === -1) return undefined;
    return {
      replacement: `[${label}]${value.slice(next, destinationEnd + 1)}`,
      end: destinationEnd + 1,
    };
  }

  if (value[next] === "[") {
    const referenceEnd = findClosing(value, next, "[", "]");
    if (referenceEnd === -1) return undefined;
    return {
      replacement: `[${label}]${value.slice(next, referenceEnd + 1)}`,
      end: referenceEnd + 1,
    };
  }

  return { replacement: `[${label}]`, end: next };
}

function readHtmlImage(value: string, start: number) {
  const end = value.indexOf(">", start + 4);
  if (end === -1) return undefined;
  const tag = value.slice(start, end + 1);
  const alt = readHtmlAttribute(tag, "alt");
  const source = readHtmlAttribute(tag, "src");
  const label = alt ? `Image: ${alt}` : "Image";
  return {
    replacement: source ? `[${label}](${source})` : `[${label}]`,
    end: end + 1,
  };
}

function readHtmlAttribute(tag: string, name: string) {
  const match = tag.match(
    new RegExp(`\\s${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)'|([^\\s>]+))`, "i"),
  );
  return match?.[1] ?? match?.[2] ?? match?.[3];
}

function findClosing(
  value: string,
  start: number,
  opening: string,
  closing: string,
) {
  let depth = 0;
  for (let index = start; index < value.length; index += 1) {
    if (isEscaped(value, index)) continue;
    if (value[index] === opening) depth += 1;
    if (value[index] !== closing) continue;
    depth -= 1;
    if (depth === 0) return index;
  }
  return -1;
}

function isEscaped(value: string, index: number) {
  let slashes = 0;
  for (let cursor = index - 1; cursor >= 0 && value[cursor] === "\\"; cursor--)
    slashes += 1;
  return slashes % 2 === 1;
}

function runLength(value: string, start: number, character: string) {
  let end = start;
  while (value[end] === character) end += 1;
  return end - start;
}
