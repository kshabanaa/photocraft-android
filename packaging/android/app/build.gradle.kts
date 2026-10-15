// Packages libphotocraft_android.so (built by cargo-ndk into src/main/jniLibs) with AndroidX
// GameActivity and PhotoCraftActivity (src/main/java).
plugins {
    id("com.android.application")
}

// `-Pphotocraft.download=true` (packaging/android/release-apk.sh): APKs to download from a GitHub
// release. One APK per ABI, so a phone downloads only its own library, with the native library
// compressed (about half the size of the stored, page-aligned library Play wants; Android extracts
// it on install).
val download = providers.gradleProperty("photocraft.download").map { it.toBoolean() }.getOrElse(false)

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
        if (!download) {
            ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
        }
    }
    buildTypes {
        // Test releases are signed with the development key until the release key exists (owner
        // task H-12): installable side by side with nothing else, upgradable only by builds signed
        // the same way.
        getByName("release") {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("debug")
        }
    }
    if (download) {
        splits {
            abi {
                isEnable = true
                reset()
                include("arm64-v8a", "x86_64")
                isUniversalApk = false
            }
        }
    }
    packaging { jniLibs { useLegacyPackaging = download } }
}

dependencies {
    // Must match the GameActivity C glue that android-activity 0.6.1 bundles (GameActivity.h: 4.4.0).
    implementation("androidx.games:games-activity:4.4.0")
    implementation("androidx.appcompat:appcompat:1.8.0")
}
