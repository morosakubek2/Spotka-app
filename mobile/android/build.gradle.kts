// mobile/android/build.gradle.kts
// Top-level build file where you can add configuration options common to all sub-projects/modules.

plugins {
    id("com.android.application") version "8.2.2" apply false
    id("com.android.library") version "8.2.2" apply false
    id("org.jetbrains.kotlin.android") version "1.9.22" apply false
    // KSP for annotation processing (if needed in future)
    id("com.google.devtools.ksp") version "1.9.22-1.0.17" apply false
}

// Define versions in one place for consistency
ext {
    set("androidMinSdk", 26)
    set("androidTargetSdk", 34)
    set("androidCompileSdk", 34)
    set("javaVersion", JavaVersion.VERSION_17)
}

tasks.register("clean", Delete::class) {
    delete(rootProject.layout.buildDirectory)
}

// Ensure Gradle JVM matches the required version (17)
// This helps prevent issues with newer Android Gradle Plugin versions
gradle.projectsLoaded {
    println("Gradle JVM: ${JavaVersion.current()}")
    if (JavaVersion.current() != JavaVersion.VERSION_17) {
        println("WARNING: Recommended Java version is 17. Current is ${JavaVersion.current()}")
    }
}
