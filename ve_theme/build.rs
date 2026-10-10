//! Turns the vhrd_brand export (`brand/palette.json`, a copy of vhrd_brand `web/palette.json`: `just brand`)
//! into `Color32` constants, one module per palette group (THM-11). No hex is typed by hand for a brand colour.

use std::{env, fmt::Write, fs, path::Path};

use serde_json::Value;

/// `signal-dark` -> `signal_dark`; a grey step `50` -> `s50`.
fn ident(key: &str) -> String {
    let id = key.replace('-', "_");
    if id.starts_with(|c: char| c.is_ascii_digit()) {
        format!("s{id}")
    } else {
        id
    }
}

fn emit(out: &mut String, name: &str, group: &Value) {
    let Value::Object(map) = group else { return };
    let _ = writeln!(out, "pub mod {} {{", ident(name));
    for (key, v) in map {
        match v {
            Value::Object(_) => emit(out, key, v),
            Value::String(s) => {
                let hex = s.strip_prefix('#').filter(|h| h.len() == 6);
                if let Some(n) = hex.and_then(|h| u32::from_str_radix(h, 16).ok()) {
                    let _ = writeln!(
                        out,
                        "    /// `{s}`\n    pub const {}: egui::Color32 = egui::Color32::from_rgb({}, {}, {});",
                        ident(key).to_uppercase(),
                        n >> 16,
                        (n >> 8) & 0xff,
                        n & 0xff
                    );
                }
            }
            _ => {}
        }
    }
    out.push_str("}\n");
}

fn main() {
    println!("cargo:rerun-if-changed=brand/palette.json");
    let json = fs::read_to_string("brand/palette.json").expect("ve_theme/brand/palette.json");
    let palette: Value = serde_json::from_str(&json).expect("brand/palette.json is JSON");
    let mut out = String::new();
    if let Value::Object(groups) = &palette {
        for (name, group) in groups {
            emit(&mut out, name, group);
        }
    }
    let dir = env::var("OUT_DIR").expect("OUT_DIR");
    fs::write(Path::new(&dir).join("brand.rs"), out).expect("write brand.rs");
}
