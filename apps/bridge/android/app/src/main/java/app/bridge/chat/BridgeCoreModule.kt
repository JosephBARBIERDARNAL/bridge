package app.bridge.chat

import app.bridge.chat.core.ChatDetail
import app.bridge.chat.core.ChatSummary
import app.bridge.chat.core.GemmaClient
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
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap

class BridgeCoreModule(private val context: ReactApplicationContext) : ReactContextBaseJavaModule(context) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val store = CredentialStore(context)
    private val requests = ConcurrentHashMap<String, RequestHandle>()
    @Volatile private var client: GemmaClient? = store.load()?.let { runCatching { GemmaClient(it.baseUrl, it.token) }.getOrNull() }

    override fun getName(): String = "BridgeCore"

    @ReactMethod
    fun configure(baseUrl: String, token: String, promise: Promise) = launch(promise) {
        val configured = GemmaClient(baseUrl, token)
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
    fun sendMessage(chatId: String, content: String, promise: Promise) = start(promise) { listener ->
        requireClient().sendMessage(chatId, content, listener)
    }

    @ReactMethod
    fun retryMessage(chatId: String, messageId: String, promise: Promise) = start(promise) { listener ->
        requireClient().retryMessage(chatId, messageId, listener)
    }

    @ReactMethod
    fun cancel(requestId: String, promise: Promise) = launch(promise) {
        requests.remove(requestId)?.let { it.cancel(); it.close() }
        null
    }

    @ReactMethod fun addListener(eventName: String) = Unit
    @ReactMethod fun removeListeners(count: Double) = Unit

    private fun start(promise: Promise, operation: (MessageStreamListener) -> RequestHandle) {
        try {
            val requestId = UUID.randomUUID().toString()
            val listener = object : MessageStreamListener {
                override fun onStarted(userMessageId: String, assistantMessageId: String) = emit(requestId, "started", Arguments.createMap().apply { putString("userMessageId", userMessageId); putString("assistantMessageId", assistantMessageId) })
                override fun onDelta(assistantMessageId: String, text: String) = emit(requestId, "delta", Arguments.createMap().apply { putString("assistantMessageId", assistantMessageId); putString("text", text) })
                override fun onCompleted(message: Message) { emit(requestId, "completed", Arguments.createMap().apply { putMap("message", messageMap(message)) }); requests.remove(requestId)?.close() }
                override fun onError(error: StreamFailure) { emit(requestId, "error", Arguments.createMap().apply { putMap("error", errorMap(error)) }); requests.remove(requestId)?.close() }
            }
            requests[requestId] = operation(listener)
            promise.resolve(requestId)
        } catch (error: Throwable) { promise.reject("bridge_error", error.message, error) }
    }

    private fun emit(requestId: String, type: String, data: WritableMap) {
        data.putString("requestId", requestId); data.putString("type", type)
        context.getJSModule(DeviceEventManagerModule.RCTDeviceEventEmitter::class.java).emit("BridgeStreamEvent", data)
    }

    private fun requireClient() = client ?: error("Bridge is not configured. Add the Tailscale URL and API token in Settings.")
    private fun launch(promise: Promise, block: () -> Any?) { scope.launch { runCatching(block).onSuccess(promise::resolve).onFailure { promise.reject("bridge_error", it.message, it) } } }

    override fun invalidate() {
        requests.values.forEach { it.cancel(); it.close() }; requests.clear(); client?.close(); scope.cancel(); super.invalidate()
    }

    private fun chatMap(value: ChatSummary) = Arguments.createMap().apply { putString("id", value.id); putString("title", value.title); putString("created_at", value.createdAt); putString("updated_at", value.updatedAt) }
    private fun messageMap(value: Message) = Arguments.createMap().apply { putString("id", value.id); putString("chat_id", value.chatId); putString("role", value.role); putString("content", value.content); putString("status", value.status); putString("created_at", value.createdAt) }
    private fun detailMap(value: ChatDetail) = Arguments.createMap().apply { putMap("chat", chatMap(value.chat)); putArray("messages", Arguments.createArray().apply { value.messages.forEach { pushMap(messageMap(it)) } }) }
    private fun healthMap(value: HealthStatus) = Arguments.createMap().apply { putString("gateway", value.gateway); putString("database", value.database); putString("ollama", value.ollama); putString("model", value.model); putBoolean("model_available", value.modelAvailable) }
    private fun errorMap(value: StreamFailure) = Arguments.createMap().apply { putString("code", value.code); putString("message", value.message); putBoolean("retryable", value.retryable) }
}

