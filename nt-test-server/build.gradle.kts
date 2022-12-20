plugins {
    id("java")
}

group = "com.team303"
version = "1.0-SNAPSHOT"

repositories {
    mavenCentral()

    maven(url = "https://frcmaven.wpi.edu/artifactory/release/")
}

dependencies {
    // Add ntcore-java
    implementation ("edu.wpi.first.ntcore:ntcore-java:2023.1.1-beta-7")
    // Add ntcore-jni for runtime.
    implementation ("edu.wpi.first.ntcore:ntcore-jni:2023.1.1-beta-7:windowsx86-64")

    // Add wpiutil-java
    implementation ("edu.wpi.first.wpiutil:wpiutil-java:2023.1.1-beta-7")
    // Add wpiutil-jni
    implementation ("edu.wpi.first.wpiutil:wpiutil-jni:2023.1.1-beta-7:windowsx86-64")

}