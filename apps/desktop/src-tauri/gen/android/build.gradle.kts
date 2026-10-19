buildscript {
    repositories {
        google()
        mavenCentral()
    }
    dependencies {
        classpath("com.android.tools.build:gradle:8.5.1")
        classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:1.9.25")
    }
}

allprojects {
    repositories {
        google()
        mavenCentral()
        // usb-serial-for-android (used by the OpenDiag transport plugin) is published on JitPack.
        maven("https://jitpack.io")
    }
}

tasks.register("clean").configure {
    delete("build")
}

