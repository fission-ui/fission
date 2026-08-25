plugins {
    id("com.android.application")
}

val releaseKeystore = System.getenv("ANDROID_KEYSTORE")
val releaseStorePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
val releaseKeyAlias = System.getenv("ANDROID_KEYSTORE_ALIAS") ?: "upload"
val releaseKeyPassword = System.getenv("ANDROID_KEY_PASSWORD") ?: releaseStorePassword
val hasReleaseSigning = !releaseKeystore.isNullOrBlank() &&
    !releaseStorePassword.isNullOrBlank() &&
    !releaseKeyAlias.isNullOrBlank() &&
    !releaseKeyPassword.isNullOrBlank()

android {
    namespace = "ai.worka.fission.mobile.smoke"
    compileSdk = (System.getenv("ANDROID_TARGET_API_LEVEL") ?: "35").toInt()

    defaultConfig {
        applicationId = "ai.worka.fission.mobile.smoke"
        minSdk = (System.getenv("ANDROID_MIN_API_LEVEL") ?: "24").toInt()
        targetSdk = (System.getenv("ANDROID_TARGET_API_LEVEL") ?: "35").toInt()
        versionCode = 1
        versionName = "0.1.0"
    }

    sourceSets {
        getByName("main") {
            manifest.srcFile("../AndroidManifest.xml")
            java.srcDirs("../java")
            res.srcDirs("../res", "src/main/res")
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }

    signingConfigs {
        create("release") {
            if (hasReleaseSigning) {
                storeFile = file(releaseKeystore!!)
                storePassword = releaseStorePassword
                keyAlias = releaseKeyAlias
                keyPassword = releaseKeyPassword
            }
        }
    }

    buildTypes {
        getByName("debug") {
            isDebuggable = true
        }
        getByName("release") {
            isDebuggable = false
            if (hasReleaseSigning) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
    }
}

dependencies {
    implementation("androidx.games:games-activity:4.4.0")
}

apply(from = "../native-modules.gradle")
