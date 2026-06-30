// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "FieldInspectorFissionHost",
    platforms: [
        .iOS(.v16),
    ],
    products: [
        .library(name: "FissionHost", targets: ["FissionHost"]),
    ],
    dependencies: [
        .package(path: "NativeModules"),
    ],
    targets: [
        .target(
            name: "FissionHost",
            dependencies: [
                .product(name: "FissionNativeModules", package: "NativeModules"),
            ],
            path: "Sources/FissionHost"
        ),
    ]
)
