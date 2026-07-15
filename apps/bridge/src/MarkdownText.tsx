import React from "react";
import {
  StyleSheet,
  Text,
  type TextStyle,
  View,
  type ViewStyle,
} from "react-native";

type Props = {
  content: string;
  textStyle: TextStyle;
  codeStyle: ViewStyle & TextStyle;
};

export default function MarkdownText({ content, textStyle, codeStyle }: Props) {
  const blocks = content.split(/```/);
  return (
    <View>
      {blocks.map((block, index) =>
        index % 2 === 1 ? (
          <Text selectable key={index} style={[styles.code, codeStyle]}>
            {block.replace(/^\w+\n/, "")}
          </Text>
        ) : (
          block
            .split(/\n{2,}/)
            .filter(Boolean)
            .map((paragraph, paragraphIndex) => (
              <Text
                selectable
                key={`${index}-${paragraphIndex}`}
                style={[textStyle, styles.paragraph]}
              >
                {inline(paragraph)}
              </Text>
            ))
        ),
      )}
    </View>
  );
}

function inline(value: string) {
  return value.split(/(\*\*[^*]+\*\*)/g).map((part, index) =>
    part.startsWith("**") && part.endsWith("**") ? (
      <Text key={index} style={styles.bold}>
        {part.slice(2, -2)}
      </Text>
    ) : (
      part
    ),
  );
}

const styles = StyleSheet.create({
  paragraph: { marginBottom: 8 },
  bold: { fontWeight: "700" },
  code: {
    fontFamily: "monospace",
    padding: 14,
    borderRadius: 10,
    marginVertical: 7,
    lineHeight: 20,
  },
});
