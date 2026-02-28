use std::ffi::{c_char, c_void, CStr};
use std::fmt::Write as _;
use std::ptr;
use std::sync::{Mutex, Once};

use objc2::declare::ClassBuilder;
use objc2::runtime::{AnyClass, AnyObject, Sel};
use objc2::{class, msg_send, sel};
use objc2_foundation::{NSPoint, NSRect, NSSize};

const UTF8_ENCODING: usize = 4;
const POLL_INTERVAL_SECS: f64 = 0.3;
const HUD_DURATION_SECS: f64 = 1.0;
const BORDERLESS_MASK: usize = 0;
const BACKING_BUFFERED: isize = 2;
const FLOATING_WINDOW_LEVEL: isize = 3;
const HUD_MIN_WIDTH: f64 = 200.0;
const HUD_MAX_WIDTH: f64 = 820.0;
const HUD_MIN_HEIGHT: f64 = 52.0;
const HUD_MAX_HEIGHT: f64 = 280.0;
const HUD_HORIZONTAL_PADDING: f64 = 16.0;
const HUD_VERTICAL_PADDING: f64 = 10.0;
const HUD_ICON_WIDTH: f64 = 22.0;
const HUD_ICON_HEIGHT: f64 = 22.0;
const HUD_GAP: f64 = 8.0;
const HUD_CHAR_WIDTH_ESTIMATE: f64 = 9.6;
const HUD_LINE_HEIGHT_ESTIMATE: f64 = 22.0;
const HUD_TEXT_MEASURE_HEIGHT: f64 = 10_000.0;
const BITMAP_IMAGE_FILE_TYPE_PNG: usize = 4;
const PIXEL_CHANNEL_TOLERANCE: u8 = 2;

struct AppState {
    last_change_count: isize,
    pasteboard: *mut AnyObject,
    window: *mut AnyObject,
    icon_label: *mut AnyObject,
    label: *mut AnyObject,
    hide_timer: *mut AnyObject,
}

// All UI interactions happen on the AppKit main thread.
unsafe impl Send for AppState {}

#[derive(Debug, Clone, Copy, PartialEq)]
struct HudLayoutMetrics {
    width: f64,
    text_width: f64,
    height: f64,
    text_height: f64,
    label_y: f64,
    icon_y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DiffSummary {
    diff_pixels: usize,
    total_pixels: usize,
}

static APP_STATE: Mutex<Option<AppState>> = Mutex::new(None);

fn main() {
    if handle_cli_flags() {
        return;
    }

    unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let _: bool = msg_send![app, setActivationPolicy: 1isize];

        let delegate_class = get_delegate_class();
        let delegate: *mut AnyObject = msg_send![delegate_class, new];
        let () = msg_send![app, setDelegate: delegate];
        let () = msg_send![app, run];
    }
}

fn handle_cli_flags() -> bool {
    let mut args = std::env::args();
    let _program = args.next();
    let Some(flag) = args.next() else {
        return false;
    };

    match flag.as_str() {
        "--version" | "-V" | "-v" => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            true
        }
        "--help" | "-h" => {
            let mut help = String::new();
            let _ = writeln!(help, "cliip-show {}", env!("CARGO_PKG_VERSION"));
            let _ = writeln!(help, "clipboard HUD resident app for macOS");
            let _ = writeln!(help);
            let _ = writeln!(help, "Options:");
            let _ = writeln!(help, "  -h, --help       Print help");
            let _ = writeln!(help, "  -v, -V, --version    Print version");
            let _ = writeln!(
                help,
                "  --render-hud-png --text <TEXT> --output <PATH>    Render HUD snapshot PNG and exit"
            );
            let _ = writeln!(
                help,
                "  --diff-png --baseline <PATH> --current <PATH> --output <PATH>    Generate visual diff PNG and exit"
            );
            print!("{help}");
            true
        }
        "--render-hud-png" => {
            let mut text: Option<String> = None;
            let mut output_path: Option<String> = None;

            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--text" => {
                        let Some(value) = args.next() else {
                            eprintln!("Missing value for --text");
                            std::process::exit(2);
                        };
                        text = Some(value);
                    }
                    "--output" => {
                        let Some(value) = args.next() else {
                            eprintln!("Missing value for --output");
                            std::process::exit(2);
                        };
                        output_path = Some(value);
                    }
                    unknown => {
                        eprintln!("Unknown option for --render-hud-png: {unknown}");
                        std::process::exit(2);
                    }
                }
            }

            let text = text.unwrap_or_else(|| "Clipboard text".to_string());
            let Some(output_path) = output_path else {
                eprintln!("--output is required for --render-hud-png");
                std::process::exit(2);
            };

            if let Err(error) = render_hud_png(&text, &output_path) {
                eprintln!("{error}");
                std::process::exit(1);
            }
            true
        }
        "--diff-png" => {
            let mut baseline_path: Option<String> = None;
            let mut current_path: Option<String> = None;
            let mut output_path: Option<String> = None;

            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--baseline" => {
                        let Some(value) = args.next() else {
                            eprintln!("Missing value for --baseline");
                            std::process::exit(2);
                        };
                        baseline_path = Some(value);
                    }
                    "--current" => {
                        let Some(value) = args.next() else {
                            eprintln!("Missing value for --current");
                            std::process::exit(2);
                        };
                        current_path = Some(value);
                    }
                    "--output" => {
                        let Some(value) = args.next() else {
                            eprintln!("Missing value for --output");
                            std::process::exit(2);
                        };
                        output_path = Some(value);
                    }
                    unknown => {
                        eprintln!("Unknown option for --diff-png: {unknown}");
                        std::process::exit(2);
                    }
                }
            }

            let Some(baseline_path) = baseline_path else {
                eprintln!("--baseline is required for --diff-png");
                std::process::exit(2);
            };
            let Some(current_path) = current_path else {
                eprintln!("--current is required for --diff-png");
                std::process::exit(2);
            };
            let Some(output_path) = output_path else {
                eprintln!("--output is required for --diff-png");
                std::process::exit(2);
            };

            match generate_diff_png(&baseline_path, &current_path, &output_path) {
                Ok(summary) => {
                    println!(
                        "diff_pixels={} total_pixels={}",
                        summary.diff_pixels, summary.total_pixels
                    );
                }
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            }
            true
        }
        unknown => {
            eprintln!("Unknown option: {unknown}");
            eprintln!("Use --help to see available options.");
            std::process::exit(2);
        }
    }
}

fn render_hud_png(text: &str, output_path: &str) -> Result<(), String> {
    unsafe {
        let _app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let (window, icon_label, label) = create_hud_window();
        let truncated = truncate_text(text, 100, 5);
        let message = nsstring_from_str(&truncated);
        let () = msg_send![label, setStringValue: message];
        let () = msg_send![message, release];
        let hud_width = hud_width_for_text(&truncated);
        layout_hud(window, icon_label, label, hud_width);

        let content_view: *mut AnyObject = msg_send![window, contentView];
        if content_view.is_null() {
            return Err("failed to get contentView".to_string());
        }

        let bounds: NSRect = msg_send![content_view, bounds];
        let bitmap = create_bitmap_rep_for_bounds(bounds)?;
        if bitmap.is_null() {
            return Err("failed to create bitmap image rep".to_string());
        }

        let () = msg_send![content_view, cacheDisplayInRect: bounds toBitmapImageRep: bitmap];
        let properties: *mut AnyObject = msg_send![class!(NSDictionary), dictionary];
        let data: *mut AnyObject = msg_send![
            bitmap,
            representationUsingType: BITMAP_IMAGE_FILE_TYPE_PNG
            properties: properties
        ];
        if data.is_null() {
            return Err("failed to encode PNG data".to_string());
        }

        let output_path_ns = nsstring_from_str(output_path);
        let success: bool = msg_send![data, writeToFile: output_path_ns atomically: true];
        let () = msg_send![output_path_ns, release];
        let () = msg_send![window, close];

        if !success {
            return Err(format!("failed to write PNG: {output_path}"));
        }
    }

    Ok(())
}

fn generate_diff_png(
    baseline_path: &str,
    current_path: &str,
    output_path: &str,
) -> Result<DiffSummary, String> {
    unsafe {
        let baseline_path_ns = nsstring_from_str(baseline_path);
        let baseline_rep: *mut AnyObject =
            msg_send![class!(NSBitmapImageRep), imageRepWithContentsOfFile: baseline_path_ns];
        let () = msg_send![baseline_path_ns, release];
        if baseline_rep.is_null() {
            return Err(format!("failed to load baseline PNG: {baseline_path}"));
        }

        let current_path_ns = nsstring_from_str(current_path);
        let current_rep: *mut AnyObject =
            msg_send![class!(NSBitmapImageRep), imageRepWithContentsOfFile: current_path_ns];
        let () = msg_send![current_path_ns, release];
        if current_rep.is_null() {
            return Err(format!("failed to load current PNG: {current_path}"));
        }

        let baseline_width: isize = msg_send![baseline_rep, pixelsWide];
        let baseline_height: isize = msg_send![baseline_rep, pixelsHigh];
        let current_width: isize = msg_send![current_rep, pixelsWide];
        let current_height: isize = msg_send![current_rep, pixelsHigh];
        if baseline_width != current_width || baseline_height != current_height {
            return Err(format!(
                "image size mismatch: baseline={}x{}, current={}x{}",
                baseline_width, baseline_height, current_width, current_height
            ));
        }

        let diff_rep: *mut AnyObject = msg_send![current_rep, copy];
        if diff_rep.is_null() {
            return Err("failed to create diff image".to_string());
        }

        let mut diff_pixels: usize = 0;
        let total_pixels = (baseline_width * baseline_height) as usize;

        for x in 0..baseline_width {
            for y in 0..baseline_height {
                let baseline_color: *mut AnyObject = msg_send![baseline_rep, colorAtX: x y: y];
                let current_color: *mut AnyObject = msg_send![current_rep, colorAtX: x y: y];
                let Some((br, bg, bb, ba)) = color_components(baseline_color) else {
                    continue;
                };
                let Some((cr, cg, cb, ca)) = color_components(current_color) else {
                    continue;
                };

                let same = to_u8(br) == to_u8(cr)
                    && to_u8(bg) == to_u8(cg)
                    && to_u8(bb) == to_u8(cb)
                    && to_u8(ba) == to_u8(ca);

                let same = same
                    || (to_u8(br).abs_diff(to_u8(cr)) <= PIXEL_CHANNEL_TOLERANCE
                        && to_u8(bg).abs_diff(to_u8(cg)) <= PIXEL_CHANNEL_TOLERANCE
                        && to_u8(bb).abs_diff(to_u8(cb)) <= PIXEL_CHANNEL_TOLERANCE
                        && to_u8(ba).abs_diff(to_u8(ca)) <= PIXEL_CHANNEL_TOLERANCE);

                let color: *mut AnyObject = if same {
                    let gray = ((cr + cg + cb) / 3.0).clamp(0.0, 1.0);
                    msg_send![class!(NSColor), colorWithCalibratedRed: gray green: gray blue: gray alpha: 0.08f64]
                } else {
                    diff_pixels += 1;
                    let delta = (to_u8(cr).abs_diff(to_u8(br)))
                        .max(to_u8(cg).abs_diff(to_u8(bg)))
                        .max(to_u8(cb).abs_diff(to_u8(bb)));
                    let intensity = (f64::from(delta.max(128))) / 255.0;
                    msg_send![class!(NSColor), colorWithCalibratedRed: intensity green: 0.0f64 blue: 0.0f64 alpha: 0.9f64]
                };
                let () = msg_send![diff_rep, setColor: color atX: x y: y];
            }
        }

        let properties: *mut AnyObject = msg_send![class!(NSDictionary), dictionary];
        let data: *mut AnyObject = msg_send![
            diff_rep,
            representationUsingType: BITMAP_IMAGE_FILE_TYPE_PNG
            properties: properties
        ];
        if data.is_null() {
            let () = msg_send![diff_rep, release];
            return Err("failed to encode diff PNG".to_string());
        }

        let output_path_ns = nsstring_from_str(output_path);
        let success: bool = msg_send![data, writeToFile: output_path_ns atomically: true];
        let () = msg_send![output_path_ns, release];
        let () = msg_send![diff_rep, release];

        if !success {
            return Err(format!("failed to write diff PNG: {output_path}"));
        }

        Ok(DiffSummary {
            diff_pixels,
            total_pixels,
        })
    }
}

unsafe fn color_components(color: *mut AnyObject) -> Option<(f64, f64, f64, f64)> {
    if color.is_null() {
        return None;
    }

    let device_rgb_name = nsstring_from_str("NSDeviceRGBColorSpace");
    let rgb_color: *mut AnyObject = msg_send![color, colorUsingColorSpaceName: device_rgb_name];
    let () = msg_send![device_rgb_name, release];
    if rgb_color.is_null() {
        return None;
    }

    let r: f64 = msg_send![rgb_color, redComponent];
    let g: f64 = msg_send![rgb_color, greenComponent];
    let b: f64 = msg_send![rgb_color, blueComponent];
    let a: f64 = msg_send![rgb_color, alphaComponent];
    Some((r, g, b, a))
}

fn to_u8(component: f64) -> u8 {
    (component.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn create_bitmap_rep_for_bounds(bounds: NSRect) -> Result<*mut AnyObject, String> {
    let width = bounds.size.width.ceil().max(1.0) as isize;
    let height = bounds.size.height.ceil().max(1.0) as isize;
    unsafe {
        let bitmap: *mut AnyObject = msg_send![class!(NSBitmapImageRep), alloc];
        let color_space = nsstring_from_str("NSCalibratedRGBColorSpace");
        let bitmap: *mut AnyObject = msg_send![
            bitmap,
            initWithBitmapDataPlanes: ptr::null_mut::<*mut u8>()
            pixelsWide: width
            pixelsHigh: height
            bitsPerSample: 8isize
            samplesPerPixel: 4isize
            hasAlpha: true
            isPlanar: false
            colorSpaceName: color_space
            bytesPerRow: 0isize
            bitsPerPixel: 0isize
        ];
        let () = msg_send![color_space, release];

        if bitmap.is_null() {
            return Err("failed to allocate fixed-size bitmap image rep".to_string());
        }

        Ok(bitmap)
    }
}

fn get_delegate_class() -> &'static AnyClass {
    static ONCE: Once = Once::new();
    static mut CLASS: *const AnyClass = ptr::null();

    ONCE.call_once(|| unsafe {
        let mut builder = ClassBuilder::new("ClipboardHudAppDelegate", class!(NSObject))
            .expect("delegate class creation failed");

        builder.add_method(
            sel!(applicationDidFinishLaunching:),
            application_did_finish_launching as extern "C" fn(_, _, _),
        );
        builder.add_method(
            sel!(pollPasteboard:),
            poll_pasteboard as extern "C" fn(_, _, _),
        );
        builder.add_method(sel!(hideHud:), hide_hud as extern "C" fn(_, _, _));

        let class = builder.register();
        CLASS = class as *const AnyClass;
    });

    unsafe { &*CLASS }
}

extern "C" fn application_did_finish_launching(this: &AnyObject, _: Sel, _: *mut AnyObject) {
    unsafe {
        let pasteboard: *mut AnyObject = msg_send![class!(NSPasteboard), generalPasteboard];
        let last_change_count: isize = msg_send![pasteboard, changeCount];

        let (window, icon_label, label) = create_hud_window();

        *APP_STATE.lock().expect("APP_STATE lock poisoned") = Some(AppState {
            last_change_count,
            pasteboard,
            window,
            icon_label,
            label,
            hide_timer: ptr::null_mut(),
        });

        let _: *mut AnyObject = msg_send![
            class!(NSTimer),
            scheduledTimerWithTimeInterval: POLL_INTERVAL_SECS
            target: this
            selector: sel!(pollPasteboard:)
            userInfo: ptr::null_mut::<AnyObject>()
            repeats: true
        ];
    }
}

extern "C" fn poll_pasteboard(this: &AnyObject, _: Sel, _: *mut AnyObject) {
    unsafe {
        let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
        let Some(state) = guard.as_mut() else {
            return;
        };

        let change_count: isize = msg_send![state.pasteboard, changeCount];
        if change_count == state.last_change_count {
            return;
        }
        state.last_change_count = change_count;

        let text_type = nsstring_from_str("public.utf8-plain-text");
        let raw_text: *mut AnyObject = msg_send![state.pasteboard, stringForType: text_type];
        let () = msg_send![text_type, release];

        let Some(text) = nsstring_to_string(raw_text) else {
            return;
        };

        let truncated = truncate_text(&text, 100, 5);
        let message = nsstring_from_str(&truncated);
        let () = msg_send![state.label, setStringValue: message];
        let () = msg_send![message, release];

        let hud_width = hud_width_for_text(&truncated);
        layout_hud(state.window, state.icon_label, state.label, hud_width);
        let () = msg_send![state.window, orderFrontRegardless];

        if !state.hide_timer.is_null() {
            let () = msg_send![state.hide_timer, invalidate];
        }

        let hide_timer: *mut AnyObject = msg_send![
            class!(NSTimer),
            scheduledTimerWithTimeInterval: HUD_DURATION_SECS
            target: this
            selector: sel!(hideHud:)
            userInfo: ptr::null_mut::<AnyObject>()
            repeats: false
        ];
        state.hide_timer = hide_timer;
    }
}

extern "C" fn hide_hud(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    unsafe {
        let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
        let Some(state) = guard.as_mut() else {
            return;
        };

        let () = msg_send![state.window, orderOut: ptr::null_mut::<AnyObject>()];

        if !state.hide_timer.is_null() {
            let () = msg_send![state.hide_timer, invalidate];
            state.hide_timer = ptr::null_mut();
        }
    }
}

unsafe fn create_hud_window() -> (*mut AnyObject, *mut AnyObject, *mut AnyObject) {
    let default_width = 600.0;
    let default_height = HUD_MIN_HEIGHT;
    let mut rect = NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: NSSize {
            width: default_width,
            height: default_height,
        },
    };

    if let Some((x, y)) = centered_origin(default_width, default_height) {
        rect.origin = NSPoint { x, y };
    }

    let window: *mut AnyObject = msg_send![class!(NSWindow), alloc];
    let window: *mut AnyObject = msg_send![
        window,
        initWithContentRect: rect
        styleMask: BORDERLESS_MASK
        backing: BACKING_BUFFERED
        defer: false
    ];

    let () = msg_send![window, setOpaque: false];
    let () = msg_send![window, setHasShadow: true];
    let () = msg_send![window, setIgnoresMouseEvents: true];
    let () = msg_send![window, setLevel: FLOATING_WINDOW_LEVEL];

    let clear: *mut AnyObject = msg_send![class!(NSColor), clearColor];
    let () = msg_send![window, setBackgroundColor: clear];

    let content_view: *mut AnyObject = msg_send![window, contentView];
    let () = msg_send![content_view, setWantsLayer: true];
    let layer: *mut AnyObject = msg_send![content_view, layer];
    let () = msg_send![layer, setCornerRadius: 14.0f64];
    let () = msg_send![layer, setMasksToBounds: true];

    let bg: *mut AnyObject =
        msg_send![class!(NSColor), colorWithCalibratedWhite: 0.0f64 alpha: 0.78f64];
    let cg_color: *mut c_void = msg_send![bg, CGColor];
    let () = msg_send![layer, setBackgroundColor: cg_color];
    let border_color_obj: *mut AnyObject =
        msg_send![class!(NSColor), colorWithCalibratedWhite: 1.0f64 alpha: 0.14f64];
    let border_color: *mut c_void = msg_send![border_color_obj, CGColor];
    let () = msg_send![layer, setBorderColor: border_color];
    let () = msg_send![layer, setBorderWidth: 1.0f64];

    let icon_rect = NSRect {
        origin: NSPoint {
            x: HUD_HORIZONTAL_PADDING,
            y: ((default_height - HUD_LINE_HEIGHT_ESTIMATE) / 2.0 + HUD_LINE_HEIGHT_ESTIMATE
                - HUD_ICON_HEIGHT)
                .max(HUD_VERTICAL_PADDING),
        },
        size: NSSize {
            width: HUD_ICON_WIDTH,
            height: HUD_ICON_HEIGHT,
        },
    };

    let icon_label: *mut AnyObject = msg_send![class!(NSTextField), alloc];
    let icon_label: *mut AnyObject = msg_send![icon_label, initWithFrame: icon_rect];
    let () = msg_send![icon_label, setBezeled: false];
    let () = msg_send![icon_label, setBordered: false];
    let () = msg_send![icon_label, setEditable: false];
    let () = msg_send![icon_label, setSelectable: false];
    let () = msg_send![icon_label, setDrawsBackground: false];
    let () = msg_send![icon_label, setAlignment: 1isize];
    let () = msg_send![icon_label, setLineBreakMode: 0isize];
    let () = msg_send![icon_label, setUsesSingleLineMode: true];
    let white: *mut AnyObject = msg_send![class!(NSColor), whiteColor];
    let () = msg_send![icon_label, setTextColor: white];
    let icon_font: *mut AnyObject = msg_send![class!(NSFont), systemFontOfSize: 18.0f64];
    let () = msg_send![icon_label, setFont: icon_font];
    let icon_text = nsstring_from_str("📋");
    let () = msg_send![icon_label, setStringValue: icon_text];
    let () = msg_send![icon_text, release];

    let label_rect = NSRect {
        origin: NSPoint {
            x: HUD_HORIZONTAL_PADDING + HUD_ICON_WIDTH + HUD_GAP,
            y: (default_height - HUD_LINE_HEIGHT_ESTIMATE) / 2.0,
        },
        size: NSSize {
            width: default_width - (HUD_HORIZONTAL_PADDING * 2.0 + HUD_ICON_WIDTH + HUD_GAP),
            height: HUD_LINE_HEIGHT_ESTIMATE,
        },
    };

    let label: *mut AnyObject = msg_send![class!(NSTextField), alloc];
    let label: *mut AnyObject = msg_send![label, initWithFrame: label_rect];

    let () = msg_send![label, setBezeled: false];
    let () = msg_send![label, setBordered: false];
    let () = msg_send![label, setEditable: false];
    let () = msg_send![label, setSelectable: false];
    let () = msg_send![label, setDrawsBackground: false];
    let () = msg_send![label, setLineBreakMode: 2isize];
    let () = msg_send![label, setUsesSingleLineMode: false];
    let () = msg_send![label, setMaximumNumberOfLines: 0isize];
    let () = msg_send![label, setAlignment: 0isize];

    let () = msg_send![label, setTextColor: white];

    let menlo_name = nsstring_from_str("Menlo");
    let font: *mut AnyObject = msg_send![class!(NSFont), fontWithName: menlo_name size: 18.0f64];
    let () = msg_send![menlo_name, release];
    if !font.is_null() {
        let () = msg_send![label, setFont: font];
    }

    let cell: *mut AnyObject = msg_send![label, cell];
    if !cell.is_null() {
        let () = msg_send![cell, setWraps: true];
        let () = msg_send![cell, setScrollable: false];
        let () = msg_send![cell, setLineBreakMode: 2isize];
    }

    let default_text = nsstring_from_str("Clipboard text");
    let () = msg_send![label, setStringValue: default_text];
    let () = msg_send![default_text, release];

    let () = msg_send![content_view, addSubview: icon_label];
    let () = msg_send![content_view, addSubview: label];
    let () = msg_send![window, orderOut: ptr::null_mut::<AnyObject>()];

    (window, icon_label, label)
}

unsafe fn centered_origin(width: f64, height: f64) -> Option<(f64, f64)> {
    let screen: *mut AnyObject = msg_send![class!(NSScreen), mainScreen];
    if screen.is_null() {
        return None;
    }

    let frame: NSRect = msg_send![screen, frame];
    let x = frame.origin.x + (frame.size.width - width) / 2.0;
    let y = frame.origin.y + (frame.size.height - height) / 2.0;
    Some((x, y))
}

unsafe fn center_window(window: *mut AnyObject, width: f64, height: f64) {
    let (x, y) = centered_origin(width, height).unwrap_or((0.0, 0.0));

    let rect = NSRect {
        origin: NSPoint { x, y },
        size: NSSize { width, height },
    };
    let () = msg_send![window, setFrame: rect display: true];
}

unsafe fn layout_hud(
    window: *mut AnyObject,
    icon_label: *mut AnyObject,
    label: *mut AnyObject,
    width: f64,
) {
    let clamped_width = width.clamp(HUD_MIN_WIDTH, HUD_MAX_WIDTH);
    let text_width = clamped_width - (HUD_HORIZONTAL_PADDING * 2.0 + HUD_ICON_WIDTH + HUD_GAP);
    let measured_text_height = measure_text_height(label, text_width);
    let metrics = compute_hud_layout_metrics(clamped_width, measured_text_height);

    let icon_rect = NSRect {
        origin: NSPoint {
            x: HUD_HORIZONTAL_PADDING,
            y: metrics.icon_y,
        },
        size: NSSize {
            width: HUD_ICON_WIDTH,
            height: HUD_ICON_HEIGHT,
        },
    };
    let label_rect = NSRect {
        origin: NSPoint {
            x: HUD_HORIZONTAL_PADDING + HUD_ICON_WIDTH + HUD_GAP,
            y: metrics.label_y,
        },
        size: NSSize {
            width: metrics.text_width,
            height: metrics.text_height,
        },
    };

    let () = msg_send![icon_label, setFrame: icon_rect];
    let () = msg_send![label, setFrame: label_rect];
    center_window(window, metrics.width, metrics.height);
}

unsafe fn measure_text_height(label: *mut AnyObject, text_width: f64) -> f64 {
    let cell: *mut AnyObject = msg_send![label, cell];
    if cell.is_null() {
        return HUD_LINE_HEIGHT_ESTIMATE;
    }

    let bounds = NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: NSSize {
            width: text_width.max(1.0),
            height: HUD_TEXT_MEASURE_HEIGHT,
        },
    };
    let size: NSSize = msg_send![cell, cellSizeForBounds: bounds];
    size.height.ceil().max(HUD_LINE_HEIGHT_ESTIMATE)
}

fn compute_hud_layout_metrics(width: f64, measured_text_height: f64) -> HudLayoutMetrics {
    let width = width.clamp(HUD_MIN_WIDTH, HUD_MAX_WIDTH);
    let text_width = width - (HUD_HORIZONTAL_PADDING * 2.0 + HUD_ICON_WIDTH + HUD_GAP);
    let measured_text_height = measured_text_height
        .min((HUD_MAX_HEIGHT - HUD_VERTICAL_PADDING * 2.0).max(HUD_LINE_HEIGHT_ESTIMATE));
    let height =
        (measured_text_height + HUD_VERTICAL_PADDING * 2.0).clamp(HUD_MIN_HEIGHT, HUD_MAX_HEIGHT);
    let text_height = (height - HUD_VERTICAL_PADDING * 2.0)
        .min(measured_text_height)
        .max(HUD_LINE_HEIGHT_ESTIMATE);
    let label_y = (height - text_height) / 2.0;
    let icon_y = (label_y + text_height - HUD_ICON_HEIGHT)
        .max(HUD_VERTICAL_PADDING)
        .min(height - HUD_ICON_HEIGHT - HUD_VERTICAL_PADDING);

    HudLayoutMetrics {
        width,
        text_width,
        height,
        text_height,
        label_y,
        icon_y,
    }
}

unsafe fn nsstring_from_str(value: &str) -> *mut AnyObject {
    let ns_string: *mut AnyObject = msg_send![class!(NSString), alloc];
    msg_send![
        ns_string,
        initWithBytes: value.as_ptr() as *const c_void
        length: value.len()
        encoding: UTF8_ENCODING
    ]
}

unsafe fn nsstring_to_string(value: *mut AnyObject) -> Option<String> {
    if value.is_null() {
        return None;
    }

    let utf8_ptr: *const c_char = msg_send![value, UTF8String];
    if utf8_ptr.is_null() {
        return Some(String::new());
    }

    Some(CStr::from_ptr(utf8_ptr).to_string_lossy().into_owned())
}

fn truncate_text(text: &str, max_width: usize, max_lines: usize) -> String {
    let mut lines: Vec<String> = split_non_trailing_lines(text)
        .into_iter()
        .map(|line| truncate_line(line, max_width))
        .collect();

    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            *last = append_ellipsis(last, max_width);
        }
    }

    lines.join("\n")
}

fn truncate_line(line: &str, max_width: usize) -> String {
    let count = line.chars().count();
    if count <= max_width {
        return line.to_string();
    }

    if max_width <= 3 {
        return "...".chars().take(max_width).collect();
    }

    let kept: String = line.chars().take(max_width - 3).collect();
    format!("{kept}...")
}

fn append_ellipsis(line: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    if max_width <= 3 {
        return "...".chars().take(max_width).collect();
    }

    let current_len = line.chars().count();
    if current_len + 3 <= max_width {
        return format!("{line}...");
    }

    let kept: String = line.chars().take(max_width - 3).collect();
    format!("{kept}...")
}

fn hud_width_for_text(text: &str) -> f64 {
    let lines = split_non_trailing_lines(text);
    let max_units = lines
        .iter()
        .map(|line| line_display_units(line))
        .fold(1.0f64, f64::max);

    (max_units * HUD_CHAR_WIDTH_ESTIMATE + HUD_HORIZONTAL_PADDING * 2.0 + HUD_ICON_WIDTH + HUD_GAP)
        .clamp(HUD_MIN_WIDTH, HUD_MAX_WIDTH)
}

fn split_non_trailing_lines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text
        .split_terminator('\n')
        .map(|line| line.trim_end_matches('\r'))
        .collect();

    while matches!(lines.last(), Some(last) if last.trim().is_empty()) {
        lines.pop();
    }

    if lines.is_empty() {
        lines.push("");
    }
    lines
}

fn line_display_units(line: &str) -> f64 {
    let units: f64 = line
        .chars()
        .map(|c| if c.is_ascii() { 1.0 } else { 2.0 })
        .sum();
    units.max(1.0)
}

#[cfg(test)]
mod tests {
    use super::{compute_hud_layout_metrics, hud_width_for_text, truncate_text};

    #[test]
    fn truncates_single_long_line() {
        let input = "abcdefghijklmnopqrstuvwxyz";
        assert_eq!(truncate_text(input, 10, 5), "abcdefg...");
    }

    #[test]
    fn truncates_lines_count_and_adds_ellipsis_to_last_line() {
        let input = "line1\nline2\nline3\nline4\nline5\nline6";
        assert_eq!(
            truncate_text(input, 100, 5),
            "line1\nline2\nline3\nline4\nline5..."
        );
    }

    #[test]
    fn handles_utf8_by_char_count() {
        let input = "あいうえおかきくけこ";
        assert_eq!(truncate_text(input, 6, 5), "あいう...");
    }

    #[test]
    fn hud_width_regression_snapshot() {
        let cases = vec![
            ("ascii_short", "hello".to_string()),
            ("ascii_40", "a".repeat(40)),
            ("wide_20", "あ".repeat(20)),
            ("ascii_very_long", "a".repeat(300)),
        ];

        let snapshot = cases
            .iter()
            .map(|(name, text)| format!("{name}: {:.1}", hud_width_for_text(text)))
            .collect::<Vec<_>>()
            .join("\n");

        let expected = "\
ascii_short: 200.0
ascii_40: 446.0
wide_20: 446.0
ascii_very_long: 820.0";

        assert_eq!(snapshot, expected);
    }

    #[test]
    fn hud_layout_regression_snapshot() {
        let cases = [
            ("one_line", 600.0, 22.0),
            ("three_lines", 600.0, 88.0),
            ("overflow", 600.0, 400.0),
            ("narrow_clamped", 100.0, 22.0),
        ];

        let snapshot = cases
            .iter()
            .map(|(name, width, measured)| {
                let metrics = compute_hud_layout_metrics(*width, *measured);
                format!(
                    "{name}: w={:.1} text_w={:.1} h={:.1} text_h={:.1} label_y={:.1} icon_y={:.1}",
                    metrics.width,
                    metrics.text_width,
                    metrics.height,
                    metrics.text_height,
                    metrics.label_y,
                    metrics.icon_y
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let expected = "\
one_line: w=600.0 text_w=538.0 h=52.0 text_h=22.0 label_y=15.0 icon_y=15.0
three_lines: w=600.0 text_w=538.0 h=108.0 text_h=88.0 label_y=10.0 icon_y=76.0
overflow: w=600.0 text_w=538.0 h=280.0 text_h=260.0 label_y=10.0 icon_y=248.0
narrow_clamped: w=200.0 text_w=138.0 h=52.0 text_h=22.0 label_y=15.0 icon_y=15.0";

        assert_eq!(snapshot, expected);
    }
}
