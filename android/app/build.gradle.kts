plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

// Test mode is a build-time switch: -PtestMode=true. Never set for release (spike b2t-29).
val testMode = (project.findProperty("testMode") as String?) == "true"

android {
    namespace = "app.cruxvid.baseline"
    compileSdk = 36

    defaultConfig {
        applicationId = "app.cruxvid.baseline"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.0.1"
        buildConfigField("boolean", "TEST_MODE", testMode.toString())
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    splits {
        abi {
            isEnable = true
            reset()
            include("arm64-v8a", "x86")
            isUniversalApk = false
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"))
        }
    }
}

dependencies {
    implementation(platform("androidx.compose:compose-bom:2025.09.00"))
    implementation("androidx.activity:activity-compose:1.11.0")
    implementation("androidx.compose.material3:material3")
}
