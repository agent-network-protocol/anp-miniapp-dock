// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "AnpMiniappDockMac",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(name: "AnpMiniappDockMac", targets: ["AnpMiniappDockMac"])
    ],
    targets: [
        .executableTarget(name: "AnpMiniappDockMac")
    ]
)
