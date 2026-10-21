//! The kettle in GPUI. The Expectation driver is embedded: when a runner is listening at
//! `EXPECTATION_URL` (http://127.0.0.1:4477 by default), it drives this window.

mod kettle;

use expectation_remote::client::{Client, Handlers, Reply};
use gpui::{
    App, Application, Bounds, Context, MouseButton, MouseDownEvent, MouseMoveEvent, Rgba, Window, WindowBounds,
    WindowKind, WindowOptions, div, prelude::*, px, rgb, size,
};
use kettle::{Kettle, PRESETS, Phase};
use std::time::Duration;

const TICK: Duration = Duration::from_millis(50);

struct KettleView {
    kettle: Kettle,
    /// The window's CGWindowID, for screenshots.
    window_id: Option<i64>,
}

fn handlers() -> Handlers<KettleView> {
    Handlers::new()
        .observe("the kettle", |v: &KettleView| v.kettle.phase_name())
        .observe("the dial", |v: &KettleView| v.kettle.dial)
        .observe("the preset", |v: &KettleView| PRESETS[v.kettle.preset])
        .observe("the child lock", |v: &KettleView| if v.kettle.lock { "on" } else { "off" })
        .set("the dial", |v, x| v.kettle.dial = x.as_f64().unwrap_or(0.0))
        .arrange("the kettle", |v, x| v.kettle.force(x.as_str().unwrap_or("off")))
        .arrange("the preset", |v, x| {
            v.kettle.preset = PRESETS.iter().position(|p| Some(*p) == x.as_str()).unwrap_or(0)
        })
        .arrange("the child lock", |v, x| v.kettle.lock = x.as_str() == Some("on"))
        .on("the user presses start", |v| v.kettle.start())
        .on("the user presses reset", |v| v.kettle.reset())
        .on("the user presses preset", |v| v.kettle.press_preset())
        .on("the user presses lock", |v| v.kettle.press_lock())
        .on("the water runs out", |v| v.kettle.run_out())
        .on_reset(|v| v.kettle = Kettle::default())
        .on_screenshot(|v| capture(v.window_id).inspect_err(|e| eprintln!("screenshot: {e}")))
}

/// The window as it is on screen, through macOS's `screencapture`.
fn capture(window_id: Option<i64>) -> Result<Reply, String> {
    let id = window_id.ok_or("no window id")?;
    let path = std::env::temp_dir().join(format!("kettle-gpui-{}.png", std::process::id()));
    let status = std::process::Command::new("screencapture")
        .args(["-x", "-o", "-l", &id.to_string()])
        .arg(&path)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("screencapture -l {id} failed: {status}"));
    }
    std::fs::read(&path).map(Reply::Png).map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn window_id(window: &Window) -> Option<i64> {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let RawWindowHandle::AppKit(h) = HasWindowHandle::window_handle(window).ok()?.as_raw() else { return None };
    unsafe {
        let view: &AnyObject = h.ns_view.cast().as_ref();
        let ns_window: *mut AnyObject = msg_send![view, window];
        let ns_window = ns_window.as_ref()?;
        let number: isize = msg_send![ns_window, windowNumber];
        Some(number as i64)
    }
}

#[cfg(not(target_os = "macos"))]
fn window_id(_: &Window) -> Option<i64> {
    None
}

impl KettleView {
    fn new(cx: &mut Context<Self>) -> Self {
        let h = handlers();
        let client = Client::connect_env(h.hello("GPUI", "Kettle"));
        // The kettle's clock, and the driver's commands, on the UI thread.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let alive = this.update(cx, |view, cx| {
                    while let Some(request) = client.try_recv() {
                        h.handle(view, request);
                    }
                    view.kettle.tick(TICK.as_secs_f64());
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
        KettleView { kettle: Kettle::default(), window_id: None }
    }
}

const BG: u32 = 0x1a1b17;
const PANEL: u32 = 0x2b2d26;
const EDGE: u32 = 0x45483d;
const SCREEN: u32 = 0x050806;
const TEXT: u32 = 0xdfe8df;
const MUTED: u32 = 0x919c91;
const OK: u32 = 0x4cff7a;
const CAUTION: u32 = 0xffb21f;
const WARNING: u32 = 0xff5a4f;

fn button(label: &'static str, cx: &mut Context<KettleView>, on: impl Fn(&mut Kettle) + 'static) -> impl IntoElement {
    div()
        .id(label)
        .flex_1()
        .py_3()
        .flex()
        .justify_center()
        .rounded_md()
        .border_1()
        .border_color(rgb(EDGE))
        .bg(rgb(0x161a16))
        .text_color(rgb(TEXT))
        .font_weight(gpui::FontWeight::BOLD)
        .hover(|s| s.bg(rgb(0x2a302a)))
        .child(label)
        .on_click(cx.listener(move |view, _, _, cx| {
            on(&mut view.kettle);
            cx.notify();
        }))
}

impl Render for KettleView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The window only has a number once it's on screen.
        if self.window_id.is_none_or(|id| id <= 0) {
            self.window_id = window_id(window);
        }
        let k = &self.kettle;
        let (state, state_color) = match k.phase {
            Phase::Off => ("OFF", TEXT),
            Phase::Starting { .. } => ("STARTING", CAUTION),
            Phase::Heating => ("HEATING", OK),
            Phase::Dry => ("DRY", WARNING),
        };
        let light: Rgba =
            if matches!(k.phase, Phase::Heating | Phase::Starting { .. }) { rgb(WARNING) } else { rgb(0x3a403a) };
        let row = |label: &'static str, value: String, color: u32| {
            div()
                .flex()
                .gap_4()
                .child(div().w(px(80.)).text_color(rgb(MUTED)).child(label))
                .child(div().font_weight(gpui::FontWeight::BOLD).text_color(rgb(color)).child(value))
        };

        let display = div()
            .p_4()
            .rounded_lg()
            .bg(rgb(SCREEN))
            .flex()
            .flex_col()
            .gap_2()
            .child(row("PRESET", PRESETS[k.preset].into(), TEXT))
            .child(row("STATE", state.into(), state_color))
            .child(row("WATER", format!("{:.0} °C", k.temperature), TEXT))
            .child(
                div().flex().gap_3().items_center().pt_2().child(div().size(px(14.)).rounded_full().bg(light)).child(
                    div()
                        .px_2()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(if k.lock { CAUTION } else { EDGE }))
                        .text_color(rgb(if k.lock { CAUTION } else { 0x3a403a }))
                        .font_weight(gpui::FontWeight::BOLD)
                        .child("LOCK"),
                ),
            );

        // The dial: a slider of 21 stops, clicked or dragged.
        let stops = (0..=20usize).map(|i| {
            let value = i as f64 * 5.0;
            let filled = value <= k.dial && k.dial > 0.0;
            div()
                .id(("stop", i))
                .flex_1()
                .h(px(28.))
                .bg(if filled { rgb(CAUTION) } else { rgb(0x161a16) })
                .border_r_1()
                .border_color(rgb(if i == 11 { TEXT } else { BG }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, _: &MouseDownEvent, _, cx| {
                        view.kettle.dial = value;
                        cx.notify();
                    }),
                )
                .on_mouse_move(cx.listener(move |view, ev: &MouseMoveEvent, _, cx| {
                    if ev.pressed_button == Some(MouseButton::Left) {
                        view.kettle.dial = value;
                        cx.notify();
                    }
                }))
        });
        let dial = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_color(rgb(MUTED))
                    .child("DIAL")
                    .child(div().text_color(rgb(TEXT)).child(format!("{:.0} %", k.dial))),
            )
            .child(div().flex().rounded_md().overflow_hidden().border_1().border_color(rgb(EDGE)).children(stops));

        div().size_full().bg(rgb(BG)).p_4().flex().flex_col().gap_4().font_family("Menlo").text_sm().child(
            div().p_3().rounded_xl().bg(rgb(PANEL)).flex().flex_col().gap_4().child(display).child(dial).child(
                div()
                    .flex()
                    .gap_2()
                    .child(button("START", cx, Kettle::start))
                    .child(button("RESET", cx, Kettle::reset))
                    .child(button("PRESET", cx, Kettle::press_preset))
                    .child(button("LOCK", cx, Kettle::press_lock)),
            ),
        )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(440.), px(380.)), cx);
        // Under test, stay above other windows: macOS stops drawing a window that's covered,
        // and screenshots would show a stale frame.
        let kind = if std::env::var_os("EXPECTATION_URL").is_some() { WindowKind::PopUp } else { WindowKind::Normal };
        cx.open_window(
            WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), kind, ..Default::default() },
            |_, cx| cx.new(KettleView::new),
        )
        .unwrap();
        cx.activate(true);
    });
}
