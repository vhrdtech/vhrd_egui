//! The shaders parse, validate and translate to GLSL ES 3.0 for WebGL2, entry point by entry point.

use naga::back::glsl;
use naga::valid::{Capabilities, ValidationFlags, Validator};

const SHAPES: &str = include_str!("../../src/shaders/shapes.wgsl");
const COLUMNS: &str = include_str!("../../src/shaders/columns.wgsl");

fn webgl2(label: &str, body: &str) {
    let source = format!("{}\n{body}", ve_gpu::COMMON_WGSL);
    let module = naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|e| panic!("{label}: {}", e.emit_to_string(&source)));
    // WebGL2 has none of the optional capabilities
    let info = Validator::new(ValidationFlags::all(), Capabilities::empty())
        .validate(&module)
        .unwrap_or_else(|e| panic!("{label}: {e:?}"));
    assert!(!module.entry_points.is_empty(), "{label}: no entry points");
    for ep in &module.entry_points {
        let options = glsl::Options {
            version: glsl::Version::Embedded {
                version: 300,
                is_webgl: true,
            },
            ..glsl::Options::default()
        };
        let pipeline = glsl::PipelineOptions {
            shader_stage: ep.stage,
            entry_point: ep.name.clone(),
            multiview: None,
        };
        let mut out = String::new();
        let mut writer = glsl::Writer::new(
            &mut out,
            &module,
            &info,
            &options,
            &pipeline,
            naga::proc::BoundsCheckPolicies::default(),
        )
        .unwrap_or_else(|e| panic!("{label} {}: {e:?}", ep.name));
        writer
            .write()
            .unwrap_or_else(|e| panic!("{label} {}: {e:?}", ep.name));
        assert!(out.contains("#version 300 es"), "{label} {}", ep.name);
    }
}

#[test]
fn the_shape_shader_translates_to_webgl2() {
    webgl2("shapes", SHAPES);
}

#[test]
fn the_column_shader_translates_to_webgl2() {
    webgl2("columns", COLUMNS);
}

#[test]
fn the_shaders_use_no_storage_buffers() {
    for (label, s) in [
        ("common", ve_gpu::COMMON_WGSL),
        ("shapes", SHAPES),
        ("columns", COLUMNS),
    ] {
        assert!(
            !s.contains("var<storage"),
            "{label} binds a storage buffer, which WebGL2 lacks"
        );
    }
}
