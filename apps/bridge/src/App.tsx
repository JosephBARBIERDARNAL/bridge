import React, { useEffect, useMemo, useRef, useState } from "react";
import {
  ActivityIndicator,
  Alert,
  Appearance,
  KeyboardAvoidingView,
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  useWindowDimensions,
  View,
} from "react-native";
import MarkdownText from "./MarkdownText";
import { createClient } from "./client";
import type {
  BridgeClient,
  ChatSummary,
  Message,
  RequestHandle,
  StreamFailure,
} from "./types";
import { dark, light } from "./theme";

const client = createClient();
const empty =
  "Start a conversation with Gemma 4. Your messages stay on your Mac.";

export default function App() {
  const systemDark = Appearance.getColorScheme() === "dark";
  const [darkMode, setDarkMode] = useState(systemDark);
  const colors = darkMode ? dark : light;
  const styles = useMemo(() => makeStyles(colors), [colors]);
  const { width } = useWindowDimensions();
  const compact = width < 760;
  const [drawer, setDrawer] = useState(!compact);
  const [chats, setChats] = useState<ChatSummary[]>([]);
  const [activeId, setActiveId] = useState<string>();
  const [messages, setMessages] = useState<Message[]>([]);
  const [prompt, setPrompt] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<StreamFailure>();
  const [loading, setLoading] = useState(true);
  const [settings, setSettings] = useState(false);
  const [settingsBusy, setSettingsBusy] = useState(false);
  const [settingsError, setSettingsError] = useState<string>();
  const [baseUrl, setBaseUrl] = useState(
    "https://your-mac.your-tailnet.ts.net",
  );
  const [token, setToken] = useState("");
  const request = useRef<RequestHandle | undefined>(undefined);
  const scroll = useRef<ScrollView>(null);

  const refreshChats = async (selectFirst = false) => {
    const values = await client.listChats();
    setChats(values);
    if (selectFirst && !activeId && values[0]) setActiveId(values[0].id);
  };

  useEffect(() => {
    void refreshChats(true)
      .catch((value) => {
        showError(value);
        if (Platform.OS !== "web") setSettings(true);
      })
      .finally(() => setLoading(false));
  }, []);
  useEffect(() => {
    if (!activeId) {
      setMessages([]);
      return;
    }
    setLoading(true);
    void client
      .getChat(activeId)
      .then((detail) => setMessages(detail.messages))
      .catch(showError)
      .finally(() => setLoading(false));
    if (compact) setDrawer(false);
  }, [activeId]);
  useEffect(() => {
    if (busy) scroll.current?.scrollToEnd({ animated: true });
  }, [messages, busy]);

  function showError(value: unknown) {
    setError({
      code: "request_failed",
      message: value instanceof Error ? value.message : String(value),
      retryable: true,
    });
  }

  async function newChat() {
    try {
      const chat = await client.createChat();
      setChats((current) => [chat, ...current]);
      setActiveId(chat.id);
    } catch (value) {
      showError(value);
    }
  }

  async function removeChat(chat: ChatSummary) {
    const remove = async () => {
      await client.deleteChat(chat.id);
      const next = chats.filter((value) => value.id !== chat.id);
      setChats(next);
      if (activeId === chat.id) setActiveId(next[0]?.id);
    };
    if (Platform.OS === "web") {
      if (confirm(`Delete “${chat.title}”?`)) await remove();
    } else
      Alert.alert("Delete chat?", chat.title, [
        { text: "Cancel" },
        { text: "Delete", style: "destructive", onPress: () => void remove() },
      ]);
  }

  async function renameChat(chat: ChatSummary) {
    if (Platform.OS !== "web")
      return Alert.alert("Rename from the web preview for now");
    const title = promptDialog("Rename chat", chat.title);
    if (!title?.trim()) return;
    const updated = await client.renameChat(chat.id, title.trim());
    setChats((current) =>
      current.map((value) => (value.id === updated.id ? updated : value)),
    );
  }

  async function ensureChat() {
    if (activeId) return activeId;
    const chat = await client.createChat();
    setChats((current) => [chat, ...current]);
    setActiveId(chat.id);
    return chat.id;
  }

  async function send() {
    const content = prompt.trim();
    if (!content || busy) return;
    try {
      const chatId = await ensureChat();
      const user: Message = {
        id: `pending-${Date.now()}`,
        chat_id: chatId,
        role: "user",
        content,
        status: "complete",
        created_at: new Date().toISOString(),
      };
      setPrompt("");
      setError(undefined);
      setBusy(true);
      setMessages((current) => [...current, user]);
      request.current = client.sendMessage(
        chatId,
        content,
        streamListener(chatId),
      );
    } catch (value) {
      setBusy(false);
      showError(value);
    }
  }

  function streamListener(chatId: string) {
    return {
      onStarted(userId: string, assistantId: string) {
        setMessages((current) => [
          ...current.map((value) =>
            value.id.startsWith("pending-") ? { ...value, id: userId } : value,
          ),
          {
            id: assistantId,
            chat_id: chatId,
            role: "assistant",
            content: "",
            status: "streaming",
            created_at: new Date().toISOString(),
          },
        ]);
      },
      onDelta(assistantId: string, text: string) {
        setMessages((current) =>
          current.map((value) =>
            value.id === assistantId
              ? { ...value, content: value.content + text }
              : value,
          ),
        );
      },
      onCompleted(message: Message) {
        setMessages((current) =>
          current.map((value) => (value.id === message.id ? message : value)),
        );
        setBusy(false);
        void refreshChats();
      },
      onError(value: StreamFailure) {
        setError(value);
        setBusy(false);
      },
    };
  }

  function stop() {
    request.current?.cancel();
    request.current = undefined;
    setBusy(false);
  }

  function retryLast() {
    if (!activeId || busy) return;
    const user = [...messages]
      .reverse()
      .find((message) => message.role === "user");
    if (!user) return;
    setError(undefined);
    setBusy(true);
    request.current = client.retryMessage(
      activeId,
      user.id,
      streamListener(activeId),
    );
  }

  async function saveSettings() {
    if (settingsBusy) return;
    setSettingsBusy(true);
    setSettingsError(undefined);
    try {
      if (client.configure) await client.configure(baseUrl.trim(), token);
      await client.health();
      setSettings(false);
      setError(undefined);
      await refreshChats(true);
    } catch (value) {
      setSettingsError(
        value instanceof Error ? value.message : String(value),
      );
    } finally {
      setSettingsBusy(false);
    }
  }

  return (
    <View style={styles.app}>
      {drawer && (
        <View style={[styles.sidebar, compact && styles.sidebarOverlay]}>
          <View style={styles.brandRow}>
            <View style={styles.logo}>
              <Text style={styles.logoText}>B</Text>
            </View>
            <Text style={styles.brand}>Bridge</Text>
            <Pressable onPress={() => setDrawer(false)}>
              <Text style={styles.icon}>×</Text>
            </Pressable>
          </View>
          <Pressable style={styles.newButton} onPress={() => void newChat()}>
            <Text style={styles.newButtonText}>＋ New chat</Text>
          </Pressable>
          <ScrollView style={styles.chatList}>
            {chats.map((chat) => (
              <Pressable
                key={chat.id}
                style={[
                  styles.chatItem,
                  activeId === chat.id && styles.chatActive,
                ]}
                onPress={() => setActiveId(chat.id)}
              >
                <Text numberOfLines={1} style={styles.chatTitle}>
                  {chat.title}
                </Text>
                <View style={styles.chatActions}>
                  <Pressable onPress={() => void renameChat(chat)}>
                    <Text style={styles.smallIcon}>✎</Text>
                  </Pressable>
                  <Pressable onPress={() => void removeChat(chat)}>
                    <Text style={[styles.smallIcon, styles.danger]}>×</Text>
                  </Pressable>
                </View>
              </Pressable>
            ))}
          </ScrollView>
          <Pressable
            style={styles.themeButton}
            onPress={() => setSettings(true)}
          >
            <Text style={styles.secondaryText}>⚙ Connection settings</Text>
          </Pressable>
          <Pressable
            style={styles.themeButton}
            onPress={() => setDarkMode((value) => !value)}
          >
            <Text style={styles.secondaryText}>
              {darkMode ? "☀  Light mode" : "☾  Dark mode"}
            </Text>
          </Pressable>
          <View style={styles.privateRow}>
            <Text style={styles.privateDot}>●</Text>
            <Text style={styles.secondaryText}>Private · Mac hosted</Text>
          </View>
        </View>
      )}
      <KeyboardAvoidingView
        style={styles.main}
        behavior={Platform.OS === "ios" ? "padding" : undefined}
      >
        <View style={styles.header}>
          <Pressable onPress={() => setDrawer((value) => !value)}>
            <Text style={styles.icon}>☰</Text>
          </Pressable>
          <View>
            <Text style={styles.headerTitle}>
              {chats.find((chat) => chat.id === activeId)?.title ?? "New chat"}
            </Text>
            <Text style={styles.model}>Gemma 4 · 26B</Text>
          </View>
          <View style={styles.online}>
            <Text style={styles.onlineText}>● Local</Text>
          </View>
        </View>
        <ScrollView
          ref={scroll}
          style={styles.messages}
          contentContainerStyle={styles.messageContent}
          keyboardShouldPersistTaps="handled"
        >
          {loading ? (
            <ActivityIndicator color={colors.text} />
          ) : messages.length === 0 ? (
            <View style={styles.empty}>
              <View style={styles.heroLogo}>
                <Text style={styles.heroText}>B</Text>
              </View>
              <Text style={styles.emptyTitle}>How can I help?</Text>
              <Text style={styles.emptyBody}>{empty}</Text>
            </View>
          ) : (
            messages.map((message) => (
              <MessageView
                key={message.id}
                message={message}
                styles={styles}
                colors={colors}
              />
            ))
          )}
          {error && (
            <View style={styles.error}>
              <Text style={styles.errorTitle}>
                Couldn’t complete the response
              </Text>
              <Text style={styles.errorText}>{error.message}</Text>
              {error.retryable && (
                <Pressable onPress={retryLast}>
                  <Text style={styles.retry}>Retry response</Text>
                </Pressable>
              )}
            </View>
          )}
        </ScrollView>
        <View style={styles.composerWrap}>
          <View style={styles.composer}>
            <TextInput
              multiline
              value={prompt}
              onChangeText={setPrompt}
              placeholder="Message Gemma 4…"
              placeholderTextColor={colors.muted}
              style={styles.input}
              onKeyPress={(event) => {
                if (
                  Platform.OS === "web" &&
                  event.nativeEvent.key === "Enter" &&
                  !(event.nativeEvent as unknown as { shiftKey?: boolean })
                    .shiftKey
                ) {
                  event.preventDefault?.();
                  void send();
                }
              }}
            />
            <Pressable
              style={[
                styles.send,
                (!prompt.trim() || busy) && styles.sendDisabled,
              ]}
              onPress={busy ? stop : () => void send()}
            >
              <Text style={styles.sendText}>{busy ? "■" : "↑"}</Text>
            </Pressable>
          </View>
          <Text style={styles.disclaimer}>
            Gemma can make mistakes. Your conversations stay on your Mac.
          </Text>
        </View>
      </KeyboardAvoidingView>
      {settings && (
        <View style={styles.modalBackdrop}>
          <View style={styles.modal}>
            <View style={styles.modalHeader}>
              <Text style={styles.modalTitle}>Connection settings</Text>
              <Pressable onPress={() => setSettings(false)}>
                <Text style={styles.icon}>×</Text>
              </Pressable>
            </View>
            <Text style={styles.label}>Tailscale HTTPS URL</Text>
            <TextInput
              autoCapitalize="none"
              autoCorrect={false}
              value={baseUrl}
              onChangeText={setBaseUrl}
              style={styles.settingsInput}
              placeholderTextColor={colors.muted}
            />
            <Text style={styles.label}>API token</Text>
            <TextInput
              autoCapitalize="none"
              autoCorrect={false}
              secureTextEntry
              value={token}
              onChangeText={setToken}
              style={styles.settingsInput}
              placeholder="Stored in Android Keystore"
              placeholderTextColor={colors.muted}
            />
            <Text style={styles.settingsHelp}>
              {Platform.OS === "web"
                ? "Real browser mode uses the token from .env through the Vite proxy."
                : "The token is encrypted by Android Keystore and is never stored in JavaScript."}
            </Text>
            {settingsError && (
              <View style={styles.settingsError}>
                <Text style={styles.settingsErrorText}>{settingsError}</Text>
              </View>
            )}
            <Pressable
              disabled={settingsBusy}
              style={[
                styles.saveButton,
                settingsBusy && styles.saveButtonDisabled,
              ]}
              onPress={() => void saveSettings()}
            >
              {settingsBusy ? (
                <View style={styles.testingRow}>
                  <ActivityIndicator size="small" color={colors.accentText} />
                  <Text style={styles.newButtonText}>Testing connection…</Text>
                </View>
              ) : (
                <Text style={styles.newButtonText}>Save and test</Text>
              )}
            </Pressable>
          </View>
        </View>
      )}
    </View>
  );
}

function MessageView({
  message,
  styles,
  colors,
}: {
  message: Message;
  styles: ReturnType<typeof makeStyles>;
  colors: typeof light;
}) {
  const user = message.role === "user";
  return (
    <View style={[styles.messageRow, user && styles.userRow]}>
      {!user && (
        <View style={styles.avatar}>
          <Text style={styles.avatarText}>B</Text>
        </View>
      )}
      <View style={[styles.messageBubble, user && styles.userBubble]}>
        {user ? (
          <Text selectable style={styles.messageText}>
            {message.content}
          </Text>
        ) : (
          <MarkdownText
            content={message.content || "▍"}
            textStyle={styles.messageText}
            codeStyle={{ backgroundColor: colors.code, color: colors.codeText }}
          />
        )}
        {message.status === "failed" && (
          <Text style={styles.failed}>Generation interrupted</Text>
        )}
      </View>
    </View>
  );
}

const promptDialog = (title: string, value: string) =>
  globalThis.prompt?.(title, value);

function makeStyles(c: typeof light) {
  return StyleSheet.create({
    app: { flex: 1, flexDirection: "row", backgroundColor: c.background },
    sidebar: {
      width: 292,
      backgroundColor: c.sidebar,
      padding: 14,
      borderRightWidth: 1,
      borderRightColor: c.border,
      zIndex: 5,
    },
    sidebarOverlay: {
      position: "absolute",
      top: 0,
      bottom: 0,
      left: 0,
      shadowColor: "#000",
      shadowOpacity: 0.25,
      shadowRadius: 20,
    },
    brandRow: {
      height: 52,
      flexDirection: "row",
      alignItems: "center",
      gap: 10,
    },
    logo: {
      width: 31,
      height: 31,
      borderRadius: 10,
      backgroundColor: c.accent,
      alignItems: "center",
      justifyContent: "center",
    },
    logoText: { color: c.accentText, fontWeight: "800" },
    brand: { color: c.text, fontSize: 18, fontWeight: "700", flex: 1 },
    icon: { fontSize: 22, color: c.text, padding: 8 },
    newButton: {
      backgroundColor: c.accent,
      paddingVertical: 13,
      paddingHorizontal: 14,
      borderRadius: 12,
      marginVertical: 10,
    },
    newButtonText: { color: c.accentText, fontWeight: "600" },
    chatList: { flex: 1, marginTop: 8 },
    chatItem: {
      minHeight: 46,
      borderRadius: 10,
      paddingHorizontal: 11,
      flexDirection: "row",
      alignItems: "center",
      marginBottom: 3,
    },
    chatActive: { backgroundColor: c.surface },
    chatTitle: { color: c.text, flex: 1, fontSize: 14 },
    chatActions: { flexDirection: "row", gap: 7 },
    smallIcon: { color: c.muted, padding: 5 },
    danger: { color: c.danger },
    themeButton: { padding: 12, borderTopWidth: 1, borderTopColor: c.border },
    privateRow: {
      flexDirection: "row",
      alignItems: "center",
      gap: 8,
      padding: 12,
    },
    privateDot: { color: "#46a758", fontSize: 10 },
    secondaryText: { color: c.muted, fontSize: 13 },
    main: { flex: 1 },
    header: {
      height: 68,
      backgroundColor: c.surface,
      borderBottomWidth: 1,
      borderBottomColor: c.border,
      paddingHorizontal: 15,
      flexDirection: "row",
      alignItems: "center",
      gap: 12,
    },
    headerTitle: { color: c.text, fontWeight: "600", fontSize: 15 },
    model: { color: c.muted, fontSize: 11, marginTop: 2 },
    online: {
      marginLeft: "auto",
      backgroundColor: c.background,
      borderRadius: 20,
      paddingVertical: 6,
      paddingHorizontal: 10,
    },
    onlineText: { color: "#398649", fontSize: 12, fontWeight: "600" },
    messages: { flex: 1 },
    messageContent: {
      width: "100%",
      maxWidth: 820,
      alignSelf: "center",
      paddingHorizontal: 20,
      paddingVertical: 28,
    },
    empty: {
      alignItems: "center",
      paddingTop: 100,
      maxWidth: 460,
      alignSelf: "center",
    },
    heroLogo: {
      width: 52,
      height: 52,
      borderRadius: 17,
      backgroundColor: c.accent,
      alignItems: "center",
      justifyContent: "center",
    },
    heroText: { color: c.accentText, fontWeight: "800", fontSize: 22 },
    emptyTitle: {
      color: c.text,
      fontSize: 27,
      fontWeight: "700",
      marginTop: 20,
    },
    emptyBody: {
      color: c.muted,
      textAlign: "center",
      lineHeight: 22,
      marginTop: 10,
    },
    messageRow: {
      flexDirection: "row",
      alignItems: "flex-start",
      gap: 12,
      marginBottom: 24,
    },
    userRow: { justifyContent: "flex-end" },
    avatar: {
      width: 28,
      height: 28,
      borderRadius: 9,
      backgroundColor: c.accent,
      alignItems: "center",
      justifyContent: "center",
    },
    avatarText: { color: c.accentText, fontWeight: "800", fontSize: 12 },
    messageBubble: { maxWidth: "88%", flexShrink: 1 },
    userBubble: {
      backgroundColor: c.user,
      paddingVertical: 11,
      paddingHorizontal: 15,
      borderRadius: 18,
      borderBottomRightRadius: 5,
    },
    messageText: { color: c.text, fontSize: 15.5, lineHeight: 24 },
    failed: { color: c.danger, fontSize: 12, marginTop: 8 },
    error: {
      backgroundColor: c.surface,
      borderColor: c.danger,
      borderWidth: 1,
      borderRadius: 12,
      padding: 13,
      marginBottom: 16,
    },
    errorTitle: { color: c.danger, fontWeight: "700" },
    errorText: { color: c.text, marginTop: 4 },
    retry: { color: c.text, fontWeight: "700", marginTop: 10 },
    composerWrap: {
      paddingHorizontal: 18,
      paddingBottom: 12,
      paddingTop: 7,
      backgroundColor: c.background,
    },
    composer: {
      width: "100%",
      maxWidth: 820,
      alignSelf: "center",
      flexDirection: "row",
      alignItems: "flex-end",
      backgroundColor: c.surface,
      borderColor: c.border,
      borderWidth: 1,
      borderRadius: 22,
      padding: 8,
      shadowColor: "#000",
      shadowOpacity: 0.07,
      shadowRadius: 10,
    },
    input: {
      color: c.text,
      flex: 1,
      minHeight: 38,
      maxHeight: 150,
      paddingHorizontal: 10,
      paddingVertical: 8,
      fontSize: 15,
      outlineStyle: "none",
    } as never,
    send: {
      width: 38,
      height: 38,
      borderRadius: 19,
      alignItems: "center",
      justifyContent: "center",
      backgroundColor: c.accent,
    },
    sendDisabled: { opacity: 0.35 },
    sendText: { color: c.accentText, fontSize: 18, fontWeight: "800" },
    disclaimer: {
      color: c.muted,
      fontSize: 10.5,
      textAlign: "center",
      marginTop: 7,
    },
    modalBackdrop: {
      position: "absolute",
      inset: 0,
      backgroundColor: "#00000070",
      zIndex: 20,
      alignItems: "center",
      justifyContent: "center",
      padding: 20,
    } as never,
    modal: {
      width: "100%",
      maxWidth: 520,
      backgroundColor: c.surface,
      borderRadius: 18,
      padding: 20,
      borderColor: c.border,
      borderWidth: 1,
    },
    modalHeader: {
      flexDirection: "row",
      alignItems: "center",
      marginBottom: 14,
    },
    modalTitle: { color: c.text, fontSize: 20, fontWeight: "700", flex: 1 },
    label: {
      color: c.text,
      fontSize: 13,
      fontWeight: "600",
      marginTop: 12,
      marginBottom: 6,
    },
    settingsInput: {
      color: c.text,
      backgroundColor: c.background,
      borderColor: c.border,
      borderWidth: 1,
      borderRadius: 10,
      padding: 12,
    },
    settingsHelp: {
      color: c.muted,
      fontSize: 12,
      lineHeight: 18,
      marginTop: 12,
    },
    settingsError: {
      backgroundColor: c.background,
      borderColor: c.danger,
      borderWidth: 1,
      borderRadius: 10,
      padding: 11,
      marginTop: 12,
    },
    settingsErrorText: { color: c.danger, fontSize: 13, lineHeight: 18 },
    saveButton: {
      backgroundColor: c.accent,
      borderRadius: 11,
      padding: 13,
      alignItems: "center",
      marginTop: 18,
    },
    saveButtonDisabled: { opacity: 0.7 },
    testingRow: { flexDirection: "row", alignItems: "center", gap: 9 },
  });
}
