package app.cruxvid.baseline

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            var result by remember { mutableStateOf("idle") }
            Column(Modifier.padding(24.dp)) {
                Text("Cruxvid baseline")
                Text("gemini=${AppConfig.geminiBaseUrl}")
                Text("resolver=${AppConfig.sourceResolver}")
                Button(onClick = { result = "ok:${AppConfig.sourceResolver}" }) { Text("Test") }
                Text("result=$result")
            }
        }
    }
}
