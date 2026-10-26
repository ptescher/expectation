// The Expectation driver for Swift apps: bind the vocabulary to the app, and the Expectation
// runner can drive it. See https://expectation.sim001.net/guide/remote.html
//
//     let driver = ExpectationDriver(platform: "iOS", app: "Kettle")
//         .observe("the kettle") { model.phase.name }
//         .set("the dial") { model.dial = $0.number ?? 0 }
//         .on("the user presses lock") { model.pressLock() }
//     driver.start()   // only connects when EXPECTATION_URL is set
//
// Handlers run on the main actor. The driver says hello to the runner, long-polls it for
// commands and reconnects when it goes away, so it can stay in a debug build.

import Foundation
#if canImport(UIKit)
import UIKit
#endif

/// Something a person can act on: a button, a field, a switch.
public struct ExpectationControl: Sendable, Equatable {
    /// The screen or dialog it is in; the same id in two views is two controls.
    public var view: String
    /// Stable, and independent of display copy (an accessibility identifier).
    public var id: String
    public var kind: String
    public var label: String
    /// The vocabulary event using it makes happen, e.g. "the user presses lock".
    public var event: String?

    public init(view: String, id: String, kind: String = "", label: String = "", event: String? = nil) {
        self.view = view; self.id = id; self.kind = kind; self.label = label; self.event = event
    }

    var json: [String: Any] {
        var o: [String: Any] = ["view": view, "id": id, "kind": kind, "label": label]
        if let event { o["event"] = event }
        return o
    }
}

/// A state's value: a name ("heating") or a number (80).
public enum ExpectationValue: Sendable, Equatable, ExpressibleByStringLiteral, ExpressibleByFloatLiteral, ExpressibleByIntegerLiteral {
    case name(String)
    case number(Double)

    public init(stringLiteral value: String) { self = .name(value) }
    public init(floatLiteral value: Double) { self = .number(value) }
    public init(integerLiteral value: Int) { self = .number(Double(value)) }

    public var string: String? { if case .name(let s) = self { s } else { nil } }
    public var number: Double? { if case .number(let n) = self { n } else { nil } }

    var json: Any {
        switch self {
        case .name(let s): s
        case .number(let n): n
        }
    }
}

@MainActor
public final class ExpectationDriver {
    public typealias Observer = @MainActor () -> ExpectationValue
    public typealias Setter = @MainActor (ExpectationValue) -> Void
    public typealias Action = @MainActor () -> Void
    /// PNG bytes.
    public typealias Shooter = @MainActor () -> Data?
    public typealias Controls = @MainActor () -> [ExpectationControl]
    /// The `(view, id)` of the controls used since the last call; empty it as you return.
    public typealias Used = @MainActor () -> [(view: String, id: String)]

    private let platform: String
    private let app: String
    private var observers: [(String, Observer)] = []
    private var setters: [(String, Setter)] = []
    private var arrangers: [(String, Setter)] = []
    private var events: [(String, Action)] = []
    private var reset: Action?
    private var shooter: Shooter?
    private var controlsList: Controls?
    private var usedList: Used?
    private var task: Task<Void, Never>?

    public init(platform: String, app: String) {
        self.platform = platform
        self.app = app
        #if canImport(UIKit)
        shooter = ExpectationDriver.keyWindowPNG
        #endif
    }

    @discardableResult public func observe(_ subject: String, _ f: @escaping Observer) -> Self {
        observers.append((subject, f)); return self
    }

    @discardableResult public func set(_ subject: String, _ f: @escaping Setter) -> Self {
        setters.append((subject, f)); return self
    }

    @discardableResult public func arrange(_ subject: String, _ f: @escaping Setter) -> Self {
        arrangers.append((subject, f)); return self
    }

    @discardableResult public func on(_ event: String, _ f: @escaping Action) -> Self {
        events.append((event, f)); return self
    }

    @discardableResult public func onReset(_ f: @escaping Action) -> Self {
        reset = f; return self
    }

    /// Lists what a person can act on now, so the report can show which controls no test used.
    @discardableResult public func controls(_ f: @escaping Controls) -> Self {
        controlsList = f; return self
    }

    /// The controls `set` and `perform` acted on since the last call; empty it as you return.
    @discardableResult public func used(_ f: @escaping Used) -> Self {
        usedList = f; return self
    }

    /// Replaces the default screenshot (the key window, on UIKit platforms). `nil` takes none,
    /// and the runner can take its own from outside.
    @discardableResult public func screenshot(_ f: Shooter?) -> Self {
        shooter = f; return self
    }

    /// Connects to the runner at `url`, or at `EXPECTATION_URL`. Without either, does nothing.
    public func start(url: URL? = nil) {
        let env = ProcessInfo.processInfo.environment["EXPECTATION_URL"].flatMap(URL.init(string:))
        guard let base = url ?? env, task == nil else { return }
        task = Task { await self.run(base) }
    }

    public func stop() {
        task?.cancel()
        task = nil
    }

    private var hello: [String: Any] {
        [
            "platform": platform, "app": app,
            "observe": observers.map(\.0), "set": setters.map(\.0), "arrange": arrangers.map(\.0),
            "events": events.map(\.0), "screenshot": shooter != nil, "reset": reset != nil,
            "controls": controlsList != nil,
        ]
    }

    private func run(_ base: URL) async {
        let session = URLSession(configuration: .ephemeral)
        while !Task.isCancelled {
            guard let (status, body) = try? await post(session, base, "hello", hello),
                  status == 200, let id = (body?["session"] as? NSNumber)?.intValue
            else {
                try? await Task.sleep(for: .seconds(1))
                continue
            }
            while !Task.isCancelled {
                var request = URLRequest(url: endpoint(base, "next?session=\(id)"))
                request.timeoutInterval = 30
                guard let (data, response) = try? await session.data(for: request),
                      let status = (response as? HTTPURLResponse)?.statusCode
                else { break }
                if status == 204 { continue }
                guard status == 200, let command = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { break }
                var reply = handle(command)
                reply["id"] = command["id"]
                guard let (posted, _) = try? await post(session, base, "reply?session=\(id)", reply), posted == 204 else { break }
            }
        }
    }

    private func post(_ session: URLSession, _ base: URL, _ path: String, _ body: [String: Any]) async throws -> (Int, [String: Any]?) {
        var request = URLRequest(url: endpoint(base, path))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONSerialization.data(withJSONObject: body)
        request.timeoutInterval = 10
        let (data, response) = try await session.data(for: request)
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        return (status, try? JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    private func endpoint(_ base: URL, _ path: String) -> URL {
        let root = base.absoluteString.hasSuffix("/") ? String(base.absoluteString.dropLast()) : base.absoluteString
        return URL(string: "\(root)/\(path)")!
    }

    private func value(_ any: Any?) -> ExpectationValue? {
        if let s = any as? String { return .name(s) }
        if let n = any as? NSNumber { return .number(n.doubleValue) }
        return nil
    }

    private func handle(_ c: [String: Any]) -> [String: Any] {
        func find<T>(_ list: [(String, T)], _ name: Any?) -> T? {
            list.first { $0.0 == name as? String }?.1
        }
        switch c["op"] as? String {
        case "reset":
            reset?()
        case "arrange":
            guard let v = value(c["value"]), let f = find(arrangers, c["subject"]) ?? find(setters, c["subject"]) else {
                return ["error": "can't arrange \(c["subject"] ?? "?")"]
            }
            f(v)
        case "set":
            guard let v = value(c["value"]), let f = find(setters, c["subject"]) else { return ["error": "can't set \(c["subject"] ?? "?")"] }
            f(v)
        case "perform":
            guard let f = find(events, c["event"]) else { return ["error": "can't make \(c["event"] ?? "?") happen"] }
            f()
        case "observe":
            guard let f = find(observers, c["subject"]) else { return ["error": "can't observe \(c["subject"] ?? "?")"] }
            return ["value": f().json]
        case "screenshot":
            guard let png = shooter?() else { return ["error": "no screenshot"] }
            return ["png": png.base64EncodedString()]
        case "controls":
            guard let f = controlsList else { return ["error": "no controls"] }
            return ["controls": f().map(\.json)]
        case "used":
            guard let f = usedList else { return ["error": "no used"] }
            return ["used": f().map { ["view": $0.view, "id": $0.id] }]
        default:
            return ["error": "unknown command \(c["op"] ?? "?")"]
        }
        return [:]
    }

    #if canImport(UIKit)
    /// The key window, as the user sees it.
    public static func keyWindowPNG() -> Data? {
        let windows = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.flatMap(\.windows)
        guard let window = windows.first(where: \.isKeyWindow) ?? windows.first else { return nil }
        let renderer = UIGraphicsImageRenderer(bounds: window.bounds)
        let image = renderer.image { _ in window.drawHierarchy(in: window.bounds, afterScreenUpdates: true) }
        return image.pngData()
    }
    #endif
}
