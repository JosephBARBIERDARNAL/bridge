import { useMemo } from "react";
import { Linking } from "react-native";
import {
  EnrichedMarkdownText,
  type MarkdownStyle,
} from "react-native-enriched-markdown";
import { sanitizeMarkdownImages } from "./chatUi";
import type { light } from "./theme";

type Props = {
  content: string;
  colors: typeof light;
};

export default function MarkdownText({ content, colors }: Props) {
  const markdown = useMemo(() => sanitizeMarkdownImages(content), [content]);
  const style = useMemo(() => markdownStyle(colors), [colors]);

  return (
    <EnrichedMarkdownText
      flavor="github"
      markdown={markdown}
      markdownStyle={style}
      md4cFlags={{ latexMath: false }}
      selectable
      onLinkPress={({ url }) => void Linking.openURL(url)}
    />
  );
}

function markdownStyle(colors: typeof light): MarkdownStyle {
  const text = {
    color: colors.text,
    fontSize: 15.5,
    lineHeight: 24,
  };
  return {
    paragraph: { ...text, marginBottom: 10 },
    h1: { ...text, fontSize: 25, fontWeight: "700", marginBottom: 12 },
    h2: { ...text, fontSize: 21, fontWeight: "700", marginBottom: 10 },
    h3: { ...text, fontSize: 18, fontWeight: "700", marginBottom: 8 },
    h4: { ...text, fontWeight: "700", marginBottom: 7 },
    h5: { ...text, fontSize: 14.5, fontWeight: "700", marginBottom: 6 },
    h6: { ...text, fontSize: 13.5, fontWeight: "700", marginBottom: 6 },
    blockquote: {
      ...text,
      borderColor: colors.muted,
      borderWidth: 3,
      gapWidth: 12,
      backgroundColor: colors.surface,
      marginBottom: 10,
    },
    list: {
      ...text,
      bulletColor: colors.text,
      markerColor: colors.text,
      gapWidth: 8,
      marginBottom: 10,
    },
    strong: { color: colors.text },
    em: { color: colors.text },
    link: { color: colors.text, underline: true },
    code: {
      color: colors.codeText,
      backgroundColor: colors.code,
      borderColor: colors.border,
      fontFamily: "monospace",
      fontSize: 14,
    },
    codeBlock: {
      color: colors.codeText,
      backgroundColor: colors.code,
      borderColor: colors.border,
      borderWidth: 1,
      borderRadius: 10,
      padding: 14,
      fontFamily: "monospace",
      fontSize: 14,
      lineHeight: 20,
      marginBottom: 10,
    },
    thematicBreak: {
      color: colors.border,
      height: 1,
      marginTop: 8,
      marginBottom: 14,
    },
    table: {
      ...text,
      borderColor: colors.border,
      borderWidth: 1,
      borderRadius: 8,
      headerBackgroundColor: colors.surface,
      headerTextColor: colors.text,
      rowEvenBackgroundColor: colors.background,
      rowOddBackgroundColor: colors.surface,
      cellPaddingHorizontal: 10,
      cellPaddingVertical: 8,
      marginBottom: 10,
    },
    taskList: {
      checkedColor: colors.muted,
      borderColor: colors.muted,
      checkmarkColor: colors.accentText,
      checkedTextColor: colors.muted,
    },
  };
}
