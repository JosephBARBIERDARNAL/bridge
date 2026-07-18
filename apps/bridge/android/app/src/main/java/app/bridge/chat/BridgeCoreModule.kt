package app.bridge.chat

import app.bridge.chat.core.ChatDetail
import app.bridge.chat.core.ChatSummary
import app.bridge.chat.core.BridgeClient
import app.bridge.chat.core.HealthStatus
import app.bridge.chat.core.Message
import app.bridge.chat.core.MessageStreamListener
import app.bridge.chat.core.RequestHandle
import app.bridge.chat.core.StreamFailure
import com.facebook.react.bridge.Arguments
import com.facebook.react.bridge.NativeModule
import com.facebook.react.bridge.Promise
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.facebook.react.bridge.WritableMap
import com.facebook.react.modules.core.DeviceEventManagerModule
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import java.util.concurrent.ConcurrentHashMap

class BridgeCoreModule(private val context: ReactApplicationContext) : ReactContextBaseJavaModule(context) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val store = CredentialStore(context)
    private val requests = ConcurrentHashMap<String, ManagedRequest>()
    @Volatile private var client: BridgeClient? = store.load()?.let { runCatching { BridgeClient(it.baseUrl, it.token) }.getOrNull() }

    override fun getName(): String = "BridgeCore"

    @ReactMethod
    fun configure(baseUrl: String, token: String, promise: Promise) = launch(promise) {
        val configured = BridgeClient(baseUrl, token)
        client?.close()
        client = configured
        store.save(baseUrl, token)
        true
    }

    @ReactMethod fun health(promise: Promise) = launch(promise) { healthMap(requireClient().health()) }
    @ReactMethod fun listChats(promise: Promise) = launch(promise) { Arguments.createArray().apply { requireClient().listChats().forEach { pushMap(chatMap(it)) } } }
    @ReactMethod fun createChat(promise: Promise) = launch(promise) { chatMap(requireClient().createChat()) }
    @ReactMethod fun getChat(id: String, promise: Promise) = launch(promise) { detailMap(requireClient().getChat(id)) }
    @ReactMethod fun renameChat(id: String, title: String, promise: Promise) = launch(promise) { chatMap(requireClient().renameChat(id, title)) }
    @ReactMethod fun deleteChat(id: String, promise: Promise) = launch(promise) { requireClient().deleteChat(id); null }

    @ReactMethod
    fun sendMessage(requestId: String, chatId: String, content: String, webSearch: Boolean, promise: Promise) = start(requestId, promise) { listener ->
        requireClient().sendMessage(chatId, content, webSearch, listener)
    }

    @ReactMethod
    fun retryMessage(requestId: String, chatId: String, messageId: String, webSearch: Boolean, promise: Promise) = start(requestId, promise) { listener ->
        requireClient().retryMessage(chatId, messageId, webSearch, listener)
    }

    @ReactMethod
    fun cancel(requestId: String, promise: Promise) = launch(promise) {
        requests.remove(requestId)?.finish(cancel = true)
        null
    }

    @ReactMethod fun addListener(eventName: String) = Unit
    @ReactMethod fun removeListeners(count: Double) = Unit

    private fun start(requestId: String, promise: Promise, operation: (MessageStreamListener) -> RequestHandle) {
        val request = ManagedRequest()
        if (requests.putIfAbsent(requestId, request) != null) {
            promise.reject("bridge_error", "Duplicate stream request ID")
            return
        }
        try {
            val listener = object : MessageStreamListener {
                override fun onStarted(userMessageId: String, assistantMessageId: String) = emit(requestId, "started", Arguments.createMap().apply { putString("userMessageId", userMessageId); putString("assistantMessageId", assistantMessageId) })
                override fun onThinkingDelta(assistantMessageId: String, text: String) = emit(requestId, "thinking_delta", Arguments.createMap().apply { putString("assistantMessageId", assistantMessageId); putString("text", text) })
                override fun onDelta(assistantMessageId: String, text: String) = emit(requestId, "delta", Arguments.createMap().apply { putString("assistantMessageId", assistantMessageId); putString("text", text) })
                override fun onToolCall(assistantMessageId: String, callIndex: UInt, name: String, argumentsJson: String) = emit(requestId, "tool_call", Arguments.createMap().apply { putString("assistantMessageId", assistantMessageId); putInt("callIndex", callIndex.toInt()); putString("name", name); putString("argumentsJson", argumentsJson) })
                override fun onToolResult(assistantMessageId: String, callIndex: UInt, name: String, recordJson: String) = emit(requestId, "tool_result", Arguments.createMap().apply { putString("assistantMessageId", assistantMessageId); putInt("callIndex", callIndex.toInt()); putString("name", name); putString("recordJson", recordJson) })
                override fun onCompleted(message: Message) { emit(requestId, "completed", Arguments.createMap().apply { putMap("message", messageMap(message)) }); requests.remove(requestId)?.finish(cancel = false) }
                override fun onError(error: StreamFailure) { emit(requestId, "error", Arguments.createMap().apply { putMap("error", errorMap(error)) }); requests.remove(requestId)?.finish(cancel = false) }
            }
            request.attach(operation(listener))
            promise.resolve(null)
        } catch (error: Throwable) {
            requests.remove(requestId)?.finish(cancel = true)
            promise.reject("bridge_error", error.message, error)
        }
    }

    private fun emit(requestId: String, type: String, data: WritableMap) {
        data.putString("requestId", requestId); data.putString("type", type)
        context.getJSModule(DeviceEventManagerModule.RCTDeviceEventEmitter::class.java).emit("BridgeStreamEvent", data)
    }

    private fun requireClient() = client ?: error("Bridge is not configured. Add the Tailscale URL and API token in Settings.")
    private fun launch(promise: Promise, block: () -> Any?) { scope.launch { runCatching(block).onSuccess(promise::resolve).onFailure { promise.reject("bridge_error", it.message, it) } } }

    override fun invalidate() {
        requests.values.forEach { it.finish(cancel = true) }; requests.clear(); client?.close(); scope.cancel(); super.invalidate()
    }

    private fun chatMap(value: ChatSummary) = Arguments.createMap().apply { putString("id", value.id); putString("title", value.title); putString("created_at", value.createdAt); putString("updated_at", value.updatedAt) }
    private fun messageMap(value: Message) = Arguments.createMap().apply { putString("id", value.id); putString("chat_id", value.chatId); putString("role", value.role); putString("content", value.content); putString("thinking", value.thinking); putString("tool_calls", value.toolCalls); putString("status", value.status); putString("created_at", value.createdAt) }
    private fun detailMap(value: ChatDetail) = Arguments.createMap().apply { putMap("chat", chatMap(value.chat)); putArray("messages", Arguments.createArray().apply { value.messages.forEach { pushMap(messageMap(it)) } }) }
    private fun healthMap(value: HealthStatus) = Arguments.createMap().apply { putString("gateway", value.gateway); putString("database", value.database); putString("ollama", value.ollama); putString("model", value.model); putBoolean("model_available", value.modelAvailable) }
    private fun errorMap(value: StreamFailure) = Arguments.createMap().apply { putString("code", value.code); putString("message", value.message); putBoolean("retryable", value.retryable) }
}

private class ManagedRequest {
    private var handle: RequestHandle? = null
    private var finished = false

    @Synchronized
    fun attach(value: RequestHandle) {
        if (finished) value.close() else handle = value
    }

    @Synchronized
    fun finish(cancel: Boolean) {
        if (finished) return
        finished = true
        handle?.let {
            if (cancel) it.cancel()
            it.close()
        }
        handle = null
    }
}
