plugins {
    id("java")
    id("org.jetbrains.kotlin.jvm") version "1.9.22"
    id("application")
}

group = "com.mofa"
version = "0.1.0"

repositories {
    mavenCentral()
}

dependencies {
    // Kotlin 运行时（UniFFI 生成的代码需要）
    implementation("org.jetbrains.kotlin:kotlin-stdlib:1.9.22")

    // JNA（Java Native Access，用于加载原生库）
    implementation("net.java.dev.jna:jna:5.14.0")

    // 测试依赖
    testImplementation("org.junit.jupiter:junit-jupiter-api:5.10.1")
    testRuntimeOnly("org.junit.jupiter:junit-jupiter-engine:5.10.1")
}

application {
    mainClass.set("com.mofa.Example")
}

java {
    sourceCompatibility = JavaVersion.VERSION_11
    targetCompatibility = JavaVersion.VERSION_11
}

kotlin {
    jvmToolchain(11)
}

tasks.test {
    useJUnitPlatform()
}

// 配置原生库路径
tasks.withType<JavaExec> {
    systemProperty("java.library.path", "${projectDir}/libs")
}

// 配置测试任务的原生库路径
tasks.test {
    systemProperty("java.library.path", "${projectDir}/libs")
}

// 打印库路径信息
tasks.register("showLibPath") {
    doLast {
        println("Native library path: ${projectDir}/libs")
        println("Expected library file: ${projectDir}/libs/libaimos.dylib (macOS) or libaimos.so (Linux)")
    }
}
