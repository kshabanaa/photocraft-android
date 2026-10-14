// Phase 0 spike: packages libphotocraft_android.so (built by cargo-ndk into src/main/jniLibs)
// with AndroidX GameActivity. No Kotlin or Java sources: the manifest names GameActivity itself.
plugins {
    id("com.android.application")
}

android {
    namespace = "local.photocraft.spike"
    compileSdk = 36
    // The NDK cargo-ndk builds with; AGP needs it to strip the .so and extract debug symbols.
    ndkVersion = "29.0.14206865"
    defaultConfig {
        applicationId = "local.photocraft.spike"
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "0.0.1-spike"
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }
    buildTypes {
        getByName("release") { isMinifyEnabled = false }
    }
    packaging { jniLibs { useLegacyPackaging = false } }
}

dependencies {
    // Must match the GameActivity C glue that android-activity 0.6.1 bundles (GameActivity.h: 4.4.0).
    implementation("androidx.games:games-activity:4.4.0")
    implementation("androidx.appcompat:appcompat:1.8.0")
}
