package com.alasdair_cooper.watch_history

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.alasdair_cooper.watch_history.shared.handleResponse
import com.alasdair_cooper.watch_history.shared.processEvent
import com.alasdair_cooper.watch_history.shared.view
import com.alasdair_cooper.watch_history.types.*
import com.alasdair_cooper.watch_history.types.HttpRequest
import com.novi.serde.Bytes
import io.ktor.client.*
import io.ktor.client.call.*
import io.ktor.client.engine.cio.*
import io.ktor.client.request.*
import io.ktor.http.*
import io.ktor.util.*
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow

open class Core : androidx.lifecycle.ViewModel() {
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
