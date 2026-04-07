// mobile/android/app/build.gradle.kts
plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    // Plugin do zarządzania zależnościami (opcjonalne, ale zalecane)
    id("com.google.devtools.ksp") version "1.9.0-1.0.13" apply false
}

android {
    namespace = "com.spotka"
    compileSdk = 34 // Android 14

    defaultConfig {
        applicationId = "com.spotka"
        minSdk = 26 // Android 8.0 (wymagane dla nowoczesnych funkcji BLE/Security)
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0-alpha"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        // Konfiguracja NDk i Architektury
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }

        // Wersjonowanie synchronizowane z Cargo.toml (opcjonalne skryptem)
        buildConfigField("String", "RUST_VERSION", "\"0.1.0-alpha\"")
    }

    // --- Integracja z Rustem (cargo-ndk) ---
    // Automatyczne budowanie bibliotek .so przed kompilacją Javy/Kotlin
    val rustArchMap = mapOf(
        "arm64-v8a" to "aarch64-linux-android",
        "armeabi-v7a" to "armv7-linux-androideabi",
        "x86_64" to "x86_64-linux-android"
    )

    androidComponents.onVariants { variant ->
        val taskName = "buildRust${variant.name.capitalize()}"
        val outputDir = file("src/main/jniLibs")
        
        // Zadanie Gradle wywołujące cargo ndk
        val buildRustTask = tasks.register<Exec>(taskName) {
            group = "building"
            description = "Builds Rust core for ${variant.name}"
            
            workingDir("../../rust-core")
            
            // Budowanie dla wszystkich wymaganych architektur
            val targets = rustArchMap.keys.joinToString(" ") { "-t $it" }
            commandLine(
                "cargo", "ndk",
                "-o", outputDir.absolutePath,
                "-t", "arm64-v8a", "-t", "armeabi-v7a", "-t", "x86_64",
                "build",
                "--lib",
                if (variant.buildType == "release") "--release" else ""
            )
            
            // Zależność od czystości (opcjonalnie, aby wymusić rebuild przy zmianach)
            // dependsOn("cleanRust") 
        }

        // Podpięcie zadania Rust pod preBuild
        tasks.named("pre${variant.name.capitalize()}Build").configure {
            dependsOn(buildRustTask)
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
            // Signing config would go here for production
        }
        debug {
            isMinifyEnabled = false
            isDebuggable = true
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        viewBinding = true
        buildConfig = true
    }
}

dependencies {
    // Android Core
    implementation("androidx.core:core-ktx:1.12.0")
    implementation("androidx.appcompat:appcompat:1.6.1")
    implementation("com.google.android.material:material:1.11.0")
    implementation("androidx.constraintlayout:constraintlayout:2.1.4")
    
    // Lifecycle & Coroutines
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.7.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.7.3")
    
    // Slint Android Backend (Jeśli używamy oficjalnego backendu)
    // implementation("com.github.slint-ui:slint-android:1.6.0") // Przykładowa zależność
    
    // Testing
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.5")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.1")
}
