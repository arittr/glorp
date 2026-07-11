struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color_a: vec4<f32>,
    @location(1) color_b: vec4<f32>,
    @location(2) uv: vec4<f32>,
    @location(3) local: vec2<f32>,
    @location(4) pixel: vec2<f32>,
    @location(5) params: vec4<f32>,
    @location(6) clip_rect: vec4<f32>,
    @location(7) clip_ellipse: vec4<f32>,
    @location(8) viewport_aperture: vec4<f32>,
    @location(9) aperture_radius: vec4<f32>,
    @location(10) rect: vec4<f32>,
};

@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) rect: vec4<f32>,
    @location(1) color_a: vec4<f32>,
    @location(2) color_b: vec4<f32>,
    @location(3) uv_rect: vec4<f32>,
    @location(4) params: vec4<f32>,
    @location(5) clip_rect: vec4<f32>,
    @location(6) clip_ellipse: vec4<f32>,
    @location(7) viewport_aperture: vec4<f32>,
    @location(8) aperture_radius: vec4<f32>,
) -> VertexOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let local = corners[vertex_index];
    let pixel = rect.xy + local * rect.zw;
    let viewport = viewport_aperture.xy;
    let ndc = vec2<f32>(pixel.x / viewport.x * 2.0 - 1.0, pixel.y / viewport.y * 2.0 - 1.0);
    var out: VertexOut;
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.color_a = color_a;
    out.color_b = color_b;
    out.uv = uv_rect;
    out.local = local;
    out.pixel = pixel;
    out.params = params;
    out.clip_rect = clip_rect;
    out.clip_ellipse = clip_ellipse;
    out.viewport_aperture = viewport_aperture;
    out.aperture_radius = aperture_radius;
    out.rect = rect;
    return out;
}

fn linear_to_srgb(channel: f32) -> f32 {
    if (channel <= 0.0031308) {
        return channel * 12.92;
    }
    return 1.055 * pow(channel, 1.0 / 2.4) - 0.055;
}

fn srgb_to_linear(channel: f32) -> f32 {
    if (channel <= 0.04045) {
        return channel / 12.92;
    }
    return pow((channel + 0.055) / 1.055, 2.4);
}

fn dither_noise(pixel: vec2<u32>) -> f32 {
    var h = pixel.x * 0x9E3779B9u ^ pixel.y * 0x85EBCA6Bu;
    h = h ^ (h >> 16u);
    h = h * 0x7FEB352Du;
    h = h ^ (h >> 15u);
    return (f32(h & 0xFFFFu) / 65535.0 - 0.5) * 3.0;
}

fn positive_angle(angle: f32) -> f32 {
    let tau = 6.283185307179586;
    return angle - floor(angle / tau) * tau;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let aperture_center = in.viewport_aperture.zw;
    let aperture_radius = in.aperture_radius.x;
    if (distance(in.pixel, aperture_center) > aperture_radius) { discard; }
    if (in.params.y > 0.5 && in.params.y < 1.5) {
        let r = in.clip_rect;
        if (in.pixel.x < r.x || in.pixel.y < r.y || in.pixel.x > r.x + r.z || in.pixel.y > r.y + r.w) { discard; }
    } else if (in.params.y >= 1.5) {
        let e = in.clip_ellipse;
        let q = (in.pixel - e.xy) / max(e.zw, vec2<f32>(0.0001));
        if (dot(q, q) > 1.0) { discard; }
    }

    let kind = in.params.x;
    var output = in.color_a;
    if (kind < 0.5) {
        let atlas_uv = mix(in.uv.xy, in.uv.zw, vec2<f32>(in.local.x, 1.0 - in.local.y));
        let sample = textureSample(atlas, atlas_sampler, atlas_uv);
        if (in.params.w > 0.5) {
            // Native-color glyph (emoji): the atlas already holds premultiplied
            // RGBA, which is exactly the fragment output convention, so it passes
            // through and the authored foreground tint is ignored entirely.
            output = sample;
        } else {
            // Coverage mask: the authored premultiplied color scaled by coverage,
            // multiplying both RGB and alpha so the result stays premultiplied.
            output = output * sample.a;
        }
    } else if (kind >= 1.5) {
        let q = in.local * 2.0 - vec2<f32>(1.0);
        let radius = length(q);
        if (radius > 1.0) { discard; }
        if (kind > 2.5 && kind < 3.5) {
            // Smooth interpolates and dithers in 8-bit sRGB output space. Recreate
            // that quantization, then return linear values for the sRGB surface.
            let t = clamp(radius, 0.0, 1.0);
            let core = vec3<f32>(
                linear_to_srgb(in.color_a.r),
                linear_to_srgb(in.color_a.g),
                linear_to_srgb(in.color_a.b),
            );
            let rim = vec3<f32>(
                linear_to_srgb(in.color_b.r),
                linear_to_srgb(in.color_b.g),
                linear_to_srgb(in.color_b.b),
            );
            let local_pixel = vec2<u32>(floor(in.local * in.rect.zw));
            let noise = dither_noise(local_pixel);
            let quantized = clamp(round(mix(core, rim, t) * 255.0 + vec3<f32>(noise)) / 255.0, vec3<f32>(0.0), vec3<f32>(1.0));
            let a = in.color_a.a;
            // Premultiplied-linear output: the tank falloff keeps its output-space
            // dither, then premultiplies the linear result by alpha like every
            // other primitive.
            output = vec4<f32>(
                srgb_to_linear(quantized.r) * a,
                srgb_to_linear(quantized.g) * a,
                srgb_to_linear(quantized.b) * a,
                a,
            );
        } else if (kind >= 3.5) {
            if (kind < 4.5) {
                output = mix(in.color_b, in.color_a, in.local.y);
            } else {
                // Exact stroked arc: one analytic primitive, with the same shared
                // centerline radius, width, angles, and round/butt cap policy as Smooth.
                let angle = positive_angle(atan2(q.y, q.x));
                let start = positive_angle(in.uv.x);
                let along = positive_angle(angle - start);
                let sweep = in.uv.y;
                let radial_hit = abs(radius - in.uv.z) <= in.uv.w;
                var hit = radial_hit && along <= sweep;
                if (!hit && in.params.w > 0.5) {
                    let start_point = vec2<f32>(cos(start), sin(start)) * in.uv.z;
                    let end_angle = start + sweep;
                    let end_point = vec2<f32>(cos(end_angle), sin(end_angle)) * in.uv.z;
                    hit = distance(q, start_point) <= in.uv.w || distance(q, end_point) <= in.uv.w;
                }
                if (!hit) { discard; }
            }
        }
    }
    if (output.a <= 0.001) { discard; }
    // Every fragment output is already premultiplied-linear, and every blend
    // pipeline is the premultiplied BlendContract, so no per-mode premultiply
    // step is needed here.
    return output;
}
