// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "ExpectationDriver",
    platforms: [.iOS(.v16), .macOS(.v13), .tvOS(.v16), .visionOS(.v1)],
    products: [.library(name: "ExpectationDriver", targets: ["ExpectationDriver"])],
    targets: [.target(name: "ExpectationDriver")]
)
