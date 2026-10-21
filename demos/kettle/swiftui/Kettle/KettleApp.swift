import ExpectationDriver
import SwiftUI

@main
struct KettleApp: App {
    @State private var kettle = Kettle()
    @State private var driver: ExpectationDriver?

    var body: some Scene {
        WindowGroup {
            ContentView(kettle: kettle)
                .task { start() }
        }
    }

    /// The kettle's clock, and the embedded Expectation driver: when a runner is listening at
    /// EXPECTATION_URL, it drives this app.
    private func start() {
        let k = kettle
        Timer.scheduledTimer(withTimeInterval: 0.05, repeats: true) { _ in
            MainActor.assumeIsolated { k.tick(0.05) }
        }
        let driver = ExpectationDriver(platform: "SwiftUI", app: "Kettle")
            .observe("the kettle") { .name(k.phase.name) }
            .observe("the dial") { .number(k.dial) }
            .observe("the preset") { .name(presets[k.preset]) }
            .observe("the child lock") { .name(k.lock ? "on" : "off") }
            .set("the dial") { k.dial = $0.number ?? 0 }
            .arrange("the kettle") { k.force($0.string ?? "off") }
            .arrange("the preset") { v in k.preset = presets.firstIndex(of: v.string ?? "") ?? 0 }
            .arrange("the child lock") { k.lock = $0.string == "on" }
            .on("the user presses start") { k.start() }
            .on("the user presses reset") { k.reset() }
            .on("the user presses preset") { k.pressPreset() }
            .on("the user presses lock") { k.pressLock() }
            .on("the water runs out") { k.runOut() }
            .onReset { k.restore() }
        driver.start()
        self.driver = driver
    }
}
