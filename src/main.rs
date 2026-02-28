use std::ffi::{c_char, c_void, CStr};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{Mutex, Once};

use objc2::declare::ClassBuilder;
use objc2::runtime::{AnyClass, AnyObject, Sel};
use objc2::{class, msg_send, sel};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use serde::{Deserialize, Serialize};

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
const HUD_CORNER_RADIUS: f64 = 14.0;
const HUD_BORDER_WIDTH: f64 = 1.0;
const HUD_ICON_FONT_SIZE: f64 = 18.0;
const HUD_TEXT_FONT_SIZE: f64 = 18.0;
const HUD_SCREEN_MARGIN: f64 = 24.0;
const BITMAP_IMAGE_FILE_TYPE_PNG: usize = 4;
const PIXEL_CHANNEL_TOLERANCE: u8 = 2;
const DEFAULT_TRUNCATE_MAX_WIDTH: usize = 100;
const DEFAULT_TRUNCATE_MAX_LINES: usize = 5;
const DEFAULT_HUD_SCALE: f64 = 1.0;

const MIN_POLL_INTERVAL_SECS: f64 = 0.05;
const MAX_POLL_INTERVAL_SECS: f64 = 5.0;
const MIN_HUD_DURATION_SECS: f64 = 0.1;
const MAX_HUD_DURATION_SECS: f64 = 10.0;
const MIN_HUD_SCALE: f64 = 0.5;
const MAX_HUD_SCALE: f64 = 2.0;
const MIN_TRUNCATE_MAX_WIDTH: usize = 1;
const MAX_TRUNCATE_MAX_WIDTH: usize = 500;
const MIN_TRUNCATE_MAX_LINES: usize = 1;
const MAX_TRUNCATE_MAX_LINES: usize = 20;
const DEFAULT_CONFIG_RELATIVE_PATH: &str = "Library/Application Support/cliip-show/config.toml";

struct AppState {
    last_change_count: isize,
    pasteboard: *mut AnyObject,
    window: *mut AnyObject,
    icon_label: *mut AnyObject,
    label: *mut AnyObject,
    hide_timer: *mut AnyObject,
    settings: DisplaySettings,
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct HudDimensions {
    min_width: f64,
    max_width: f64,
    min_height: f64,
    max_height: f64,
    horizontal_padding: f64,
    vertical_padding: f64,
    icon_width: f64,
    icon_height: f64,
    gap: f64,
    line_height_estimate: f64,
    char_width_estimate: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DiffSummary {
    diff_pixels: usize,
    total_pixels: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum HudPosition {
    Top,
    #[default]
    Center,
    Bottom,
}

impl HudPosition {
    fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Center => "center",
            Self::Bottom => "bottom",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum HudBackgroundColor {
    #[default]
    Default,
    Yellow,
    Blue,
    Green,
    Red,
    Purple,
}

impl HudBackgroundColor {
    fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Yellow => "yellow",
            Self::Blue => "blue",
            Self::Green => "green",
            Self::Red => "red",
            Self::Purple => "purple",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct DisplaySettings {
    poll_interval_secs: f64,
    hud_duration_secs: f64,
    truncate_max_width: usize,
    truncate_max_lines: usize,
    hud_position: HudPosition,
    hud_scale: f64,
    hud_background_color: HudBackgroundColor,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AppConfigFile {
    #[serde(default)]
    display: DisplayConfigFile,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DisplayConfigFile {
    poll_interval_secs: Option<f64>,
    hud_duration_secs: Option<f64>,
    max_chars_per_line: Option<usize>,
    max_lines: Option<usize>,
    hud_position: Option<HudPosition>,
    hud_scale: Option<f64>,
    hud_background_color: Option<HudBackgroundColor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigKey {
    PollIntervalSecs,
    HudDurationSecs,
    MaxCharsPerLine,
    MaxLines,
    HudPosition,
    HudScale,
    HudBackgroundColor,
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

fn default_display_settings() -> DisplaySettings {
    DisplaySettings {
        poll_interval_secs: POLL_INTERVAL_SECS,
        hud_duration_secs: HUD_DURATION_SECS,
        truncate_max_width: DEFAULT_TRUNCATE_MAX_WIDTH,
        truncate_max_lines: DEFAULT_TRUNCATE_MAX_LINES,
        hud_position: HudPosition::default(),
        hud_scale: DEFAULT_HUD_SCALE,
        hud_background_color: HudBackgroundColor::default(),
    }
}

fn display_settings() -> DisplaySettings {
    let mut settings = default_display_settings();
    match config_file_path() {
        Ok(config_path) => match load_config_file(&config_path) {
            Ok((config, _)) => {
                settings = apply_config_file(settings, &config);
            }
            Err(error) => {
                eprintln!("warning: {error}");
            }
        },
        Err(error) => {
            eprintln!("warning: {error}");
        }
    }
    apply_env_overrides(settings)
}

fn apply_config_file(base: DisplaySettings, config: &AppConfigFile) -> DisplaySettings {
    let mut settings = base;
    if let Some(value) = config.display.poll_interval_secs {
        settings.poll_interval_secs = parse_f64_value(
            value,
            settings.poll_interval_secs,
            MIN_POLL_INTERVAL_SECS,
            MAX_POLL_INTERVAL_SECS,
        );
    }
    if let Some(value) = config.display.hud_duration_secs {
        settings.hud_duration_secs = parse_f64_value(
            value,
            settings.hud_duration_secs,
            MIN_HUD_DURATION_SECS,
            MAX_HUD_DURATION_SECS,
        );
    }
    if let Some(value) = config.display.max_chars_per_line {
        settings.truncate_max_width =
            parse_usize_value(value, MIN_TRUNCATE_MAX_WIDTH, MAX_TRUNCATE_MAX_WIDTH);
    }
    if let Some(value) = config.display.max_lines {
        settings.truncate_max_lines =
            parse_usize_value(value, MIN_TRUNCATE_MAX_LINES, MAX_TRUNCATE_MAX_LINES);
    }
    if let Some(value) = config.display.hud_position {
        settings.hud_position = value;
    }
    if let Some(value) = config.display.hud_scale {
        settings.hud_scale =
            parse_f64_value(value, settings.hud_scale, MIN_HUD_SCALE, MAX_HUD_SCALE);
    }
    if let Some(value) = config.display.hud_background_color {
        settings.hud_background_color = value;
    }
    settings
}

fn apply_env_overrides(base: DisplaySettings) -> DisplaySettings {
    let mut settings = base;
    if let Some(value) = read_env_option("CLIIP_SHOW_POLL_INTERVAL_SECS") {
        settings.poll_interval_secs = parse_f64_setting(
            &value,
            settings.poll_interval_secs,
            MIN_POLL_INTERVAL_SECS,
            MAX_POLL_INTERVAL_SECS,
        );
    }
    if let Some(value) = read_env_option("CLIIP_SHOW_HUD_DURATION_SECS") {
        settings.hud_duration_secs = parse_f64_setting(
            &value,
            settings.hud_duration_secs,
            MIN_HUD_DURATION_SECS,
            MAX_HUD_DURATION_SECS,
        );
    }
    if let Some(value) = read_env_option("CLIIP_SHOW_MAX_CHARS_PER_LINE") {
        settings.truncate_max_width = parse_usize_setting(
            &value,
            settings.truncate_max_width,
            MIN_TRUNCATE_MAX_WIDTH,
            MAX_TRUNCATE_MAX_WIDTH,
        );
    }
    if let Some(value) = read_env_option("CLIIP_SHOW_MAX_LINES") {
        settings.truncate_max_lines = parse_usize_setting(
            &value,
            settings.truncate_max_lines,
            MIN_TRUNCATE_MAX_LINES,
            MAX_TRUNCATE_MAX_LINES,
        );
    }
    if let Some(value) = read_env_option("CLIIP_SHOW_HUD_POSITION") {
        settings.hud_position = parse_hud_position_setting(&value, settings.hud_position);
    }
    if let Some(value) = read_env_option("CLIIP_SHOW_HUD_SCALE") {
        settings.hud_scale =
            parse_f64_setting(&value, settings.hud_scale, MIN_HUD_SCALE, MAX_HUD_SCALE);
    }
    if let Some(value) = read_env_option("CLIIP_SHOW_HUD_BACKGROUND_COLOR") {
        settings.hud_background_color =
            parse_hud_background_color_setting(&value, settings.hud_background_color);
    }
    settings
}

fn parse_hud_position(raw: &str) -> Option<HudPosition> {
    let normalized = raw.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "top" => Some(HudPosition::Top),
        "center" => Some(HudPosition::Center),
        "bottom" => Some(HudPosition::Bottom),
        _ => None,
    }
}

fn parse_hud_position_setting(raw: &str, default: HudPosition) -> HudPosition {
    parse_hud_position(raw).unwrap_or(default)
}

fn parse_hud_background_color(raw: &str) -> Option<HudBackgroundColor> {
    let normalized = raw.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "default" => Some(HudBackgroundColor::Default),
        "yellow" => Some(HudBackgroundColor::Yellow),
        "blue" => Some(HudBackgroundColor::Blue),
        "green" => Some(HudBackgroundColor::Green),
        "red" => Some(HudBackgroundColor::Red),
        "purple" => Some(HudBackgroundColor::Purple),
        _ => None,
    }
}

fn parse_hud_background_color_setting(
    raw: &str,
    default: HudBackgroundColor,
) -> HudBackgroundColor {
    parse_hud_background_color(raw).unwrap_or(default)
}

fn read_env_option(name: &str) -> Option<String> {
    let Ok(raw) = std::env::var(name) else {
        return None;
    };
    Some(raw.trim().to_string())
}

fn parse_f64_value(value: f64, default: f64, min: f64, max: f64) -> f64 {
    if !value.is_finite() {
        return default;
    }
    value.clamp(min, max)
}

fn parse_usize_value(value: usize, min: usize, max: usize) -> usize {
    value.clamp(min, max)
}

fn config_file_path() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("CLIIP_SHOW_CONFIG_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    let home =
        std::env::var("HOME").map_err(|_| "failed to resolve HOME for config path".to_string())?;
    let trimmed = home.trim();
    if trimmed.is_empty() {
        return Err("failed to resolve HOME for config path".to_string());
    }
    Ok(PathBuf::from(trimmed).join(DEFAULT_CONFIG_RELATIVE_PATH))
}

fn load_config_file(path: &Path) -> Result<(AppConfigFile, bool), String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok((AppConfigFile::default(), false));
        }
        Err(err) => {
            return Err(format!(
                "failed to read config file {}: {err}",
                path.display()
            ));
        }
    };
    toml::from_str::<AppConfigFile>(&content)
        .map(|config| (config, true))
        .map_err(|err| format!("failed to parse config file {}: {err}", path.display()))
}

fn save_config_file(path: &Path, config: &AppConfigFile) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| {
        format!(
            "failed to determine parent directory for config file {}",
            path.display()
        )
    })?;
    fs::create_dir_all(parent).map_err(|err| {
        format!(
            "failed to create config directory {}: {err}",
            parent.display()
        )
    })?;

    let content =
        toml::to_string_pretty(config).map_err(|err| format!("failed to encode config: {err}"))?;
    fs::write(path, content)
        .map_err(|err| format!("failed to write config file {}: {err}", path.display()))?;
    Ok(())
}

fn parse_config_key(raw: &str) -> Option<ConfigKey> {
    match raw {
        "poll_interval_secs" | "poll-interval-secs" => Some(ConfigKey::PollIntervalSecs),
        "hud_duration_secs" | "hud-duration-secs" => Some(ConfigKey::HudDurationSecs),
        "max_chars_per_line" | "max-chars-per-line" => Some(ConfigKey::MaxCharsPerLine),
        "max_lines" | "max-lines" => Some(ConfigKey::MaxLines),
        "hud_position" | "hud-position" => Some(ConfigKey::HudPosition),
        "hud_scale" | "hud-scale" => Some(ConfigKey::HudScale),
        "hud_background_color" | "hud-background-color" => Some(ConfigKey::HudBackgroundColor),
        _ => None,
    }
}

fn set_config_value(
    config: &mut AppConfigFile,
    key: ConfigKey,
    value: &str,
) -> Result<Option<String>, String> {
    match key {
        ConfigKey::PollIntervalSecs => {
            let raw = value.trim();
            let parsed = raw
                .parse::<f64>()
                .map_err(|_| format!("invalid f64 value for poll_interval_secs: {raw}"))?;
            if !parsed.is_finite() {
                return Err(format!(
                    "invalid finite f64 value for poll_interval_secs: {raw}"
                ));
            }
            let clamped = parsed.clamp(MIN_POLL_INTERVAL_SECS, MAX_POLL_INTERVAL_SECS);
            config.display.poll_interval_secs = Some(clamped);
            if parsed < MIN_POLL_INTERVAL_SECS || parsed > MAX_POLL_INTERVAL_SECS {
                return Ok(Some(format!(
                    "poll_interval_secs was clamped from {parsed} to {clamped} (allowed range: {MIN_POLL_INTERVAL_SECS}..={MAX_POLL_INTERVAL_SECS})"
                )));
            }
        }
        ConfigKey::HudDurationSecs => {
            let raw = value.trim();
            let parsed = raw
                .parse::<f64>()
                .map_err(|_| format!("invalid f64 value for hud_duration_secs: {raw}"))?;
            if !parsed.is_finite() {
                return Err(format!(
                    "invalid finite f64 value for hud_duration_secs: {raw}"
                ));
            }
            let clamped = parsed.clamp(MIN_HUD_DURATION_SECS, MAX_HUD_DURATION_SECS);
            config.display.hud_duration_secs = Some(clamped);
            if parsed < MIN_HUD_DURATION_SECS || parsed > MAX_HUD_DURATION_SECS {
                return Ok(Some(format!(
                    "hud_duration_secs was clamped from {parsed} to {clamped} (allowed range: {MIN_HUD_DURATION_SECS}..={MAX_HUD_DURATION_SECS})"
                )));
            }
        }
        ConfigKey::MaxCharsPerLine => {
            let raw = value.trim();
            let parsed = raw
                .parse::<usize>()
                .map_err(|_| format!("invalid usize value for max_chars_per_line: {raw}"))?;
            let clamped = parse_usize_value(parsed, MIN_TRUNCATE_MAX_WIDTH, MAX_TRUNCATE_MAX_WIDTH);
            config.display.max_chars_per_line = Some(clamped);
            if parsed < MIN_TRUNCATE_MAX_WIDTH || parsed > MAX_TRUNCATE_MAX_WIDTH {
                return Ok(Some(format!(
                    "max_chars_per_line was clamped from {parsed} to {clamped} (allowed range: {MIN_TRUNCATE_MAX_WIDTH}..={MAX_TRUNCATE_MAX_WIDTH})"
                )));
            }
        }
        ConfigKey::MaxLines => {
            let raw = value.trim();
            let parsed = raw
                .parse::<usize>()
                .map_err(|_| format!("invalid usize value for max_lines: {raw}"))?;
            let clamped = parse_usize_value(parsed, MIN_TRUNCATE_MAX_LINES, MAX_TRUNCATE_MAX_LINES);
            config.display.max_lines = Some(clamped);
            if parsed < MIN_TRUNCATE_MAX_LINES || parsed > MAX_TRUNCATE_MAX_LINES {
                return Ok(Some(format!(
                    "max_lines was clamped from {parsed} to {clamped} (allowed range: {MIN_TRUNCATE_MAX_LINES}..={MAX_TRUNCATE_MAX_LINES})"
                )));
            }
        }
        ConfigKey::HudPosition => {
            let raw = value.trim();
            let parsed = parse_hud_position(raw).ok_or_else(|| {
                format!("invalid hud_position value: {raw} (allowed: top, center, bottom)")
            })?;
            config.display.hud_position = Some(parsed);
        }
        ConfigKey::HudScale => {
            let raw = value.trim();
            let parsed = raw
                .parse::<f64>()
                .map_err(|_| format!("invalid f64 value for hud_scale: {raw}"))?;
            if !parsed.is_finite() {
                return Err(format!("invalid finite f64 value for hud_scale: {raw}"));
            }
            let clamped = parsed.clamp(MIN_HUD_SCALE, MAX_HUD_SCALE);
            config.display.hud_scale = Some(clamped);
            if parsed < MIN_HUD_SCALE || parsed > MAX_HUD_SCALE {
                return Ok(Some(format!(
                    "hud_scale was clamped from {parsed} to {clamped} (allowed range: {MIN_HUD_SCALE}..={MAX_HUD_SCALE})"
                )));
            }
        }
        ConfigKey::HudBackgroundColor => {
            let raw = value.trim();
            let parsed = parse_hud_background_color(raw).ok_or_else(|| {
                format!(
                    "invalid hud_background_color value: {raw} (allowed: default, yellow, blue, green, red, purple)"
                )
            })?;
            config.display.hud_background_color = Some(parsed);
        }
    }
    Ok(None)
}

fn print_effective_settings(settings: DisplaySettings) {
    println!("poll_interval_secs = {}", settings.poll_interval_secs);
    println!("hud_duration_secs = {}", settings.hud_duration_secs);
    println!("max_chars_per_line = {}", settings.truncate_max_width);
    println!("max_lines = {}", settings.truncate_max_lines);
    println!("hud_position = {}", settings.hud_position.as_str());
    println!("hud_scale = {}", settings.hud_scale);
    println!(
        "hud_background_color = {}",
        settings.hud_background_color.as_str()
    );
}

fn settings_to_config_file(settings: DisplaySettings) -> AppConfigFile {
    AppConfigFile {
        display: DisplayConfigFile {
            poll_interval_secs: Some(settings.poll_interval_secs),
            hud_duration_secs: Some(settings.hud_duration_secs),
            max_chars_per_line: Some(settings.truncate_max_width),
            max_lines: Some(settings.truncate_max_lines),
            hud_position: Some(settings.hud_position),
            hud_scale: Some(settings.hud_scale),
            hud_background_color: Some(settings.hud_background_color),
        },
    }
}

fn handle_config_command<I: Iterator<Item = String>>(args: &mut I) -> bool {
    let path = match config_file_path() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let Some(cmd) = args.next() else {
        eprintln!("Usage: cliip-show --config <path|show|init|set>");
        std::process::exit(2);
    };

    match cmd.as_str() {
        "path" => {
            if args.next().is_some() {
                eprintln!("Usage: cliip-show --config path");
                std::process::exit(2);
            }
            println!("{}", path.display());
            true
        }
        "show" => {
            if args.next().is_some() {
                eprintln!("Usage: cliip-show --config show");
                std::process::exit(2);
            }
            println!("config_path = {}", path.display());
            let (config, loaded_from_file) = match load_config_file(&path) {
                Ok(result) => result,
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            };
            if loaded_from_file {
                println!("config_file = exists");
                println!("[saved]");
                if let Some(value) = config.display.poll_interval_secs {
                    println!("poll_interval_secs = {}", value);
                }
                if let Some(value) = config.display.hud_duration_secs {
                    println!("hud_duration_secs = {}", value);
                }
                if let Some(value) = config.display.max_chars_per_line {
                    println!("max_chars_per_line = {}", value);
                }
                if let Some(value) = config.display.max_lines {
                    println!("max_lines = {}", value);
                }
                if let Some(value) = config.display.hud_position {
                    println!("hud_position = {}", value.as_str());
                }
                if let Some(value) = config.display.hud_scale {
                    println!("hud_scale = {}", value);
                }
                if let Some(value) = config.display.hud_background_color {
                    println!("hud_background_color = {}", value.as_str());
                }
            } else {
                println!("config_file = not_found");
            }
            println!("[effective]");
            let effective =
                apply_env_overrides(apply_config_file(default_display_settings(), &config));
            print_effective_settings(effective);
            true
        }
        "init" => {
            let mut force = false;
            if let Some(arg) = args.next() {
                if arg == "--force" {
                    force = true;
                    if args.next().is_some() {
                        eprintln!("Usage: cliip-show --config init [--force]");
                        std::process::exit(2);
                    }
                } else {
                    eprintln!("Usage: cliip-show --config init [--force]");
                    std::process::exit(2);
                }
            }

            if !force && path.exists() {
                eprintln!(
                    "config file already exists: {} (use --force to overwrite)",
                    path.display()
                );
                std::process::exit(2);
            }

            let config = settings_to_config_file(default_display_settings());
            if let Err(error) = save_config_file(&path, &config) {
                eprintln!("{error}");
                std::process::exit(1);
            }
            println!("initialized config: {}", path.display());
            true
        }
        "set" => {
            let Some(key_raw) = args.next() else {
                eprintln!("Usage: cliip-show --config set <key> <value>");
                eprintln!(
                    "Available keys: poll_interval_secs, hud_duration_secs, max_chars_per_line, max_lines, hud_position, hud_scale, hud_background_color"
                );
                std::process::exit(2);
            };
            let Some(value_raw) = args.next() else {
                eprintln!("Usage: cliip-show --config set <key> <value>");
                std::process::exit(2);
            };
            if args.next().is_some() {
                eprintln!("Usage: cliip-show --config set <key> <value>");
                std::process::exit(2);
            }
            let Some(key) = parse_config_key(key_raw.trim()) else {
                eprintln!(
                    "Unknown key: {key_raw}. Available keys: poll_interval_secs, hud_duration_secs, max_chars_per_line, max_lines, hud_position, hud_scale, hud_background_color"
                );
                std::process::exit(2);
            };

            let mut config = match load_config_file(&path) {
                Ok((config, _)) => config,
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            };

            let warning = match set_config_value(&mut config, key, value_raw.trim()) {
                Ok(warning) => warning,
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(2);
                }
            };
            if let Err(error) = save_config_file(&path, &config) {
                eprintln!("{error}");
                std::process::exit(1);
            }
            if let Some(warning) = warning {
                eprintln!("warning: {warning}");
            }
            println!("updated config: {}", path.display());
            println!("[effective]");
            let effective =
                apply_env_overrides(apply_config_file(default_display_settings(), &config));
            print_effective_settings(effective);
            true
        }
        unknown => {
            eprintln!("Unknown --config command: {unknown}");
            eprintln!("Usage: cliip-show --config <path|show|init|set>");
            std::process::exit(2);
        }
    }
}

fn parse_f64_setting(raw: &str, default: f64, min: f64, max: f64) -> f64 {
    let Ok(value) = raw.parse::<f64>() else {
        return default;
    };
    if !value.is_finite() {
        return default;
    }
    value.clamp(min, max)
}

fn parse_usize_setting(raw: &str, default: usize, min: usize, max: usize) -> usize {
    let Ok(value) = raw.parse::<usize>() else {
        return default;
    };
    value.clamp(min, max)
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
            let _ = writeln!(
                help,
                "  --config <path|show|init|set ...>    Manage persistent settings file"
            );
            let _ = writeln!(help);
            let _ = writeln!(help, "Config commands (persistent settings):");
            let _ = writeln!(help, "  cliip-show --config init");
            let _ = writeln!(help, "  cliip-show --config init --force");
            let _ = writeln!(help, "  cliip-show --config show");
            let _ = writeln!(help, "  cliip-show --config set hud_duration_secs 2.5");
            let _ = writeln!(help, "  cliip-show --config set max_lines 3");
            let _ = writeln!(help, "  cliip-show --config set hud_position top");
            let _ = writeln!(help, "  cliip-show --config set hud_scale 1.2");
            let _ = writeln!(help, "  cliip-show --config set hud_background_color blue");
            let _ = writeln!(help);
            let _ = writeln!(help, "Config keys:");
            let _ = writeln!(help, "  poll_interval_secs   default=0.3 (0.05 - 5.0)");
            let _ = writeln!(help, "  hud_duration_secs    default=1.0 (0.1 - 10.0)");
            let _ = writeln!(help, "  max_chars_per_line   default=100 (1 - 500)");
            let _ = writeln!(help, "  max_lines            default=5 (1 - 20)");
            let _ = writeln!(
                help,
                "  hud_position         default=center (top|center|bottom)"
            );
            let _ = writeln!(help, "  hud_scale            default=1.0 (0.5 - 2.0)");
            let _ = writeln!(
                help,
                "  hud_background_color default=default (default|yellow|blue|green|red|purple)"
            );
            let _ = writeln!(help);
            let _ = writeln!(help, "For Homebrew service:");
            let _ = writeln!(help, "  brew services restart cliip-show");
            let _ = writeln!(help);
            let _ = writeln!(help, "Persistent config file:");
            let _ = writeln!(
                help,
                "  default: ~/Library/Application Support/cliip-show/config.toml"
            );
            let _ = writeln!(help, "  override path via: CLIIP_SHOW_CONFIG_PATH");
            let _ = writeln!(help);
            let _ = writeln!(help, "Display settings via env vars (override file):");
            let _ = writeln!(
                help,
                "  CLIIP_SHOW_POLL_INTERVAL_SECS   Poll interval seconds (0.05 - 5.0)"
            );
            let _ = writeln!(
                help,
                "  CLIIP_SHOW_HUD_DURATION_SECS    HUD visible seconds (0.1 - 10.0)"
            );
            let _ = writeln!(
                help,
                "  CLIIP_SHOW_MAX_CHARS_PER_LINE   Max chars per line (1 - 500)"
            );
            let _ = writeln!(
                help,
                "  CLIIP_SHOW_MAX_LINES            Max lines in HUD (1 - 20)"
            );
            let _ = writeln!(
                help,
                "  CLIIP_SHOW_HUD_POSITION         HUD position (top|center|bottom)"
            );
            let _ = writeln!(
                help,
                "  CLIIP_SHOW_HUD_SCALE            HUD scale (0.5 - 2.0)"
            );
            let _ = writeln!(
                help,
                "  CLIIP_SHOW_HUD_BACKGROUND_COLOR HUD background color (default|yellow|blue|green|red|purple)"
            );
            print!("{help}");
            true
        }
        "--config" => handle_config_command(&mut args),
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
        let settings = display_settings();
        let (window, icon_label, label) = create_hud_window(settings);
        let truncated = truncate_text(
            text,
            settings.truncate_max_width,
            settings.truncate_max_lines,
        );
        let message = nsstring_from_str(&truncated);
        let () = msg_send![label, setStringValue: message];
        let () = msg_send![message, release];
        let hud_width = hud_width_for_text_with_scale(&truncated, settings.hud_scale);
        layout_hud(window, icon_label, label, hud_width, settings);

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
        let settings = display_settings();
        let pasteboard: *mut AnyObject = msg_send![class!(NSPasteboard), generalPasteboard];
        let last_change_count: isize = msg_send![pasteboard, changeCount];

        let (window, icon_label, label) = create_hud_window(settings);

        *APP_STATE.lock().expect("APP_STATE lock poisoned") = Some(AppState {
            last_change_count,
            pasteboard,
            window,
            icon_label,
            label,
            hide_timer: ptr::null_mut(),
            settings,
        });

        let _: *mut AnyObject = msg_send![
            class!(NSTimer),
            scheduledTimerWithTimeInterval: settings.poll_interval_secs
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

        let truncated = truncate_text(
            &text,
            state.settings.truncate_max_width,
            state.settings.truncate_max_lines,
        );
        let message = nsstring_from_str(&truncated);
        let () = msg_send![state.label, setStringValue: message];
        let () = msg_send![message, release];

        let hud_width = hud_width_for_text_with_scale(&truncated, state.settings.hud_scale);
        layout_hud(
            state.window,
            state.icon_label,
            state.label,
            hud_width,
            state.settings,
        );
        let () = msg_send![state.window, orderFrontRegardless];

        if !state.hide_timer.is_null() {
            let () = msg_send![state.hide_timer, invalidate];
        }

        let hide_timer: *mut AnyObject = msg_send![
            class!(NSTimer),
            scheduledTimerWithTimeInterval: state.settings.hud_duration_secs
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

fn hud_dimensions(scale: f64) -> HudDimensions {
    let clamped_scale = parse_f64_value(scale, DEFAULT_HUD_SCALE, MIN_HUD_SCALE, MAX_HUD_SCALE);
    HudDimensions {
        min_width: HUD_MIN_WIDTH * clamped_scale,
        max_width: HUD_MAX_WIDTH * clamped_scale,
        min_height: HUD_MIN_HEIGHT * clamped_scale,
        max_height: HUD_MAX_HEIGHT * clamped_scale,
        horizontal_padding: HUD_HORIZONTAL_PADDING * clamped_scale,
        vertical_padding: HUD_VERTICAL_PADDING * clamped_scale,
        icon_width: HUD_ICON_WIDTH * clamped_scale,
        icon_height: HUD_ICON_HEIGHT * clamped_scale,
        gap: HUD_GAP * clamped_scale,
        line_height_estimate: HUD_LINE_HEIGHT_ESTIMATE * clamped_scale,
        char_width_estimate: HUD_CHAR_WIDTH_ESTIMATE * clamped_scale,
    }
}

fn hud_background_rgba(color: HudBackgroundColor) -> (f64, f64, f64, f64) {
    match color {
        HudBackgroundColor::Default => (0.0, 0.0, 0.0, 0.78),
        HudBackgroundColor::Yellow => (0.43, 0.34, 0.04, 0.9),
        HudBackgroundColor::Blue => (0.08, 0.22, 0.53, 0.9),
        HudBackgroundColor::Green => (0.08, 0.35, 0.22, 0.9),
        HudBackgroundColor::Red => (0.47, 0.14, 0.14, 0.9),
        HudBackgroundColor::Purple => (0.36, 0.16, 0.47, 0.9),
    }
}

unsafe fn create_hud_window(
    settings: DisplaySettings,
) -> (*mut AnyObject, *mut AnyObject, *mut AnyObject) {
    let clamped_scale = parse_f64_value(
        settings.hud_scale,
        DEFAULT_HUD_SCALE,
        MIN_HUD_SCALE,
        MAX_HUD_SCALE,
    );
    let dims = hud_dimensions(clamped_scale);
    let default_width = (600.0 * clamped_scale).clamp(dims.min_width, dims.max_width);
    let default_height = dims.min_height;
    let mut rect = NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: NSSize {
            width: default_width,
            height: default_height,
        },
    };

    if let Some((x, y)) = hud_origin(
        default_width,
        default_height,
        settings.hud_position,
        clamped_scale,
    ) {
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
    let corner_radius = (HUD_CORNER_RADIUS * clamped_scale).clamp(8.0, 30.0);
    let () = msg_send![layer, setCornerRadius: corner_radius];
    let () = msg_send![layer, setMasksToBounds: true];

    let (bg_r, bg_g, bg_b, bg_a) = hud_background_rgba(settings.hud_background_color);
    let bg: *mut AnyObject = msg_send![
        class!(NSColor),
        colorWithCalibratedRed: bg_r
        green: bg_g
        blue: bg_b
        alpha: bg_a
    ];
    let cg_color: *mut c_void = msg_send![bg, CGColor];
    let () = msg_send![layer, setBackgroundColor: cg_color];
    let border_alpha = if settings.hud_background_color == HudBackgroundColor::Default {
        0.14
    } else {
        0.2
    };
    let border_color_obj: *mut AnyObject =
        msg_send![class!(NSColor), colorWithCalibratedWhite: 1.0f64 alpha: border_alpha];
    let border_color: *mut c_void = msg_send![border_color_obj, CGColor];
    let () = msg_send![layer, setBorderColor: border_color];
    let border_width = (HUD_BORDER_WIDTH * clamped_scale).clamp(1.0, 2.5);
    let () = msg_send![layer, setBorderWidth: border_width];

    let icon_rect = NSRect {
        origin: NSPoint {
            x: dims.horizontal_padding,
            y: ((default_height - dims.line_height_estimate) / 2.0 + dims.line_height_estimate
                - dims.icon_height)
                .max(dims.vertical_padding),
        },
        size: NSSize {
            width: dims.icon_width,
            height: dims.icon_height,
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
    let icon_font_size = (HUD_ICON_FONT_SIZE * clamped_scale).clamp(10.0, 44.0);
    let icon_font: *mut AnyObject = msg_send![class!(NSFont), systemFontOfSize: icon_font_size];
    let () = msg_send![icon_label, setFont: icon_font];
    let icon_text = nsstring_from_str("📋");
    let () = msg_send![icon_label, setStringValue: icon_text];
    let () = msg_send![icon_text, release];

    let label_rect = NSRect {
        origin: NSPoint {
            x: dims.horizontal_padding + dims.icon_width + dims.gap,
            y: (default_height - dims.line_height_estimate) / 2.0,
        },
        size: NSSize {
            width: default_width - (dims.horizontal_padding * 2.0 + dims.icon_width + dims.gap),
            height: dims.line_height_estimate,
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
    let text_font_size = (HUD_TEXT_FONT_SIZE * clamped_scale).clamp(10.0, 44.0);
    let font: *mut AnyObject =
        msg_send![class!(NSFont), fontWithName: menlo_name size: text_font_size];
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

unsafe fn main_screen_visible_frame() -> Option<NSRect> {
    let screen: *mut AnyObject = msg_send![class!(NSScreen), mainScreen];
    if screen.is_null() {
        return None;
    }

    let frame: NSRect = msg_send![screen, visibleFrame];
    Some(frame)
}

fn hud_origin_for_frame(
    frame: NSRect,
    width: f64,
    height: f64,
    position: HudPosition,
    scale: f64,
) -> (f64, f64) {
    let min_x = frame.origin.x;
    let max_x = frame.origin.x + (frame.size.width - width).max(0.0);
    let min_y = frame.origin.y;
    let max_y = frame.origin.y + (frame.size.height - height).max(0.0);

    let x = frame.origin.x + (frame.size.width - width) / 2.0;
    let margin = (HUD_SCREEN_MARGIN
        * parse_f64_value(scale, DEFAULT_HUD_SCALE, MIN_HUD_SCALE, MAX_HUD_SCALE))
    .clamp(12.0, 80.0);
    let y = match position {
        HudPosition::Top => max_y - margin,
        HudPosition::Center => frame.origin.y + (frame.size.height - height) / 2.0,
        HudPosition::Bottom => min_y + margin,
    };
    let x = x.clamp(min_x, max_x);
    let y = y.clamp(min_y, max_y);
    (x, y)
}

unsafe fn hud_origin(
    width: f64,
    height: f64,
    position: HudPosition,
    scale: f64,
) -> Option<(f64, f64)> {
    let frame = main_screen_visible_frame()?;
    Some(hud_origin_for_frame(frame, width, height, position, scale))
}

unsafe fn position_window(
    window: *mut AnyObject,
    width: f64,
    height: f64,
    position: HudPosition,
    scale: f64,
) {
    let (x, y) = hud_origin(width, height, position, scale).unwrap_or((0.0, 0.0));

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
    settings: DisplaySettings,
) {
    let dims = hud_dimensions(settings.hud_scale);
    let clamped_width = width.clamp(dims.min_width, dims.max_width);
    let text_width = clamped_width - (dims.horizontal_padding * 2.0 + dims.icon_width + dims.gap);
    let measured_text_height = measure_text_height(label, text_width, settings.hud_scale);
    let metrics = compute_hud_layout_metrics_with_scale(
        clamped_width,
        measured_text_height,
        settings.hud_scale,
    );

    let icon_rect = NSRect {
        origin: NSPoint {
            x: dims.horizontal_padding,
            y: metrics.icon_y,
        },
        size: NSSize {
            width: dims.icon_width,
            height: dims.icon_height,
        },
    };
    let label_rect = NSRect {
        origin: NSPoint {
            x: dims.horizontal_padding + dims.icon_width + dims.gap,
            y: metrics.label_y,
        },
        size: NSSize {
            width: metrics.text_width,
            height: metrics.text_height,
        },
    };

    let () = msg_send![icon_label, setFrame: icon_rect];
    let () = msg_send![label, setFrame: label_rect];
    position_window(
        window,
        metrics.width,
        metrics.height,
        settings.hud_position,
        settings.hud_scale,
    );
}

unsafe fn measure_text_height(label: *mut AnyObject, text_width: f64, scale: f64) -> f64 {
    let dims = hud_dimensions(scale);
    let cell: *mut AnyObject = msg_send![label, cell];
    if cell.is_null() {
        return dims.line_height_estimate;
    }

    let bounds = NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: NSSize {
            width: text_width.max(1.0),
            height: HUD_TEXT_MEASURE_HEIGHT,
        },
    };
    let size: NSSize = msg_send![cell, cellSizeForBounds: bounds];
    size.height.ceil().max(dims.line_height_estimate)
}

#[cfg(test)]
fn compute_hud_layout_metrics(width: f64, measured_text_height: f64) -> HudLayoutMetrics {
    compute_hud_layout_metrics_with_scale(width, measured_text_height, DEFAULT_HUD_SCALE)
}

fn compute_hud_layout_metrics_with_scale(
    width: f64,
    measured_text_height: f64,
    scale: f64,
) -> HudLayoutMetrics {
    let dims = hud_dimensions(scale);
    let width = width.clamp(dims.min_width, dims.max_width);
    let text_width = width - (dims.horizontal_padding * 2.0 + dims.icon_width + dims.gap);
    let measured_text_height = measured_text_height
        .min((dims.max_height - dims.vertical_padding * 2.0).max(dims.line_height_estimate));
    let height = (measured_text_height + dims.vertical_padding * 2.0)
        .clamp(dims.min_height, dims.max_height);
    let text_height = (height - dims.vertical_padding * 2.0)
        .min(measured_text_height)
        .max(dims.line_height_estimate);
    let label_y = (height - text_height) / 2.0;
    let icon_y = (label_y + text_height - dims.icon_height)
        .max(dims.vertical_padding)
        .min(height - dims.icon_height - dims.vertical_padding);

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

#[cfg(test)]
fn hud_width_for_text(text: &str) -> f64 {
    hud_width_for_text_with_scale(text, DEFAULT_HUD_SCALE)
}

fn hud_width_for_text_with_scale(text: &str, scale: f64) -> f64 {
    let dims = hud_dimensions(scale);
    let lines = split_non_trailing_lines(text);
    let max_units = lines
        .iter()
        .map(|line| line_display_units(line))
        .fold(1.0f64, f64::max);

    (max_units * dims.char_width_estimate
        + dims.horizontal_padding * 2.0
        + dims.icon_width
        + dims.gap)
        .clamp(dims.min_width, dims.max_width)
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
    use super::{
        compute_hud_layout_metrics, hud_origin_for_frame, hud_width_for_text, parse_config_key,
        parse_f64_setting, parse_usize_setting, set_config_value, truncate_text, AppConfigFile,
        ConfigKey, HudBackgroundColor, HudPosition, NSPoint, NSRect, NSSize,
    };

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

    #[test]
    fn parse_f64_setting_clamps_and_fallbacks() {
        assert_eq!(parse_f64_setting("0.01", 1.0, 0.1, 5.0), 0.1);
        assert_eq!(parse_f64_setting("8.0", 1.0, 0.1, 5.0), 5.0);
        assert_eq!(parse_f64_setting("1.5", 1.0, 0.1, 5.0), 1.5);
        assert_eq!(parse_f64_setting("abc", 1.0, 0.1, 5.0), 1.0);
    }

    #[test]
    fn parse_usize_setting_clamps_and_fallbacks() {
        assert_eq!(parse_usize_setting("0", 10, 1, 20), 1);
        assert_eq!(parse_usize_setting("100", 10, 1, 20), 20);
        assert_eq!(parse_usize_setting("5", 10, 1, 20), 5);
        assert_eq!(parse_usize_setting("abc", 10, 1, 20), 10);
    }

    #[test]
    fn parse_config_key_accepts_aliases() {
        assert_eq!(
            parse_config_key("poll_interval_secs"),
            Some(ConfigKey::PollIntervalSecs)
        );
        assert_eq!(
            parse_config_key("poll-interval-secs"),
            Some(ConfigKey::PollIntervalSecs)
        );
        assert_eq!(
            parse_config_key("hud_position"),
            Some(ConfigKey::HudPosition)
        );
        assert_eq!(parse_config_key("hud-scale"), Some(ConfigKey::HudScale));
        assert_eq!(parse_config_key("hub_background_color"), None);
        assert_eq!(parse_config_key("hub-background-color"), None);
        assert_eq!(parse_config_key("unknown"), None);
    }

    #[test]
    fn hud_origin_for_frame_positions_by_setting() {
        let frame = NSRect {
            origin: NSPoint { x: 0.0, y: 0.0 },
            size: NSSize {
                width: 1000.0,
                height: 800.0,
            },
        };

        let (top_x, top_y) = hud_origin_for_frame(frame, 600.0, 100.0, HudPosition::Top, 1.0);
        let (center_x, center_y) =
            hud_origin_for_frame(frame, 600.0, 100.0, HudPosition::Center, 1.0);
        let (bottom_x, bottom_y) =
            hud_origin_for_frame(frame, 600.0, 100.0, HudPosition::Bottom, 1.0);

        assert_eq!(top_x, 200.0);
        assert_eq!(center_x, 200.0);
        assert_eq!(bottom_x, 200.0);
        assert_eq!(top_y, 676.0);
        assert_eq!(center_y, 350.0);
        assert_eq!(bottom_y, 24.0);
    }

    #[test]
    fn set_config_value_clamps_values() {
        let mut config = AppConfigFile::default();
        let poll_warning = set_config_value(&mut config, ConfigKey::PollIntervalSecs, "0.01")
            .expect("set poll interval");
        let lines_warning =
            set_config_value(&mut config, ConfigKey::MaxLines, "999").expect("set max lines");

        assert_eq!(config.display.poll_interval_secs, Some(0.05));
        assert_eq!(config.display.max_lines, Some(20));
        assert!(poll_warning.is_some());
        assert!(lines_warning.is_some());
    }

    #[test]
    fn set_config_value_accepts_new_display_options() {
        let mut config = AppConfigFile::default();
        let position_warning =
            set_config_value(&mut config, ConfigKey::HudPosition, "bottom").expect("set position");
        let scale_warning =
            set_config_value(&mut config, ConfigKey::HudScale, "9.9").expect("set scale");
        let color_warning = set_config_value(&mut config, ConfigKey::HudBackgroundColor, "blue")
            .expect("set background color");

        assert_eq!(config.display.hud_position, Some(HudPosition::Bottom));
        assert_eq!(config.display.hud_scale, Some(2.0));
        assert_eq!(
            config.display.hud_background_color,
            Some(HudBackgroundColor::Blue)
        );
        assert!(position_warning.is_none());
        assert!(scale_warning.is_some());
        assert!(color_warning.is_none());
    }

    #[test]
    fn set_config_value_rejects_non_finite_f64_values() {
        let mut config = AppConfigFile::default();
        let poll_err = set_config_value(&mut config, ConfigKey::PollIntervalSecs, "NaN")
            .expect_err("reject NaN");
        let duration_err = set_config_value(&mut config, ConfigKey::HudDurationSecs, "inf")
            .expect_err("reject inf");

        assert!(poll_err.contains("invalid finite f64 value for poll_interval_secs"));
        assert!(duration_err.contains("invalid finite f64 value for hud_duration_secs"));
        assert_eq!(config.display.poll_interval_secs, None);
        assert_eq!(config.display.hud_duration_secs, None);
    }

    #[test]
    fn set_config_value_rejects_invalid_enum_values() {
        let mut config = AppConfigFile::default();
        let position_err = set_config_value(&mut config, ConfigKey::HudPosition, "middle")
            .expect_err("reject invalid position");
        let color_err = set_config_value(&mut config, ConfigKey::HudBackgroundColor, "orange")
            .expect_err("reject invalid color");

        assert!(position_err.contains("invalid hud_position value"));
        assert!(color_err.contains("invalid hud_background_color value"));
        assert_eq!(config.display.hud_position, None);
        assert_eq!(config.display.hud_background_color, None);
    }
}
