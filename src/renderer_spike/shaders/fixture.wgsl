struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) local: vec2<f32>,
    @location(3) aperture: vec2<f32>,
    @location(4) kind: f32,
};

@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;

struct AtlasOverrides {
    rects: array<vec4<f32>, 16>,
};

@group(0) @binding(2) var<storage, read> atlas_overrides: AtlasOverrides;

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) base_rect: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) base_atlas_rect: vec4<f32>,
    @location(3) params: vec4<f32>,
    @location(4) offset: vec2<f32>,
) -> VertexOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    let local = corners[vertex_index];
    let logical = base_rect.xy + offset + local * base_rect.zw;
    let ndc = vec2<f32>(
        logical.x / 360.0 * 2.0 - 1.0,
        1.0 - logical.y / 360.0 * 2.0,
    );
    var atlas_rect = base_atlas_rect;
    if (params.z >= 0.0) {
        atlas_rect = atlas_overrides.rects[u32(params.z)];
    }
    var out: VertexOut;
    out.position = vec4<f32>(ndc, params.y, 1.0);
    out.color = color;
    out.uv = mix(atlas_rect.xy, atlas_rect.zw, local);
    out.local = local;
    out.aperture = vec2<f32>(ndc.x, -ndc.y);
    out.kind = params.x;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    if (length(in.aperture) > 1.0) {
        discard;
    }
    var alpha = 1.0;
    if (in.kind < 0.5) {
        alpha = textureSample(atlas, atlas_sampler, in.uv).a;
    } else if (in.kind > 1.5 && in.kind < 2.5) {
        let centered = in.local * 2.0 - vec2<f32>(1.0, 1.0);
        if (dot(centered, centered) > 1.0) {
            discard;
        }
    } else if (in.kind >= 2.5) {
        let centered = in.local * 2.0 - vec2<f32>(1.0, 1.0);
        let radius = length(centered);
        let angle = atan2(centered.y, centered.x);
        if (radius < 0.62 || radius > 1.0 || (angle > -0.35 && angle < 0.35)) {
            discard;
        }
    }
    let output = vec4<f32>(in.color.rgb, in.color.a * alpha);
    if (output.a <= 0.001) {
        discard;
    }
    return output;
}
