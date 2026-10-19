plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "org.opendiag.transport"
    compileSdk = 34

    defaultConfig {
        minSdk = 24
        consumerProguardFiles("consumer-rules.pro")
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.9.0")
    // USB serial drivers (FTDI, CH34x, CP210x, PL2303, CDC-ACM). MIT licence.
    implementation("com.github.mik3y:usb-serial-for-android:3.8.1")
    implementation(project(":tauri-android"))
}
