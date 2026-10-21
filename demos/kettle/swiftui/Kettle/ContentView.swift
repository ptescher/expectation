import SwiftUI

private extension Color {
    init(hex: UInt32) {
        self.init(red: Double(hex >> 16 & 0xff) / 255, green: Double(hex >> 8 & 0xff) / 255, blue: Double(hex & 0xff) / 255)
    }
    static let bg = Color(hex: 0x1a1b17), panel = Color(hex: 0x2b2d26), edge = Color(hex: 0x45483d)
    static let screen = Color(hex: 0x050806), text = Color(hex: 0xdfe8df), muted = Color(hex: 0x919c91)
    static let ok = Color(hex: 0x4cff7a), caution = Color(hex: 0xffb21f), warning = Color(hex: 0xff5a4f)
    static let faint = Color(hex: 0x3a403a), control = Color(hex: 0x161a16)
}

struct ContentView: View {
    @Bindable var kettle: Kettle

    private var state: (String, Color) {
        switch kettle.phase {
        case .off: ("OFF", .text)
        case .starting: ("STARTING", .caution)
        case .heating: ("HEATING", .ok)
        case .dry: ("DRY", .warning)
        }
    }

    private var lit: Bool {
        switch kettle.phase {
        case .heating, .starting: true
        default: false
        }
    }

    var body: some View {
        ZStack {
            Color.bg.ignoresSafeArea()
            VStack(spacing: 16) {
                VStack(alignment: .leading, spacing: 8) {
                    row("PRESET", presets[kettle.preset], .text)
                    row("STATE", state.0, state.1)
                    row("WATER", "\(Int(kettle.temperature.rounded())) °C", .text)
                    HStack(spacing: 12) {
                        Circle().fill(lit ? Color.warning : .faint).frame(width: 14, height: 14)
                        Text("LOCK").bold()
                            .foregroundStyle(kettle.lock ? Color.caution : .faint)
                            .padding(.horizontal, 6)
                            .overlay(RoundedRectangle(cornerRadius: 3).stroke(kettle.lock ? Color.caution : .edge))
                    }
                    .padding(.top, 6)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(16)
                .background(Color.screen, in: RoundedRectangle(cornerRadius: 8))

                VStack(spacing: 6) {
                    HStack {
                        Text("DIAL").foregroundStyle(Color.muted)
                        Spacer()
                        Text("\(Int(kettle.dial)) %").foregroundStyle(Color.text)
                    }
                    Slider(value: $kettle.dial, in: 0...100, step: 5).tint(.caution)
                }

                HStack(spacing: 8) {
                    button("START", kettle.start)
                    button("RESET", kettle.reset)
                    button("PRESET", kettle.pressPreset)
                    button("LOCK", kettle.pressLock)
                }
            }
            .padding(14)
            .background(Color.panel, in: RoundedRectangle(cornerRadius: 14))
            .padding(16)
            .font(.system(.body, design: .monospaced))
        }
    }

    private func row(_ label: String, _ value: String, _ color: Color) -> some View {
        HStack(spacing: 16) {
            Text(label).foregroundStyle(Color.muted).frame(width: 80, alignment: .leading)
            Text(value).bold().foregroundStyle(color)
        }
    }

    private func button(_ label: String, _ action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(label).bold().font(.system(size: 14, design: .monospaced))
                .frame(maxWidth: .infinity).padding(.vertical, 12)
                .foregroundStyle(Color.text)
                .background(Color.control, in: RoundedRectangle(cornerRadius: 6))
                .overlay(RoundedRectangle(cornerRadius: 6).stroke(Color.edge))
        }
    }
}
