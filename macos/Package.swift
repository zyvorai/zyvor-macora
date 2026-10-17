// swift-tools-version: 5.9
import PackageDescription
let package = Package(
    name: "Macora",
    platforms: [.macOS(.v14)],
    products: [.executable(name: "Macora", targets: ["MacoraApp"]), .library(name: "MacoraCore", targets: ["MacoraCore"])],
    targets: [
        .target(name: "MacoraCore"),
        .executableTarget(name: "MacoraApp", dependencies: ["MacoraCore"]),
        .testTarget(name: "MacoraCoreTests", dependencies: ["MacoraCore"])
    ]
)
