package com.alasdair_cooper.watch_history

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.browser.auth.AuthTabIntent
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Logout
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Person
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.viewmodel.compose.viewModel
import coil.compose.AsyncImage
import coil.request.ImageRequest
import com.alasdair_cooper.watch_history.types.Event
import com.alasdair_cooper.watch_history.ui.theme.AppTheme
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlin.jvm.optionals.getOrDefault
import kotlin.jvm.optionals.getOrElse
import kotlin.jvm.optionals.getOrNull

@AndroidEntryPoint
class MainActivity : ComponentActivity() {
    val core: Core by viewModels()

    val authTabLauncher = AuthTabIntent.registerActivityResultLauncher(this) { authResult ->
        handleAuthResult(authResult) { resultUri ->
            lifecycleScope.launch {
                core.update(Event.CallbackReceived(resultUri.toString()))
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        handleDeepLink(intent)
        enableEdgeToEdge()
        setContent {
            AppTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background,
                ) {
                    View(core, this::openUrl)
                }
            }
        }

        lifecycleScope.launch { core.update(Event.InitialLoad()) }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleDeepLink(intent)
    }

    private fun handleDeepLink(intent: Intent) {
        val action: String? = intent.action
        val data: Uri? = intent.data

        if (action == Intent.ACTION_VIEW && data != null) {
            runBlocking {
                core.update(Event.CallbackReceived(data.toString()))
            }
        }
    }

    private fun handleAuthResult(authResult: AuthTabIntent.AuthResult, onSuccess: (Uri) -> Unit) {
        when (authResult.resultCode) {
            AuthTabIntent.RESULT_OK -> {
                onSuccess(authResult.resultUri!!)
            }

            AuthTabIntent.RESULT_CANCELED -> {
                // Handle cancellation if needed
            }
        }
    }

    fun openUrl(url: Uri) {
        val authTabIntent = AuthTabIntent.Builder().build()
        authTabIntent.launch(authTabLauncher, url, "www.alasdaircooper.net", "/watch-history/github-callback")
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun View(core: Core, openUrl: (Uri) -> Unit) {
    val coroutineScope = rememberCoroutineScope()
    val scrollBehavior = TopAppBarDefaults.enterAlwaysScrollBehavior(rememberTopAppBarState())
    var expanded by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        core.shellEvents.collect { event ->
            when (event) {
                is Core.ShellEvent.OpenUrl -> {
                    openUrl(event.url)
                }

                is Core.ShellEvent.CallbackReceived -> {
                    core.update(Event.CallbackReceived(event.url.toString()))
                }
            }
        }
    }

    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        floatingActionButton = {
            FloatingActionButton(onClick = {}) {
                Icon(Icons.Filled.Add, contentDescription = null)
            }
        },
        topBar = {
            CenterAlignedTopAppBar(title = { Text("Watch History") }, actions = {
                Box {
                    IconButton(onClick = { expanded = !expanded }) {
                        val avatarUrl = core.view?.user_info?.getOrNull()?.avatar_url
                        if (avatarUrl != null) {
                            AsyncImage(
                                model = ImageRequest.Builder(LocalContext.current)
                                    .data(avatarUrl)
                                    .crossfade(true)
                                    .build(),
                                contentDescription = null,
                                contentScale = ContentScale.Crop,
                                modifier = Modifier
                                    .size(48.dp)
                                    .clip(CircleShape)
                            )
                        } else {
                            Icon(
                                Icons.Filled.Person,
                                contentDescription = null
                            )
                        }
                        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
                            val userInfo = core.view?.user_info?.getOrNull();
                            if (userInfo != null) {
                                DropdownMenuItem(
                                    text = { Text(userInfo.name) },
                                    leadingIcon = { Icon(Github, contentDescription = null) },
                                    onClick = {

                                    }
                                )
                                DropdownMenuItem(
                                    text = { Text("Logout") },
                                    trailingIcon = {
                                        Icon(
                                            Icons.AutoMirrored.Default.Logout,
                                            contentDescription = null
                                        )
                                    },
                                    onClick = {
                                        coroutineScope.launch { core.update(Event.LogoutButtonClicked()) }
                                        expanded = false
                                    }
                                )
                            } else {
                                DropdownMenuItem(
                                    text = { Text("Login with GitHub") },
                                    leadingIcon = { Icon(Github, contentDescription = null) },
                                    onClick = {
                                        coroutineScope.launch { core.update(Event.LoginButtonClicked()) }
                                        expanded = false
                                    }
                                )
                            }
                        }
                    }
                }
            }, scrollBehavior = scrollBehavior)
        }) { innerPadding ->
        Content(innerPadding)
    }
}


@Composable
fun Content(innerPadding: PaddingValues, core: Core = viewModel()) {
    LazyColumn(
        contentPadding = innerPadding,
        modifier = Modifier
            .fillMaxSize()
    ) {
        items(core.view?.films.orEmpty()) { film ->
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(16.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Text(
                    text = film.title,
                    fontSize = 20.sp
                )
                Text(
                    text = film.rating::class.simpleName ?: "",
                    fontSize = 16.sp
                )
            }
        }
    }
}

@Preview(showBackground = true)
@Composable
fun DefaultPreview() {
    AppTheme { View(viewModel()) {} }
}
