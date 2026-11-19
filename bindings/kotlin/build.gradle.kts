import org.gradle.api.tasks.testing.logging.TestExceptionFormat
plugins {
    kotlin("jvm") version "2.4.21"
    `maven-publish`
}
group = "jp.genshi"
version = "0.1.0-alpha.1"
repositories { mavenCentral() }
kotlin { jvmToolchain(21) }
dependencies {
    api("net.java.dev.jna:jna:5.18.1")
    testImplementation(kotlin("test"))
    testImplementation("org.junit.jupiter:junit-jupiter:5.13.4")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}
val repoRoot = projectDir.resolve("../..").canonicalFile
val nativeDir = providers.gradleProperty("clins.nativeDir").orElse(repoRoot.resolve("target/debug").path)
val buildNative = tasks.register<Exec>("buildNative") {
    workingDir(repoRoot)
    commandLine("cargo", "build", "--locked", "-p", "clins-kt")
    inputs.files(fileTree(repoRoot.resolve("crates/core")) { include("**/*.rs", "**/*.toml", "**/*.gz") })
    inputs.files("Cargo.toml", "src/lib.rs", "src/bin/bindgen.rs", repoRoot.resolve("Cargo.toml"), repoRoot.resolve("Cargo.lock"))
    outputs.file(repoRoot.resolve("target/debug/" + System.mapLibraryName("clins_kt")))
}
val generateBindings = tasks.register<Exec>("generateBindings") {
    dependsOn(buildNative)
    workingDir(repoRoot)
    commandLine("cargo", "run", "--locked", "-p", "clins-kt", "--bin", "clins-bindgen", "--", "generate", "--library", nativeDir.map { "$it/" + System.mapLibraryName("clins_kt") }.get(), "--language", "kotlin", "--config", "bindings/kotlin/uniffi.toml", "--out-dir", "bindings/kotlin/build/generated/uniffi", "--no-format")
    inputs.file(nativeDir.map { "$it/" + System.mapLibraryName("clins_kt") })
    inputs.file("uniffi.toml")
    outputs.dir(layout.buildDirectory.dir("generated/uniffi"))
}
sourceSets.main { kotlin.srcDir(layout.buildDirectory.dir("generated/uniffi")) }
tasks.named("compileKotlin") { dependsOn(generateBindings) }
tasks.test {
    useJUnitPlatform()
    systemProperty("clins.repoRoot", repoRoot.path)
    testLogging { exceptionFormat = TestExceptionFormat.FULL }
}
java { withSourcesJar() }
tasks.named("sourcesJar") { dependsOn(generateBindings) }
publishing {
    publications { create<MavenPublication>("clins") { from(components["java"]) } }
    repositories { maven { name = "localBuild"; url = uri(layout.buildDirectory.dir("repository")) } }
}

val nativePlatform = run {
    val os = System.getProperty("os.name").lowercase()
    val arch = System.getProperty("os.arch").lowercase()
    val canonicalArch = when (arch) { "amd64", "x86_64" -> "x86-64"; "arm64" -> "aarch64"; else -> arch }
    val canonicalOs = when { os.contains("mac") -> "darwin"; os.contains("win") -> "win32"; else -> "linux" }
    "$canonicalOs-$canonicalArch"
}
val nativeJar = tasks.register<Jar>("nativeJar") {
    dependsOn(buildNative)
    archiveClassifier.set(nativePlatform)
    from(nativeDir.map { "$it/" + System.mapLibraryName("clins_kt") }) { into(nativePlatform) }
    from(listOf("LICENSE", "THIRD_PARTY_NOTICES.md", "THIRD_PARTY_LICENSES.txt")) { into("META-INF") }
    from("LICENSES") { into("META-INF/licenses") }
}
publishing.publications.named<MavenPublication>("clins") { artifact(nativeJar) }
tasks.jar {
    from(listOf("LICENSE", "THIRD_PARTY_NOTICES.md", "THIRD_PARTY_LICENSES.txt")) { into("META-INF") }
    from("LICENSES") { into("META-INF/licenses") }
}

dependencies { testRuntimeOnly(files(nativeJar)) }
