package com.alasdair_cooper.watch_history

import android.graphics.drawable.Icon
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Logout
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Person
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.alasdair_cooper.watch_history.types.Event
import com.alasdair_cooper.watch_history.types.WatchedFilm
import com.alasdair_cooper.watch_history.ui.theme.AppTheme
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            AppTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background,
                ) {
                    View()
                }
            }
        }
    }
}

class MainCore : Core() {
    init {
        viewModelScope.launch { update(Event.InitialLoad()) }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun View(core: MainCore = viewModel()) {
    val scrollBehavior = TopAppBarDefaults.enterAlwaysScrollBehavior(rememberTopAppBarState())
    var expanded by remember { mutableStateOf(false) }

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
                        Icon(
                            Icons.Filled.Person,
                            contentDescription = null
                        )
                        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
                            DropdownMenuItem(
                                text = { Text("alasdair-cooper") },
                                leadingIcon = { Icon(Github, contentDescription = null) },
                                onClick = {}
                            )
                            DropdownMenuItem(
                                text = { Text("Logout") },
                                trailingIcon = { Icon(Icons.AutoMirrored.Default.Logout, contentDescription = null) },
                                onClick = {}
                            )
                        }
                    }
                }
            }, scrollBehavior = scrollBehavior)
        }) { innerPadding ->
        Content(innerPadding, core.view?.films.orEmpty())
    }
}


@Composable
fun Content(innerPadding: PaddingValues, films: List<WatchedFilm>) {
    LazyColumn(
        contentPadding = innerPadding,
        modifier = Modifier
            .fillMaxSize()
    ) {
        items(films) { film ->
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
    AppTheme { View() }
}
