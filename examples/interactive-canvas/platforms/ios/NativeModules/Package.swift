// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "NativeModules",
    platforms: [
        .iOS(.v16),
    ],
    products: [
        .library(name: "FissionNativeModules", targets: ["FissionNativeModules"]),
    ],
    dependencies: [],
    targets: [
        .target(
            name: "FissionNativeModules",
            dependencies: [],
            path: "Sources/FissionNativeModules"
        ),
    ]
)
