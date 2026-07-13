// Retained companion scene ABI. Rust-side size/offset tests lock the matching
// records before any GPU objects consume this source.

const NONE_U32: u32 = 0xffffffffu;
const GLYPH_FLAG_VISIBLE: u32 = 1u;
const GLYPH_FLAG_COLOR: u32 = 2u;

struct PrimitiveGpuValue {
    node_index: u32,
    material_index: u32,
    aux_node_index: u32,
    primitive_kind: u32,
    material_kind: u32,
    resource_kind: u32,
    blend: u32,
    depth: u32,
    space: u32,
    instance_group: u32,
    instance_base: u32,
    binding_index: u32,
    authored_order: u32,
    content_base: u32,
    frame_base: u32,
    aux_content_base: u32,
}

struct SceneContentGpuValue {
    kind: u32,
    glyph_entry_index: u32,
    slot: u32,
    subslot: u32,
    signed_data: vec2<i32>,
    flags: u32,
    variant: u32,
}

struct GlyphAtlasGpuEntry {
    visible_uv: vec4<f32>,
    ink_origin_size: vec4<f32>,
    metrics: vec3<f32>,
    flags: u32,
    allocated_cell: vec4<u32>,
}

struct NodeGpuValue {
    world: mat4x4<f32>,
    opacity: f32,
    visible: u32,
    material_parameter_offset: u32,
    material_parameter_count: u32,
    // scale, y offset in points-up coordinates, opacity, saturation
    depth_cue: vec4<f32>,
}

struct ContentGlobalsGpuValue {
    palette_rgba: array<vec4<u32>, 8>,
    mood: u32,
    weather: u32,
    glyph_grid_dimensions: vec2<u32>,
    glyph_grid_origin_points: vec2<f32>,
    glyph_cell_extent_points: vec2<f32>,
}

struct FrameGlobalsGpuValue {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    viewport_points: vec2<f32>,
    viewport_pixels: vec2<f32>,
    aperture: vec4<f32>,
    gauges: vec4<f32>,
    dim_amount: f32,
    light_count: u32,
    padding: vec2<u32>,
}

struct FrameGpuValue {
    kind: u32,
    slot: u32,
    flags: u32,
    variant: u32,
    values: array<f32, 8>,
}

struct AnalyticFrameGpuValue {
    id: u32,
    semantic: u32,
    shape: u32,
    flags: u32,
    rect_points: vec4<f32>,
    payload: array<vec4<f32>, 4>,
}

struct AnalyticContentGpuValue {
    id: u32,
    semantic: u32,
    shape: u32,
    flags: u32,
    payload: array<vec4<u32>, 2>,
}

struct NodeBuffer {
    values: array<NodeGpuValue>,
}

struct ContentGlobalsBuffer {
    globals: ContentGlobalsGpuValue,
}

struct FrameBuffer {
    globals: FrameGlobalsGpuValue,
    values: array<FrameGpuValue, 124>,
    analytics: array<AnalyticFrameGpuValue, 16>,
}

struct PrimitiveBuffer {
    values: array<PrimitiveGpuValue>,
}

struct SceneContentBuffer {
    values: array<SceneContentGpuValue, 462>,
    analytics: array<AnalyticContentGpuValue, 16>,
}

struct GlyphEntryBuffer {
    values: array<GlyphAtlasGpuEntry>,
}

struct HudGlyphGpuValue {
    rect_points: vec4<f32>,
    glyph_entry_index: u32,
    role: u32,
    visible: u32,
    padding: u32,
}

struct HudGlyphBuffer {
    values: array<HudGlyphGpuValue>,
}

@group(0) @binding(0) var<storage, read> node_buffer: NodeBuffer;
@group(0) @binding(1) var<storage, read> content_globals_buffer: ContentGlobalsBuffer;
@group(0) @binding(2) var<storage, read> frame_buffer: FrameBuffer;
@group(0) @binding(3) var<storage, read> primitive_buffer: PrimitiveBuffer;
@group(0) @binding(4) var<storage, read> scene_content_buffer: SceneContentBuffer;
@group(0) @binding(5) var<storage, read> glyph_entry_buffer: GlyphEntryBuffer;

@group(1) @binding(0) var coverage_texture: texture_2d<f32>;
@group(1) @binding(1) var color_texture: texture_2d<f32>;
@group(1) @binding(2) var atlas_sampler: sampler;

@group(2) @binding(0) var intermediate_texture: texture_2d<f32>;

@group(3) @binding(0) var<storage, read> hud_glyph_buffer: HudGlyphBuffer;

struct SceneVertexInput {
    @location(0) local_position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) primitive_index: u32,
    @location(4) material_index: u32,
    @builtin(instance_index) instance_index: u32,
}

struct SceneVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) content_index: u32,
    @location(2) opacity: f32,
    @location(3) saturation: f32,
    @location(4) @interpolate(flat) instance_group: u32,
}

fn content_index_for_primitive(
    primitive: PrimitiveGpuValue,
    instance_index: u32,
) -> u32 {
    if (primitive.content_base == NONE_U32) {
        return NONE_U32;
    }
    return primitive.content_base + instance_index;
}

fn apply_node_depth_cue(
    input: SceneVertexInput,
    node: NodeGpuValue,
) -> vec4<f32> {
    let local = vec3<f32>(
        input.local_position.x * node.depth_cue.x,
        input.local_position.y * node.depth_cue.x,
        input.local_position.z,
    );
    var world_position = node.world * vec4<f32>(local, 1.0);
    world_position.y = world_position.y + node.depth_cue.y;
    return world_position;
}

fn scene_vertex_output(
    input: SceneVertexInput,
    position: vec4<f32>,
    primitive: PrimitiveGpuValue,
    node: NodeGpuValue,
) -> SceneVertexOutput {
    var output: SceneVertexOutput;
    output.position = position;
    output.uv = input.uv;
    output.content_index = content_index_for_primitive(primitive, input.instance_index);
    output.opacity = node.opacity * f32(node.visible);
    output.saturation = node.depth_cue.w;
    output.instance_group = primitive.instance_group;
    return output;
}

@vertex
fn vs_world(input: SceneVertexInput) -> SceneVertexOutput {
    let primitive = primitive_buffer.values[input.primitive_index];
    let node = node_buffer.values[primitive.node_index];
    let world_position = apply_node_depth_cue(input, node);
    let position = frame_buffer.globals.projection * frame_buffer.globals.view * world_position;
    return scene_vertex_output(input, position, primitive, node);
}

@vertex
fn vs_screen(input: SceneVertexInput) -> SceneVertexOutput {
    let primitive = primitive_buffer.values[input.primitive_index];
    let node = node_buffer.values[primitive.node_index];
    let point_position = apply_node_depth_cue(input, node);
    let normalized = vec2<f32>(
        point_position.x * 2.0 / frame_buffer.globals.viewport_points.x - 1.0,
        point_position.y * 2.0 / frame_buffer.globals.viewport_points.y - 1.0,
    );
    let position = vec4<f32>(normalized, point_position.z, 1.0);
    return scene_vertex_output(input, position, primitive, node);
}

fn srgb_channel_to_linear(value: f32) -> f32 {
    if (value <= 0.04045) {
        return value / 12.92;
    }
    return pow((value + 0.055) / 1.055, 2.4);
}

fn srgb_to_linear(value: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        srgb_channel_to_linear(value.r),
        srgb_channel_to_linear(value.g),
        srgb_channel_to_linear(value.b),
    );
}

fn apply_saturation(color: vec3<f32>, saturation: f32) -> vec3<f32> {
    let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    return mix(vec3<f32>(luminance), color, max(saturation, 0.0));
}

fn palette_linear(input: SceneVertexOutput) -> vec4<f32> {
    var palette_index = 0u;
    if (input.content_index != NONE_U32 &&
        (input.instance_group == 1u || input.instance_group == 2u)) {
        let role = scene_content_buffer.values[input.content_index].flags;
        if (role >= 1u && role <= 8u) {
            palette_index = role - 1u;
        }
    }
    let packed = content_globals_buffer.globals.palette_rgba[palette_index];
    let straight_srgb = vec4<f32>(packed) / 255.0;
    return vec4<f32>(srgb_to_linear(straight_srgb.rgb), straight_srgb.a);
}

fn tank_paint_linear(content: SceneContentGpuValue) -> vec4<f32> {
    let packed = u32(content.signed_data.x);
    let straight_srgb = vec3<f32>(
        f32(packed & 0xffu),
        f32((packed >> 8u) & 0xffu),
        f32((packed >> 16u) & 0xffu),
    ) / 255.0;
    return vec4<f32>(srgb_to_linear(straight_srgb), 1.0);
}

fn glyph_paint_linear(input: SceneVertexOutput) -> vec4<f32> {
    let content = scene_content_buffer.values[input.content_index];
    if (content.kind == 3u) {
        return tank_paint_linear(content);
    }
    return palette_linear(input);
}

fn premultiply_scene_color(
    straight: vec4<f32>,
    coverage: f32,
    opacity: f32,
    saturation: f32,
) -> vec4<f32> {
    let alpha = straight.a * coverage * opacity;
    let linear_rgb = apply_saturation(straight.rgb, saturation);
    return vec4<f32>(linear_rgb * alpha, alpha);
}

fn glyph_entry_for(input: SceneVertexOutput) -> GlyphAtlasGpuEntry {
    let content = scene_content_buffer.values[input.content_index];
    return glyph_entry_buffer.values[content.glyph_entry_index];
}

fn glyph_uv(input: SceneVertexOutput, entry: GlyphAtlasGpuEntry) -> vec2<f32> {
    let atlas_local = vec2<f32>(input.uv.x, 1.0 - input.uv.y);
    return mix(entry.visible_uv.xy, entry.visible_uv.zw, atlas_local);
}

@fragment
fn fs_analytic(input: SceneVertexOutput) -> @location(0) vec4<f32> {
    let centered = input.uv - vec2<f32>(0.5);
    let distance = length(centered);
    let edge_width = fwidth(distance);
    let coverage = 1.0 - smoothstep(0.5 - edge_width, 0.5 + edge_width, distance);
    let output = premultiply_scene_color(
        palette_linear(input),
        coverage,
        input.opacity,
        input.saturation,
    );
    if (output.a <= 0.0) {
        discard;
    }
    return output;
}

@fragment
fn fs_glyph(input: SceneVertexOutput) -> @location(0) vec4<f32> {
    if (input.content_index == NONE_U32) {
        discard;
    }
    let content = scene_content_buffer.values[input.content_index];
    if (content.glyph_entry_index == NONE_U32) {
        discard;
    }
    let entry = glyph_entry_for(input);
    if ((entry.flags & GLYPH_FLAG_VISIBLE) == 0u) {
        discard;
    }
    var output: vec4<f32>;
    if ((entry.flags & GLYPH_FLAG_COLOR) != 0u) {
        // Rgba8UnormSrgb sampling returns linear RGB and straight alpha.
        let straight_linear = textureSampleLevel(color_texture, atlas_sampler, glyph_uv(input, entry), 0.0);
        if (straight_linear.a <= 0.0) {
            discard;
        }
        output = premultiply_scene_color(
            straight_linear,
            1.0,
            input.opacity,
            input.saturation,
        );
    } else {
        let coverage = textureSampleLevel(coverage_texture, atlas_sampler, glyph_uv(input, entry), 0.0).r;
        if (coverage <= 0.0) {
            discard;
        }
        output = premultiply_scene_color(
            glyph_paint_linear(input),
            coverage,
            input.opacity,
            input.saturation,
        );
    }
    if (output.a <= 0.0) {
        discard;
    }
    return output;
}

struct HudVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) glyph_entry_index: u32,
    @location(2) @interpolate(flat) role: u32,
    @location(3) @interpolate(flat) visible: u32,
}

fn hud_quad_corner(vertex_index: u32) -> vec2<f32> {
    switch vertex_index {
        case 0u: { return vec2<f32>(0.0, 0.0); }
        case 1u: { return vec2<f32>(1.0, 0.0); }
        case 2u: { return vec2<f32>(0.0, 1.0); }
        case 3u: { return vec2<f32>(0.0, 1.0); }
        case 4u: { return vec2<f32>(1.0, 0.0); }
        default: { return vec2<f32>(1.0, 1.0); }
    }
}

@vertex
fn vs_hud(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> HudVertexOutput {
    let instance = hud_glyph_buffer.values[instance_index];
    var output: HudVertexOutput;
    output.uv = vec2<f32>(0.0);
    output.glyph_entry_index = instance.glyph_entry_index;
    output.role = instance.role;
    output.visible = instance.visible;
    if (instance.visible == 0u || instance.rect_points.z <= 0.0 || instance.rect_points.w <= 0.0) {
        output.position = vec4<f32>(2.0, 2.0, 0.0, 1.0);
        return output;
    }

    let corner = hud_quad_corner(vertex_index);
    let point_position = instance.rect_points.xy + corner * instance.rect_points.zw;
    let normalized = vec2<f32>(
        point_position.x * 2.0 / frame_buffer.globals.viewport_points.x - 1.0,
        point_position.y * 2.0 / frame_buffer.globals.viewport_points.y - 1.0,
    );
    output.position = vec4<f32>(normalized, 0.0, 1.0);
    output.uv = corner;
    return output;
}

@fragment
fn fs_hud(input: HudVertexOutput) -> @location(0) vec4<f32> {
    if (input.visible == 0u || input.role > 2u) {
        discard;
    }
    if (input.glyph_entry_index >= arrayLength(&glyph_entry_buffer.values)) {
        discard;
    }
    let entry = glyph_entry_buffer.values[input.glyph_entry_index];
    if ((entry.flags & GLYPH_FLAG_VISIBLE) == 0u || (entry.flags & GLYPH_FLAG_COLOR) != 0u) {
        discard;
    }

    let atlas_local = vec2<f32>(input.uv.x, 1.0 - input.uv.y);
    let uv = mix(entry.visible_uv.xy, entry.visible_uv.zw, atlas_local);
    let coverage = textureSampleLevel(coverage_texture, atlas_sampler, uv, 0.0).r;
    if (coverage <= 0.0) {
        discard;
    }
    var straight_srgb = vec4<f32>(0.62, 0.63, 0.77, 1.0);
    if (input.role == 0u) {
        straight_srgb = vec4<f32>(0.93, 0.93, 0.97, 1.0);
    }
    let alpha = straight_srgb.a * coverage;
    let linear_rgb = srgb_to_linear(straight_srgb.rgb);
    return vec4<f32>(linear_rgb * alpha, alpha);
}

struct FinalVertexOutput {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs_final(@builtin(vertex_index) vertex_index: u32) -> FinalVertexOutput {
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index & 2u) * 2 - 1);
    var output: FinalVertexOutput;
    output.position = vec4<f32>(x, y, 0.0, 1.0);
    return output;
}

@fragment
fn fs_final(input: FinalVertexOutput) -> @location(0) vec4<f32> {
    // Sampling the sRGB intermediate decodes its stored premultiplied RGB back
    // to linear before the PostMultiplied surface receives straight RGB.
    let pixel = vec2<i32>(input.position.xy);
    let sampled = textureLoad(intermediate_texture, pixel, 0);
    if (sampled.a == 0.0) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(sampled.rgb / sampled.a, sampled.a);
}
