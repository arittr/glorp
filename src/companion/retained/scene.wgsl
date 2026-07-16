// Retained companion scene ABI. Rust-side size/offset tests lock the matching
// records before any GPU objects consume this source.

const NONE_U32: u32 = 0xffffffffu;
const GLYPH_FLAG_VISIBLE: u32 = 1u;
const GLYPH_FLAG_COLOR: u32 = 2u;
const PROP_FRAME_GPU_BASE: u32 = 0u;
const PROP_FRAME_GPU_STRIDE: u32 = 1u;
const PROP_FRAME_GPU_COUNT: u32 = 10u;
const FRAME_GPU_VALUE_COUNT: u32 = 124u;

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
    values: array<FrameGpuValue, FRAME_GPU_VALUE_COUNT>,
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

@group(2) @binding(0) var scene_sampled_texture: texture_2d<f32>;

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
    @location(5) @interpolate(flat) analytic_id: u32,
    @location(6) point_position: vec2<f32>,
    @location(7) local_coordinate: vec2<f32>,
}

struct GlyphInstancePlacement {
    world_position: vec4<f32>,
    content_index: u32,
    opacity: f32,
    saturation: f32,
    valid: u32,
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

fn apply_node_depth_cue_to_point(
    local_position: vec3<f32>,
    node: NodeGpuValue,
) -> vec4<f32> {
    let local = vec3<f32>(
        local_position.x * node.depth_cue.x,
        local_position.y * node.depth_cue.x,
        local_position.z,
    );
    var world_position = node.world * vec4<f32>(local, 1.0);
    world_position.y = world_position.y + node.depth_cue.y;
    return world_position;
}

fn invalid_glyph_placement() -> GlyphInstancePlacement {
    var placement: GlyphInstancePlacement;
    placement.world_position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    placement.content_index = NONE_U32;
    placement.opacity = 0.0;
    placement.saturation = 0.0;
    placement.valid = 0u;
    return placement;
}

fn checked_instance_content_index(base: u32, instance_index: u32) -> u32 {
    if (base == NONE_U32 || base >= 462u || instance_index > 461u - base) {
        return NONE_U32;
    }
    return base + instance_index;
}

fn metric_ink_offset(
    quad_corner: vec2<f32>,
    entry: GlyphAtlasGpuEntry,
) -> vec2<f32> {
    let cell_extent = content_globals_buffer.globals.glyph_cell_extent_points;
    // LogicalGlyphScale::OneCell deliberately fits each entry uniformly into
    // one authored cell. Wide/native-color glyphs become width-limited while
    // ordinary glyphs are normally height-limited; neither is distorted.
    let scale = min(
        cell_extent.x / entry.metrics.x,
        cell_extent.y / entry.metrics.y,
    );
    return entry.ink_origin_size.xy * scale
        + quad_corner * entry.ink_origin_size.zw * scale;
}

fn projected_metric_ink_offset(
    quad_corner: vec2<f32>,
    entry: GlyphAtlasGpuEntry,
    destination_cell_extent: vec2<f32>,
) -> vec2<f32> {
    let scale = destination_cell_extent / entry.metrics.xy;
    return entry.ink_origin_size.xy * scale
        + quad_corner * entry.ink_origin_size.zw * scale;
}

fn glyph_instance_placement(
    input: SceneVertexInput,
    primitive: PrimitiveGpuValue,
    node: NodeGpuValue,
) -> GlyphInstancePlacement {
    let is_wall = primitive.primitive_kind == 2u
        && primitive.instance_group == 0u
        && primitive.binding_index == 1u;
    let is_floor = primitive.primitive_kind == 2u
        && primitive.instance_group == 0u
        && primitive.binding_index == 2u;
    var content_index = NONE_U32;
    if (is_wall || is_floor) {
        if (input.instance_index >= 130u) {
            return invalid_glyph_placement();
        }
        content_index = checked_instance_content_index(
            primitive.aux_content_base,
            input.instance_index,
        );
    } else {
        content_index = checked_instance_content_index(
            primitive.content_base,
            input.instance_index,
        );
    }
    if (content_index >= 462u) {
        return invalid_glyph_placement();
    }
    let content = scene_content_buffer.values[content_index];
    if (content.glyph_entry_index == NONE_U32
        || content.glyph_entry_index >= arrayLength(&glyph_entry_buffer.values)) {
        return invalid_glyph_placement();
    }
    let entry = glyph_entry_buffer.values[content.glyph_entry_index];
    let cell_extent = content_globals_buffer.globals.glyph_cell_extent_points;
    if ((entry.flags & GLYPH_FLAG_VISIBLE) == 0u
        || entry.metrics.x <= 0.0
        || entry.metrics.y <= 0.0
        || entry.ink_origin_size.z <= 0.0
        || entry.ink_origin_size.w <= 0.0
        || cell_extent.x <= 0.0
        || cell_extent.y <= 0.0) {
        return invalid_glyph_placement();
    }

    var base = vec2<f32>(0.0);
    var instance_opacity = 1.0;
    var destination_cell_extent = cell_extent;
    if (is_floor) {
        if (input.instance_index >= 130u
            || content.kind != 1u
            || primitive.frame_base != 2u
            || primitive.frame_base >= 16u) {
            return invalid_glyph_placement();
        }
        let analytic = frame_buffer.analytics[primitive.frame_base];
        let mask_tag = analytic.payload[0].x;
        let facing_value = analytic.payload[0].y;
        if ((analytic.flags & 1u) == 0u
            || analytic.id != 2u
            || analytic.semantic != 3u
            || analytic.shape != 3u
            || analytic.rect_points.z <= 0.0
            || analytic.rect_points.w <= 0.0
            || mask_tag != 1.0
            || (facing_value != -1.0 && facing_value != 1.0)) {
            return invalid_glyph_placement();
        }
        let floor_cell = analytic.rect_points.zw / vec2<f32>(13.0, 10.0);
        let source_col = input.instance_index % 13u;
        let source_row = input.instance_index / 13u;
        let facing = i32(round(facing_value));
        let projected_col = select(12u - source_col, source_col, facing > 0);
        base = analytic.rect_points.xy + vec2<f32>(
            f32(projected_col) * floor_cell.x,
            f32(9u - source_row) * floor_cell.y,
        );
        destination_cell_extent = floor_cell;
    } else if (is_wall || primitive.instance_group == 1u || primitive.instance_group == 2u) {
        if (input.instance_index >= 130u || content.kind != 1u) {
            return invalid_glyph_placement();
        }
        let pet_col = input.instance_index % 13u;
        let pet_row = input.instance_index / 13u;
        base = vec2<f32>(
            f32(pet_col) * cell_extent.x,
            f32(9u - pet_row) * cell_extent.y,
        );
    } else if (primitive.instance_group == 3u) {
        if (input.instance_index >= 9u || primitive.frame_base >= FRAME_GPU_VALUE_COUNT || content.kind != 2u) {
            return invalid_glyph_placement();
        }
        let frame = frame_buffer.values[primitive.frame_base];
        if ((frame.flags & 1u) == 0u) {
            return invalid_glyph_placement();
        }
        base = vec2<f32>(
            frame.values[0] + frame.values[2] + f32(content.signed_data.x) * cell_extent.x,
            frame.values[1] + frame.values[3] - f32(content.signed_data.y) * cell_extent.y,
        );
        instance_opacity = frame.values[4];
    } else if (primitive.instance_group == 4u) {
        if (input.instance_index >= 32u
            || primitive.frame_base >= FRAME_GPU_VALUE_COUNT
            || input.instance_index > FRAME_GPU_VALUE_COUNT - 1u - primitive.frame_base
            || content.kind != 5u) {
            return invalid_glyph_placement();
        }
        let frame_index = primitive.frame_base + input.instance_index;
        if (frame_index >= FRAME_GPU_VALUE_COUNT) {
            return invalid_glyph_placement();
        }
        let frame = frame_buffer.values[frame_index];
        if ((frame.flags & 1u) == 0u) {
            return invalid_glyph_placement();
        }
        base = vec2<f32>(frame.values[2], frame.values[3]);
        instance_opacity = frame.values[4];
    } else if (primitive.instance_group == 5u || primitive.instance_group == 6u) {
        if (input.instance_index >= 8u
            || primitive.frame_base >= FRAME_GPU_VALUE_COUNT
            || input.instance_index > FRAME_GPU_VALUE_COUNT - 1u - primitive.frame_base
            || content.kind != 3u) {
            return invalid_glyph_placement();
        }
        let frame_index = primitive.frame_base + input.instance_index;
        if (frame_index >= FRAME_GPU_VALUE_COUNT) {
            return invalid_glyph_placement();
        }
        let frame = frame_buffer.values[frame_index];
        let expected_layer = primitive.instance_group - 4u;
        if ((frame.flags & 3u) != 3u || (frame.variant & 0xffffu) != expected_layer) {
            return invalid_glyph_placement();
        }
        base = vec2<f32>(
            frame.values[2] - 0.5 * cell_extent.x,
            frame.values[3] - 0.5 * cell_extent.y,
        );
    } else if (primitive.instance_group == 7u) {
        if (input.instance_index >= 64u
            || primitive.frame_base >= FRAME_GPU_VALUE_COUNT
            || input.instance_index > FRAME_GPU_VALUE_COUNT - 1u - primitive.frame_base
            || content.kind != 4u) {
            return invalid_glyph_placement();
        }
        let frame_index = primitive.frame_base + input.instance_index;
        if (frame_index >= FRAME_GPU_VALUE_COUNT) {
            return invalid_glyph_placement();
        }
        let frame = frame_buffer.values[frame_index];
        if ((frame.flags & 1u) == 0u) {
            return invalid_glyph_placement();
        }
        base = vec2<f32>(frame.values[0], frame.values[1]);
        instance_opacity = frame.values[2];
    } else {
        return invalid_glyph_placement();
    }

    var ink_offset = metric_ink_offset(input.local_position.xy, entry);
    if (is_floor) {
        ink_offset = projected_metric_ink_offset(
            input.local_position.xy,
            entry,
            destination_cell_extent,
        );
    }
    let local_xy = base + ink_offset;
    var world_position: vec4<f32>;
    if (is_wall) {
        if (primitive.frame_base >= 16u
            || primitive.aux_node_index == NONE_U32
            || primitive.aux_node_index >= arrayLength(&node_buffer.values)) {
            return invalid_glyph_placement();
        }
        let analytic = frame_buffer.analytics[primitive.frame_base];
        if ((analytic.flags & 1u) == 0u) {
            return invalid_glyph_placement();
        }
        let aux_node = node_buffer.values[primitive.aux_node_index];
        world_position = apply_node_depth_cue_to_point(
            vec3<f32>(local_xy, input.local_position.z),
            aux_node,
        );
        world_position.x = world_position.x + analytic.payload[0].y;
        world_position.y = world_position.y + analytic.payload[0].z;
        let primary_depth = node.world * vec4<f32>(0.0, 0.0, input.local_position.z, 1.0);
        world_position.z = primary_depth.z;
    } else {
        world_position = apply_node_depth_cue_to_point(
            vec3<f32>(local_xy, input.local_position.z),
            node,
        );
    }

    var placement: GlyphInstancePlacement;
    placement.world_position = world_position;
    placement.content_index = content_index;
    placement.opacity = node.opacity * f32(node.visible) * instance_opacity;
    placement.saturation = node.depth_cue.w;
    placement.valid = 1u;
    return placement;
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
    output.analytic_id = NONE_U32;
    output.point_position = vec2<f32>(0.0);
    output.local_coordinate = input.local_position.xy;
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
fn vs_world_glyph(input: SceneVertexInput) -> SceneVertexOutput {
    if (input.primitive_index >= arrayLength(&primitive_buffer.values)) {
        var invalid: SceneVertexOutput;
        invalid.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        invalid.uv = input.uv;
        invalid.content_index = NONE_U32;
        invalid.opacity = 0.0;
        invalid.saturation = 0.0;
        invalid.instance_group = 0u;
        invalid.analytic_id = NONE_U32;
        invalid.point_position = vec2<f32>(0.0);
        invalid.local_coordinate = input.local_position.xy;
        return invalid;
    }
    let primitive = primitive_buffer.values[input.primitive_index];
    if (primitive.node_index >= arrayLength(&node_buffer.values)) {
        var invalid: SceneVertexOutput;
        invalid.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        invalid.uv = input.uv;
        invalid.content_index = NONE_U32;
        invalid.opacity = 0.0;
        invalid.saturation = 0.0;
        invalid.instance_group = primitive.instance_group;
        invalid.analytic_id = NONE_U32;
        invalid.point_position = vec2<f32>(0.0);
        invalid.local_coordinate = input.local_position.xy;
        return invalid;
    }
    let node = node_buffer.values[primitive.node_index];
    let placement = glyph_instance_placement(input, primitive, node);
    var output: SceneVertexOutput;
    if (placement.valid == 0u) {
        output.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    } else {
        output.position = frame_buffer.globals.projection
            * frame_buffer.globals.view
            * placement.world_position;
    }
    output.uv = input.uv;
    output.content_index = placement.content_index;
    output.opacity = placement.opacity;
    output.saturation = placement.saturation;
    output.instance_group = primitive.instance_group;
    output.analytic_id = select(NONE_U32, primitive.binding_index,
        primitive.primitive_kind == 2u
            && (primitive.binding_index == 1u || primitive.binding_index == 2u));
    output.point_position = placement.world_position.xy;
    output.local_coordinate = input.local_position.xy;
    return output;
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

fn expected_analytic_shape(analytic_id: u32) -> u32 {
    switch analytic_id {
        case 0u: { return 1u; }
        case 1u: { return 2u; }
        case 2u: { return 3u; }
        case 3u: { return 4u; }
        case 4u: { return 5u; }
        case 5u: { return 6u; }
        case 6u: { return 7u; }
        case 7u: { return 8u; }
        case 8u: { return 9u; }
        default: { return NONE_U32; }
    }
}

fn valid_analytic_role(
    analytic_id: u32,
    analytic: AnalyticFrameGpuValue,
    content: AnalyticContentGpuValue,
) -> bool {
    let expected_shape = expected_analytic_shape(analytic_id);
    return analytic_id < 9u
        && expected_shape != NONE_U32
        && (analytic.flags & 1u) != 0u
        && (content.flags & 1u) != 0u
        && analytic.id == analytic_id
        && content.id == analytic_id
        && analytic.semantic == analytic_id + 1u
        && content.semantic == analytic_id + 1u
        && analytic.shape == expected_shape
        && content.shape == expected_shape
        && analytic.rect_points.z > 0.0
        && analytic.rect_points.w > 0.0;
}

fn invalid_analytic_vertex(
    input: SceneVertexInput,
    analytic_id: u32,
) -> SceneVertexOutput {
    var output: SceneVertexOutput;
    output.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    output.uv = input.uv;
    output.content_index = NONE_U32;
    output.opacity = 0.0;
    output.saturation = 0.0;
    output.instance_group = 0u;
    output.analytic_id = analytic_id;
    output.point_position = vec2<f32>(0.0);
    output.local_coordinate = input.local_position.xy;
    return output;
}

fn analytic_vertex(
    input: SceneVertexInput,
    screen_space: bool,
) -> SceneVertexOutput {
    if (input.primitive_index >= arrayLength(&primitive_buffer.values)) {
        return invalid_analytic_vertex(input, NONE_U32);
    }
    let primitive = primitive_buffer.values[input.primitive_index];
    let analytic_id = primitive.binding_index;
    if (analytic_id >= 16u || primitive.node_index >= arrayLength(&node_buffer.values)) {
        return invalid_analytic_vertex(input, analytic_id);
    }
    let analytic = frame_buffer.analytics[primitive.binding_index];
    let content = scene_content_buffer.analytics[primitive.binding_index];
    if (!valid_analytic_role(analytic_id, analytic, content)) {
        return invalid_analytic_vertex(input, analytic_id);
    }
    let node = node_buffer.values[primitive.node_index];
    let point_position = analytic.rect_points.xy
        + input.local_position.xy * analytic.rect_points.zw;
    let world_position = apply_node_depth_cue_to_point(
        vec3<f32>(point_position, input.local_position.z),
        node,
    );
    var position: vec4<f32>;
    if (screen_space) {
        let normalized = vec2<f32>(
            world_position.x * 2.0 / frame_buffer.globals.viewport_points.x - 1.0,
            world_position.y * 2.0 / frame_buffer.globals.viewport_points.y - 1.0,
        );
        position = vec4<f32>(normalized, world_position.z, 1.0);
    } else {
        position = frame_buffer.globals.projection
            * frame_buffer.globals.view
            * world_position;
    }
    var output: SceneVertexOutput;
    output.position = position;
    output.uv = input.uv;
    output.content_index = NONE_U32;
    output.opacity = node.opacity * f32(node.visible);
    output.saturation = node.depth_cue.w;
    output.instance_group = primitive.instance_group;
    output.analytic_id = analytic_id;
    output.point_position = point_position;
    output.local_coordinate = input.local_position.xy;
    return output;
}

@vertex
fn vs_world_analytic(input: SceneVertexInput) -> SceneVertexOutput {
    return analytic_vertex(input, false);
}

@vertex
fn vs_screen_analytic(input: SceneVertexInput) -> SceneVertexOutput {
    return analytic_vertex(input, true);
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

fn linear_channel_to_srgb(value: f32) -> f32 {
    if (value <= 0.0031308) {
        return value * 12.92;
    }
    return 1.055 * pow(value, 1.0 / 2.4) - 0.055;
}

fn linear_to_srgb(value: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        linear_channel_to_srgb(value.r),
        linear_channel_to_srgb(value.g),
        linear_channel_to_srgb(value.b),
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

fn packed_rgba8_linear(packed: u32) -> vec4<f32> {
    let straight_srgb = vec4<f32>(
        f32(packed & 0xffu),
        f32((packed >> 8u) & 0xffu),
        f32((packed >> 16u) & 0xffu),
        f32((packed >> 24u) & 0xffu),
    ) / 255.0;
    return vec4<f32>(srgb_to_linear(straight_srgb.rgb), straight_srgb.a);
}

fn packed_rgba8_unorm(packed: u32) -> vec4<f32> {
    return vec4<f32>(
        f32(packed & 0xffu),
        f32((packed >> 8u) & 0xffu),
        f32((packed >> 16u) & 0xffu),
        f32((packed >> 24u) & 0xffu),
    ) / 255.0;
}

fn packed_rgb8_linear(packed: u32) -> vec3<f32> {
    return srgb_to_linear(vec3<f32>(
        f32(packed & 0xffu),
        f32((packed >> 8u) & 0xffu),
        f32((packed >> 16u) & 0xffu),
    ) / 255.0);
}

fn explicit_packed_paint_linear(content: SceneContentGpuValue) -> vec4<f32> {
    return packed_rgba8_linear(content.variant);
}

fn glyph_paint_linear(input: SceneVertexOutput) -> vec4<f32> {
    let content = scene_content_buffer.values[input.content_index];
    if (content.kind == 3u) {
        return tank_paint_linear(content);
    }
    if ((content.kind == 2u && (content.flags & 64u) != 0u)
        || (content.kind == 5u && (content.flags & 1u) != 0u)
        || (content.kind == 4u && (content.flags & 256u) != 0u)) {
        return explicit_packed_paint_linear(content);
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

fn analytic_premultiply(
    straight_linear: vec4<f32>,
    coverage: f32,
    opacity: f32,
    saturation: f32,
) -> vec4<f32> {
    let alpha = straight_linear.a * clamp(coverage, 0.0, 1.0) * opacity;
    let rgb = apply_saturation(straight_linear.rgb, saturation);
    return vec4<f32>(rgb * alpha, alpha);
}

fn circle_coverage(distance: f32, radius: f32) -> f32 {
    let edge = max(fwidth(distance), 0.0001);
    return 1.0 - smoothstep(radius - edge, radius + edge, distance);
}

fn fs_room_aperture(
    input: SceneVertexOutput,
    content: AnalyticContentGpuValue,
    analytic: AnalyticFrameGpuValue,
) -> vec4<f32> {
    let center = analytic.payload[0].xy;
    let radius = analytic.payload[0].z;
    let feather = analytic.payload[0].w;
    if (radius <= 0.0 || feather < 0.0) {
        return vec4<f32>(0.0);
    }
    let distance = length(input.point_position - center);
    let radial = smoothstep(0.0, radius, distance);
    let core = packed_rgb8_linear(content.payload[0].x);
    let rim = packed_rgb8_linear(content.payload[0].y);
    let bed = packed_rgb8_linear(content.payload[0].z);
    let fleck = packed_rgb8_linear(content.payload[0].w);
    let viewport = frame_buffer.globals.viewport_points;
    let point_y_down = vec2<f32>(
        input.point_position.x,
        viewport.y - input.point_position.y,
    );
    let normalized_x = input.point_position.x / viewport.x - 0.5;
    let horizon_y = viewport.y * (0.76 + 0.04 * normalized_x * normalized_x);
    let bed_feather = max(viewport.y * 0.12, 1.0);
    let bed_mix = smoothstep(horizon_y, horizon_y + bed_feather, point_y_down.y);

    let point_step = max(fwidth(input.point_position), vec2<f32>(0.0001));
    let backing_scale_xy = vec2<f32>(1.0) / point_step;
    let backing_scale = 0.5 * (backing_scale_xy.x + backing_scale_xy.y);
    let physical_hash_point = vec2<u32>(max(
        floor(input.point_position * backing_scale),
        vec2<f32>(0.0),
    ));
    var hash = (physical_hash_point.x * 0x9e3779b9u)
        ^ (physical_hash_point.y * 0x85ebca6bu);
    hash = hash ^ (hash >> 16u);
    hash = hash * 0x7feb352du;
    hash = hash ^ (hash >> 15u);
    let dither_levels = (f32(hash & 0xffffu) / 65535.0 - 0.5) * 3.0;
    let fleck_random = f32((hash >> 16u) & 0xffffu) / 65535.0;
    let fleck_density = max(bed_mix - 0.35, 0.0) * 0.16;
    let fleck_mix = select(0.0, 0.35 + 0.55 * bed_mix, fleck_random < fleck_density);

    var room = mix(core, rim, radial);
    room = mix(room, bed, bed_mix * 0.72);
    room = mix(room, fleck, fleck_mix);
    var room_srgb = linear_to_srgb(room);
    room_srgb = clamp(
        room_srgb + vec3<f32>(dither_levels / 255.0),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );
    let straight = vec4<f32>(srgb_to_linear(room_srgb), 1.0);
    return analytic_premultiply(straight, 1.0, input.opacity, input.saturation);
}

fn fs_prop_shadows(
    input: SceneVertexOutput,
    content: AnalyticContentGpuValue,
) -> vec4<f32> {
    let cell_extent = content_globals_buffer.globals.glyph_cell_extent_points;
    if (min(cell_extent.x, cell_extent.y) <= 0.0) {
        return vec4<f32>(0.0);
    }
    var union_coverage = 0.0;
    for (var slot = 0u; slot < PROP_FRAME_GPU_COUNT; slot = slot + 1u) {
        let frame_index = PROP_FRAME_GPU_BASE + slot * PROP_FRAME_GPU_STRIDE;
        if (frame_index < FRAME_GPU_VALUE_COUNT) {
            let frame = frame_buffer.values[frame_index];
            let visible = (frame.flags & 1u) != 0u;
            let footprint = vec2<f32>(frame.values[5], frame.values[6]);
            let strength = frame.values[7];
            if (frame.kind == 1u
                && visible
                && min(footprint.x, footprint.y) > 0.0
                && strength > 0.0) {
                let origin = vec2<f32>(frame.values[0], frame.values[1])
                    + vec2<f32>(frame.values[2], frame.values[3]);
                let radii = vec2<f32>(
                    max(footprint.x * 0.375, cell_extent.x),
                    cell_extent.y * 0.15,
                );
                let center = vec2<f32>(
                    origin.x + footprint.x * 0.5,
                    origin.y - max(footprint.y - cell_extent.y, 0.0) + radii.y,
                );
                let distance = length((input.point_position - center) / radii);
                let edge = max(fwidth(distance), 0.0001);
                let ellipse = 1.0 - smoothstep(1.0 - edge, 1.0 + edge, distance);
                let slot_coverage = ellipse
                    * clamp(strength, 0.0, 1.0)
                    * clamp(frame.values[4], 0.0, 1.0);
                union_coverage = max(union_coverage, slot_coverage);
            }
        }
    }
    let straight = vec4<f32>(packed_rgb8_linear(content.payload[0].x), 1.0);
    return analytic_premultiply(
        straight,
        clamp(union_coverage, 0.0, 1.0),
        input.opacity,
        input.saturation,
    );
}

fn fs_status_tone(
    input: SceneVertexOutput,
    content: AnalyticContentGpuValue,
    analytic: AnalyticFrameGpuValue,
) -> vec4<f32> {
    // Authored thickness reserves conservative edge padding around this
    // intentionally filled status disc; it is not a stroke width in scene v2.
    let center = analytic.payload[0].xy;
    let radius = analytic.payload[0].z;
    let tone = analytic.payload[1].x;
    if (radius <= 0.0 || (tone != 1.0 && tone != 2.0)) {
        return vec4<f32>(0.0);
    }
    let packed = select(content.payload[0].x, content.payload[0].y, tone == 2.0);
    let coverage = circle_coverage(length(input.point_position - center), radius);
    return analytic_premultiply(
        packed_rgba8_linear(packed),
        coverage,
        input.opacity,
        input.saturation,
    );
}

fn fs_mood_rings(
    input: SceneVertexOutput,
    content: AnalyticContentGpuValue,
    analytic: AnalyticFrameGpuValue,
) -> vec4<f32> {
    let center = analytic.payload[0].xy;
    let max_radius = analytic.payload[0].z;
    let frame_ring_count = analytic.payload[0].w;
    let feather = analytic.payload[1].x;
    let content_ring_count = f32(content.payload[0].y);
    if (max_radius <= 0.0
        || feather < 0.0
        || frame_ring_count < 1.0
        || frame_ring_count != content_ring_count) {
        return vec4<f32>(0.0);
    }
    let distance = length(input.point_position - center);
    let per_ring_alpha = f32(content.payload[0].z) / 255.0;
    let linear_rgb = apply_saturation(
        packed_rgb8_linear(content.payload[0].x),
        input.saturation,
    );
    var composed = vec4<f32>(0.0);
    // The content contract caps this role at eight nested discs. Compose each
    // disc separately so every internal boundary receives authored feathering
    // and derivative AA instead of a hard integer-alpha step.
    for (var ring = 0u; ring < 8u; ring = ring + 1u) {
        if (f32(ring) < frame_ring_count) {
            let radius = max_radius * f32(ring + 1u) / frame_ring_count;
            let edge = max(feather, fwidth(distance));
            let coverage = 1.0 - smoothstep(radius - edge, radius + edge, distance);
            let alpha = per_ring_alpha * coverage * input.opacity;
            let layer = vec4<f32>(linear_rgb * alpha, alpha);
            composed = over_premultiplied(layer, composed);
        }
    }
    return composed;
}

fn normalized_degrees(value: f32) -> f32 {
    return value - floor(value / 360.0) * 360.0;
}

fn round_arc_coverage(
    point: vec2<f32>,
    center: vec2<f32>,
    radius: f32,
    stroke_width: f32,
    start_degrees: f32,
    sweep_degrees: f32,
) -> f32 {
    if (radius <= 0.0 || stroke_width <= 0.0 || sweep_degrees <= 0.0) {
        return 0.0;
    }
    let local = point - center;
    let angle = normalized_degrees(degrees(atan2(local.y, local.x)));
    let start = normalized_degrees(start_degrees);
    let delta = normalized_degrees(angle - start);
    let half_width = 0.5 * stroke_width;
    let radial_distance = abs(length(local) - radius);
    let edge = max(fwidth(radial_distance), 0.0001);
    let body = select(
        0.0,
        1.0 - smoothstep(half_width - edge, half_width + edge, radial_distance),
        delta <= sweep_degrees,
    );
    let start_radians = radians(start);
    let end_radians = radians(start + sweep_degrees);
    let start_point = center + radius * vec2<f32>(cos(start_radians), sin(start_radians));
    let end_point = center + radius * vec2<f32>(cos(end_radians), sin(end_radians));
    let start_cap = circle_coverage(length(point - start_point), half_width);
    let end_cap = circle_coverage(length(point - end_point), half_width);
    return max(body, max(start_cap, end_cap));
}

fn over_premultiplied(top: vec4<f32>, bottom: vec4<f32>) -> vec4<f32> {
    return top + bottom * (1.0 - top.a);
}

fn gauge_lane_geometry(
    analytic: AnalyticFrameGpuValue,
    lane: u32,
) -> vec4<f32> {
    switch lane {
        case 0u: {
            return vec4<f32>(
                analytic.payload[0].z,
                analytic.payload[0].w,
                analytic.payload[1].x,
                analytic.payload[1].y,
            );
        }
        case 1u: {
            return vec4<f32>(
                analytic.payload[1].z,
                analytic.payload[1].w,
                analytic.payload[2].x,
                analytic.payload[2].y,
            );
        }
        case 2u: {
            return vec4<f32>(
                analytic.payload[2].z,
                analytic.payload[2].w,
                analytic.payload[3].x,
                analytic.payload[3].y,
            );
        }
        default: { return vec4<f32>(0.0); }
    }
}

fn gauge_arc_color(
    input: SceneVertexOutput,
    content: AnalyticContentGpuValue,
    analytic: AnalyticFrameGpuValue,
    lane: u32,
    fraction: f32,
) -> vec4<f32> {
    let center = analytic.payload[0].xy;
    let geometry = gauge_lane_geometry(analytic, lane);
    let radius = geometry.x;
    let width = geometry.y;
    let start = geometry.z;
    let sweep = geometry.w;
    let track_coverage = round_arc_coverage(
        input.point_position, center, radius, width, start, sweep,
    );
    let fill_coverage = round_arc_coverage(
        input.point_position,
        center,
        radius,
        width,
        start,
        sweep * clamp(fraction, 0.0, 1.0),
    );
    let track = analytic_premultiply(
        packed_rgba8_linear(content.payload[lane / 2u][(lane % 2u) * 2u]),
        track_coverage,
        input.opacity,
        input.saturation,
    );
    let fill = analytic_premultiply(
        packed_rgba8_linear(content.payload[lane / 2u][(lane % 2u) * 2u + 1u]),
        fill_coverage,
        input.opacity,
        input.saturation,
    );
    return over_premultiplied(fill, track);
}

fn daily_rollover_color(
    first_rollover: vec4<f32>,
    rollover_contract: vec4<f32>,
    rollover: f32,
) -> vec4<f32> {
    let cap_srgb = rollover_contract.rgb;
    let first_srgb = linear_to_srgb(first_rollover.rgb);
    let remaining = pow(rollover_contract.a, max(rollover - 1.0, 0.0));
    let rollover_srgb = mix(cap_srgb, first_srgb, remaining);
    return vec4<f32>(srgb_to_linear(rollover_srgb), first_rollover.a);
}

fn fs_gauges(
    input: SceneVertexOutput,
    content: AnalyticContentGpuValue,
    analytic: AnalyticFrameGpuValue,
) -> vec4<f32> {
    if ((analytic.flags & 0x00011100u) != 0x00011100u) {
        return vec4<f32>(0.0);
    }
    // The packed order is xp, daily, daily-overage, pace. Geometry lane order
    // remains xp, daily, pace.
    let xp = gauge_arc_color(input, content, analytic, 0u, frame_buffer.globals.gauges.x);
    let daily = gauge_arc_color(input, content, analytic, 1u, frame_buffer.globals.gauges.y);
    let pace = gauge_arc_color(input, content, analytic, 2u, frame_buffer.globals.gauges.w);
    let daily_geometry = gauge_lane_geometry(analytic, 1u);
    let daily_excess = max(frame_buffer.globals.gauges.z, 0.0);
    let completed_rollovers = floor(daily_excess);
    let current_fraction = daily_excess - completed_rollovers;
    let first_rollover = packed_rgba8_linear(content.payload[1].z);
    let rollover_contract = packed_rgba8_unorm(content.payload[1].w);
    var completed_overage = vec4<f32>(0.0);
    if (completed_rollovers >= 1.0) {
        let completed_coverage = round_arc_coverage(
            input.point_position,
            analytic.payload[0].xy,
            daily_geometry.x,
            daily_geometry.y,
            daily_geometry.z,
            daily_geometry.w,
        );
        completed_overage = analytic_premultiply(
            daily_rollover_color(first_rollover, rollover_contract, completed_rollovers),
            completed_coverage,
            input.opacity,
            input.saturation,
        );
    }
    var current_overage = vec4<f32>(0.0);
    if (current_fraction > 0.0) {
        let current_coverage = round_arc_coverage(
            input.point_position,
            analytic.payload[0].xy,
            daily_geometry.x,
            daily_geometry.y,
            daily_geometry.z,
            daily_geometry.w * clamp(current_fraction, 0.0, 1.0),
        );
        current_overage = analytic_premultiply(
            daily_rollover_color(
                first_rollover,
                rollover_contract,
                completed_rollovers + 1.0,
            ),
            current_coverage,
            input.opacity,
            input.saturation,
        );
    }
    let overage = over_premultiplied(current_overage, completed_overage);
    return over_premultiplied(pace, over_premultiplied(overage, over_premultiplied(daily, xp)));
}

fn fs_trouble(
    input: SceneVertexOutput,
    content: AnalyticContentGpuValue,
    analytic: AnalyticFrameGpuValue,
) -> vec4<f32> {
    // Authored thickness reserves conservative edge padding around this
    // intentionally filled trouble disc; it is not a stroke width in scene v2.
    let radius = analytic.payload[0].z;
    if (radius <= 0.0) {
        return vec4<f32>(0.0);
    }
    let coverage = circle_coverage(
        length(input.point_position - analytic.payload[0].xy),
        radius,
    );
    return analytic_premultiply(
        packed_rgba8_linear(content.payload[0].x),
        coverage,
        input.opacity,
        input.saturation,
    );
}

fn fs_dim(
    input: SceneVertexOutput,
    content: AnalyticContentGpuValue,
    analytic: AnalyticFrameGpuValue,
) -> vec4<f32> {
    let straight = vec4<f32>(
        packed_rgb8_linear(content.payload[0].x),
        1.0,
    );
    return analytic_premultiply(
        straight,
        1.0,
        input.opacity,
        input.saturation,
    );
}

@fragment
fn fs_analytic(input: SceneVertexOutput) -> @location(0) vec4<f32> {
    if (input.analytic_id >= 16u) {
        discard;
    }
    let analytic = frame_buffer.analytics[input.analytic_id];
    let content = scene_content_buffer.analytics[input.analytic_id];
    if (!valid_analytic_role(input.analytic_id, analytic, content)) {
        discard;
    }
    var output = vec4<f32>(0.0);
    switch input.analytic_id {
        case 0u: { output = fs_room_aperture(input, content, analytic); }
        case 3u: { output = fs_status_tone(input, content, analytic); }
        case 4u: { output = fs_mood_rings(input, content, analytic); }
        case 5u: { output = fs_gauges(input, content, analytic); }
        case 6u: { output = fs_trouble(input, content, analytic); }
        case 7u: { output = fs_dim(input, content, analytic); }
        case 8u: { output = fs_prop_shadows(input, content); }
        default: { discard; }
    }
    if (output.a <= 0.0) {
        discard;
    }
    return output;
}

@fragment
fn fs_glyph(input: SceneVertexOutput) -> @location(0) vec4<f32> {
    if (input.content_index == NONE_U32 || input.content_index >= 462u) {
        discard;
    }
    let content = scene_content_buffer.values[input.content_index];
    if (content.glyph_entry_index == NONE_U32
        || content.glyph_entry_index >= arrayLength(&glyph_entry_buffer.values)) {
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

@fragment
fn fs_wall_shadow_glyph(input: SceneVertexOutput) -> @location(0) vec4<f32> {
    if (input.analytic_id != 1u
        || input.content_index == NONE_U32
        || input.content_index >= 462u) {
        discard;
    }
    let analytic = frame_buffer.analytics[input.analytic_id];
    let content = scene_content_buffer.analytics[input.analytic_id];
    if (!valid_analytic_role(input.analytic_id, analytic, content)) {
        discard;
    }
    let glyph = scene_content_buffer.values[input.content_index];
    if (glyph.glyph_entry_index == NONE_U32
        || glyph.glyph_entry_index >= arrayLength(&glyph_entry_buffer.values)) {
        discard;
    }
    let entry = glyph_entry_buffer.values[glyph.glyph_entry_index];
    if ((entry.flags & GLYPH_FLAG_VISIBLE) == 0u) {
        discard;
    }
    // The wall role always samples crisp atlas AA coverage. Native atlas color
    // and pet palette roles are irrelevant; the packed tint remains readable
    // when a dark display crushes the tank's rear wall toward black.
    let uv = glyph_uv(input, entry);
    var coverage: f32;
    if ((entry.flags & GLYPH_FLAG_COLOR) != 0u) {
        // Native color contributes alpha coverage only; sampled RGB remains
        // irrelevant to the authored shadow tint.
        coverage = textureSampleLevel(color_texture, atlas_sampler, uv, 0.0).a;
    } else {
        coverage = textureSampleLevel(coverage_texture, atlas_sampler, uv, 0.0).r;
    }
    let authored_opacity = f32(content.payload[0].y) / 255.0;
    let alpha = coverage * authored_opacity * input.opacity;
    if (alpha <= 0.0) {
        discard;
    }
    let linear_rgb = packed_rgb8_linear(content.payload[0].x);
    return vec4<f32>(linear_rgb * alpha, alpha);
}

@fragment
fn fs_floor_shadow_glyph(input: SceneVertexOutput) -> @location(0) vec4<f32> {
    if (input.analytic_id != 2u
        || input.content_index == NONE_U32
        || input.content_index >= 462u) {
        discard;
    }
    let analytic = frame_buffer.analytics[input.analytic_id];
    let content = scene_content_buffer.analytics[input.analytic_id];
    if (!valid_analytic_role(input.analytic_id, analytic, content)
        || analytic.payload[0].x != 1.0
        || (analytic.payload[0].y != -1.0 && analytic.payload[0].y != 1.0)) {
        discard;
    }
    let glyph = scene_content_buffer.values[input.content_index];
    if (glyph.kind != 1u
        || glyph.glyph_entry_index == NONE_U32
        || glyph.glyph_entry_index >= arrayLength(&glyph_entry_buffer.values)) {
        discard;
    }
    let entry = glyph_entry_buffer.values[glyph.glyph_entry_index];
    if ((entry.flags & GLYPH_FLAG_VISIBLE) == 0u) {
        discard;
    }
    let uv = glyph_uv(input, entry);
    var coverage: f32;
    if ((entry.flags & GLYPH_FLAG_COLOR) != 0u) {
        coverage = textureSampleLevel(color_texture, atlas_sampler, uv, 0.0).a;
    } else {
        coverage = textureSampleLevel(coverage_texture, atlas_sampler, uv, 0.0).r;
    }
    let paint = packed_rgba8_linear(content.payload[0].x);
    let alpha = coverage * paint.a * input.opacity;
    if (alpha <= 0.0) {
        discard;
    }
    return vec4<f32>(paint.rgb * alpha, alpha);
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
fn fs_aperture_composite(input: FinalVertexOutput) -> @location(0) vec4<f32> {
    let aperture = frame_buffer.analytics[0u];
    let aperture_content = scene_content_buffer.analytics[0u];
    if (!valid_analytic_role(0u, aperture, aperture_content)) {
        return vec4<f32>(0.0);
    }
    let radius = aperture.payload[0].z;
    let feather = aperture.payload[0].w;
    let viewport_points = frame_buffer.globals.viewport_points;
    let dimensions = textureDimensions(scene_sampled_texture);
    if (radius <= 0.0 || feather < 0.0
        || min(viewport_points.x, viewport_points.y) <= 0.0
        || min(dimensions.x, dimensions.y) == 0u) {
        return vec4<f32>(0.0);
    }
    let pixel = vec2<i32>(input.position.xy);
    let sampled = textureLoad(scene_sampled_texture, pixel, 0);
    let normalized_pixel = input.position.xy / vec2<f32>(dimensions);
    let point_position = vec2<f32>(
        normalized_pixel.x * viewport_points.x,
        (1.0 - normalized_pixel.y) * viewport_points.y,
    );
    let distance = length(point_position - aperture.payload[0].xy);
    let point_per_pixel = max(
        viewport_points.x / f32(dimensions.x),
        viewport_points.y / f32(dimensions.y),
    );
    let edge = max(feather, point_per_pixel);
    let coverage = 1.0 - smoothstep(radius - edge, radius + edge, distance);
    return sampled * coverage;
}

@fragment
fn fs_aperture_surface(input: FinalVertexOutput) -> @location(0) vec4<f32> {
    let aperture = frame_buffer.analytics[0u];
    let aperture_content = scene_content_buffer.analytics[0u];
    if (!valid_analytic_role(0u, aperture, aperture_content)) {
        return vec4<f32>(0.0);
    }
    let radius = aperture.payload[0].z;
    let feather = aperture.payload[0].w;
    let viewport_points = frame_buffer.globals.viewport_points;
    let dimensions = textureDimensions(scene_sampled_texture);
    if (radius <= 0.0 || feather < 0.0
        || min(viewport_points.x, viewport_points.y) <= 0.0
        || min(dimensions.x, dimensions.y) == 0u) {
        return vec4<f32>(0.0);
    }
    let pixel = vec2<i32>(input.position.xy);
    let sampled = textureLoad(scene_sampled_texture, pixel, 0);
    let normalized_pixel = input.position.xy / vec2<f32>(dimensions);
    let point_position = vec2<f32>(
        normalized_pixel.x * viewport_points.x,
        (1.0 - normalized_pixel.y) * viewport_points.y,
    );
    let distance = length(point_position - aperture.payload[0].xy);
    let point_per_pixel = max(
        viewport_points.x / f32(dimensions.x),
        viewport_points.y / f32(dimensions.y),
    );
    let edge = max(feather, point_per_pixel);
    let coverage = 1.0 - smoothstep(radius - edge, radius + edge, distance);
    let alpha = sampled.a * coverage;
    if (alpha == 0.0) {
        return vec4<f32>(0.0);
    }
    // The PostMultiplied surface expects straight linear RGB. Coverage affects
    // alpha only after unpremultiplication, exactly matching the historical
    // aperture-intermediate plus `fs_final` pair.
    return vec4<f32>(sampled.rgb / sampled.a, alpha);
}

@fragment
fn fs_final(input: FinalVertexOutput) -> @location(0) vec4<f32> {
    // Sampling the sRGB intermediate decodes its stored premultiplied RGB back
    // to linear before the PostMultiplied surface receives straight RGB.
    let pixel = vec2<i32>(input.position.xy);
    let sampled = textureLoad(scene_sampled_texture, pixel, 0);
    if (sampled.a == 0.0) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(sampled.rgb / sampled.a, sampled.a);
}
