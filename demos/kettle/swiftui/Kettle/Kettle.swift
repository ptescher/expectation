// The kettle itself, the same on every platform: the dial has to stay above 55 % for 0.6 s
// before it starts, starting takes 0.22 s, and while the child lock is on the dial and the
// buttons (all but LOCK and RESET) do nothing.

import Foundation
import Observation

let presets = ["WARM", "HOT", "BOIL"]
private let targets = [70.0, 85.0, 100.0]

enum Phase: Equatable {
    case off
    case starting(Double)
    case heating
    case dry

    var name: String {
        switch self {
        case .off: "off"
        case .starting: "starting"
        case .heating: "heating"
        case .dry: "dry"
        }
    }
}

@MainActor @Observable
final class Kettle {
    var phase = Phase.off
    var dial = 0.0
    var preset = 0
    var lock = false
    var temperature = 20.0
    private var hold = 0.0

    func tick(_ dt: Double) {
        let dial = lock ? 0 : self.dial
        switch phase {
        case .off where dial > 55:
            hold += dt
            if hold > 0.6 {
                phase = .starting(0)
                hold = 0
            }
        case .off:
            hold = 0
        case .starting(let t):
            phase = t + dt >= 0.22 ? .heating : .starting(t + dt)
        case .heating:
            temperature = min(temperature + dt * 8, targets[preset])
        case .dry:
            break
        }
    }

    func start() {
        if !lock && phase == .off { phase = .starting(0) }
    }

    func reset() {
        phase = .off
        hold = 0
        temperature = 20
    }

    func pressPreset() {
        if !lock { preset = (preset + 1) % presets.count }
    }

    func pressLock() { lock.toggle() }

    func runOut() {
        switch phase {
        case .heating, .starting: phase = .dry
        default: break
        }
    }

    /// For tests: put the kettle in a phase directly.
    func force(_ name: String) {
        switch name {
        case "starting": phase = .starting(0)
        case "heating": phase = .heating; temperature = 60
        case "dry": phase = .dry
        default: reset()
        }
    }

    func restore() {
        reset()
        dial = 0
        preset = 0
        lock = false
    }
}
