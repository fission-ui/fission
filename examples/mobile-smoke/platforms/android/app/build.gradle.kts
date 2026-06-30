plugins {
    id("com.android.application")
}

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
}

apply(from = "../native-modules.gradle")
