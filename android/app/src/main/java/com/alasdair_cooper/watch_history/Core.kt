package com.alasdair_cooper.watch_history

import android.app.Application
import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.byteArrayPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.preferencesDataStore
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.ViewModelProvider.AndroidViewModelFactory.Companion.APPLICATION_KEY
import androidx.lifecycle.createSavedStateHandle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.alasdair_cooper.watch_history.shared.handleResponse
import com.alasdair_cooper.watch_history.shared.processEvent
import com.alasdair_cooper.watch_history.shared.view
import com.alasdair_cooper.watch_history.types.*
import com.alasdair_cooper.watch_history.types.HttpRequest
import com.novi.serde.Bytes
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import io.ktor.client.*
import io.ktor.client.call.*
import io.ktor.client.engine.cio.*
import io.ktor.client.request.*
import io.ktor.http.*
import io.ktor.util.*
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

@HiltViewModel
open class Core @Inject constructor(val keyValueStore: KeyValueStore) : androidx.lifecycle.ViewModel() {
    init {
        viewModelScope.launch { update(Event.InitialLoad()) }
    }

    var view: ViewModel? by mutableStateOf(null)
        private set

    private val _shellEvents = MutableSharedFlow<ShellEvent>()
    val shellEvents: SharedFlow<ShellEvent> = _shellEvents

    private val httpClient = HttpClient(CIO)

    suspend fun update(event: Event) {
        val effects = processEvent(event.bincodeSerialize())

        val requests = Requests.bincodeDeserialize(effects)
        for (request in requests) {
            processEffect(request)
        }
    }

    private suspend fun processEffect(request: Request) {
        when (val effect = request.effect) {
            is Effect.Render -> {
                this.view = ViewModel.bincodeDeserialize(view())
            }

            is Effect.Redirect -> {
                this.view = ViewModel.bincodeDeserialize(view())
                _shellEvents.emit(ShellEvent.OpenUrl(effect.value.url))
            }

            is Effect.Http -> {
                val response = requestHttp(httpClient, effect.value)

                val effects =
                    handleResponse(
                        request.id.toUInt(),
                        HttpResult.Ok(response).bincodeSerialize()
                    )

                val requests = Requests.bincodeDeserialize(effects)
                for (request in requests) {
                    processEffect(request)
                }
            }

            is Effect.KeyValue -> {
                val response =
                    handleKeyValueOperation(effect.value) ?: throw Exception("Unsupported KeyValue operation: $effect")

                val effects =
                    handleResponse(
                        request.id.toUInt(),
                        KeyValueResult.Ok(response).bincodeSerialize()
                    )

                val requests = Requests.bincodeDeserialize(effects)
                for (request in requests) {
                    processEffect(request)
                }
            }
        }
    }

    private suspend fun handleKeyValueOperation(
        operation: KeyValueOperation,
    ): KeyValueResponse? {
        return when (operation) {
            is KeyValueOperation.Get -> {
                val value = keyValueStore.get(operation.key)
                if (value != null) {
                    KeyValueResponse.Get(Value.Bytes(Bytes.valueOf(value)))
                } else {
                    KeyValueResponse.Get(Value.None())
                }
            }

            is KeyValueOperation.Set -> {
                keyValueStore.set(operation.key, operation.value.content())
                KeyValueResponse.Set(Value.None())
            }

            is KeyValueOperation.Delete -> {
                keyValueStore.delete(operation.key)
                KeyValueResponse.Delete(Value.None())
            }

            is KeyValueOperation.ListKeys -> {
                val keys = keyValueStore.listKeys()
                KeyValueResponse.ListKeys(keys, 0)
            }

            is KeyValueOperation.Exists -> {
                val exists = keyValueStore.exists(operation.key)
                KeyValueResponse.Exists(exists)
            }

            else -> null
        }
    }

    private suspend fun requestHttp(
        client: HttpClient,
        request: HttpRequest,
    ): HttpResponse {
        val response = client.request(request.url) {
            this.method = HttpMethod(request.method)
            this.headers {
                for (header in request.headers) {
                    append(header.name, header.value)
                }
            }
            request.body.let { bytes ->
                val bodyBytes = bytes.content()
                setBody(bodyBytes)
            }
        }
        val bytes = Bytes.valueOf(response.body())
        val headers = response.headers.flattenEntries().map { HttpHeader(it.first, it.second) }
        return HttpResponse(response.status.value.toShort(), headers, bytes)
    }

    sealed class ShellEvent {
        data class OpenUrl(val url: String) : ShellEvent()
    }
}

private val Context.dataStore by preferencesDataStore("settings")

@Singleton
class KeyValueStore @Inject constructor(@ApplicationContext context: Context) {
    private val dataStore = context.dataStore

    suspend fun get(key: String): ByteArray? {
        return dataStore.data
            .map { preferences -> preferences[byteArrayPreferencesKey(key)] }
            .first()
    }

    suspend fun set(key: String, value: ByteArray) {
        dataStore.edit { preferences ->
            preferences[byteArrayPreferencesKey(key)] = value
        }
    }

    suspend fun delete(key: String) {
        dataStore.edit { preferences ->
            preferences.remove(byteArrayPreferencesKey(key))
        }
    }

    suspend fun listKeys(): List<String> {
        return dataStore.data
            .map { preferences -> preferences.asMap().keys.map { it.name } }
            .first()
    }

    suspend fun exists(key: String): Boolean {
        return dataStore.data
            .map { preferences -> preferences.asMap().containsKey(byteArrayPreferencesKey(key)) }
            .first()
    }
}
