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

// Antialiased edge coverage in [0, 1] for a fragment `signed_distance` from an
// edge, over one physical pixel `pixel_width` (supplied by `fwidth` of the same
// distance field). Mirrors `parity::analytic_coverage`: inside (< 0) is fully
// covered, outside (> 0) fully uncovered, with a smoothstep ramp across the band.
fn analytic_coverage(signed_distance: f32, pixel_width: f32) -> f32 {
    let half = 0.5 * max(pixel_width, 1e-7);
    return 1.0 - smoothstep(-half, half, signed_distance);
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // Aperture: analytic coverage across one physical pixel at the porthole edge,
    // replacing the former hard discard so the rim antialiases like Smooth's oval
    // clip. `fwidth` gives the edge's screen-space width, so the transition band is
    // one physical pixel at any backing scale. Nested coverages multiply together
    // (aperture x clip x primitive).
    let aperture_center = in.viewport_aperture.zw;
    let aperture_radius = in.aperture_radius.x;
    let aperture_sd = distance(in.pixel, aperture_center) - aperture_radius;
    let aperture_width = fwidth(aperture_sd);
    var coverage = analytic_coverage(aperture_sd, aperture_width);
    // Cheap far-outside early-out: a fragment more than a physical pixel beyond the
    // porthole contributes nothing. The EDGE band above stays analytic; only the
    // fully-exterior region is discarded.
    if (aperture_sd > aperture_width) { discard; }

    if (in.params.y > 0.5 && in.params.y < 1.5) {
        // Rect clip stays an axis-aligned hard cut: its edges are horizontal and
        // vertical, where a box clip shows no meaningful curved-edge aliasing.
        let r = in.clip_rect;
        if (in.pixel.x < r.x || in.pixel.y < r.y || in.pixel.x > r.x + r.z || in.pixel.y > r.y + r.w) { discard; }
    } else if (in.params.y >= 1.5) {
        // Ellipse clip: analytic coverage on the normalized ellipse field so a
        // clipped layer's curved boundary antialiases instead of hard-stepping.
        let e = in.clip_ellipse;
        let q = (in.pixel - e.xy) / max(e.zw, vec2<f32>(0.0001));
        let ellipse_sd = length(q) - 1.0;
        coverage = coverage * analytic_coverage(ellipse_sd, fwidth(ellipse_sd));
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
        if (kind > 2.5 && kind < 3.5) {
            // TANK depth base. Its round edge coincides with the porthole, whose
            // aperture coverage above already antialiases the rim, so it does NOT
            // add the round-primitive coverage (that would double-darken the rim).
            // Invariant: the tank base is always opaque and drawn source-copy
            // (Replace), so `color_a`/`color_b` premultiplied equal their straight
            // linear values and the sRGB dither below is exact. Do NOT introduce a
            // translucent tank base.
            //
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
        } else if (kind > 5.5) {
            // Radial gradient (kind 6): premultiplied interpolation from the
            // authored inner colour at the centre (radius 0) to the outer colour at
            // the rim (radius 1), with the round primitive's analytic edge coverage.
            // Unlike the opaque tank falloff (kind 3), this carries the inner→outer
            // ALPHA falloff a soft cast shadow needs. Interpolating the premultiplied
            // endpoints keeps a fading-alpha gradient free of dark-edge fringing.
            let t = clamp(radius, 0.0, 1.0);
            output = mix(in.color_a, in.color_b, t);
            let round_sd = radius - 1.0;
            coverage = coverage * analytic_coverage(round_sd, fwidth(round_sd));
        } else if (kind >= 4.5) {
            // Exact stroked arc: analytic coverage on the arc's signed distance,
            // with the same shared centerline radius, width, angles, and round/butt
            // cap policy as Smooth. The primitive rect carries a margin past the
            // stroke so the outer edge's transition band is not clipped.
            let center = in.uv.z;
            let half_width = in.uv.w;
            let sweep = in.uv.y;
            let start = positive_angle(in.uv.x);
            let along = positive_angle(atan2(q.y, q.x) - start);
            var centerline_distance = abs(radius - center);
            if (along > sweep) {
                if (in.params.w > 0.5) {
                    // Round cap: signed distance falls off from the nearest
                    // centerline endpoint, so the cap is a smooth half-disc.
                    let start_point = vec2<f32>(cos(start), sin(start)) * center;
                    let end_point = vec2<f32>(cos(start + sweep), sin(start + sweep)) * center;
                    centerline_distance = min(distance(q, start_point), distance(q, end_point));
                } else {
                    // Butt cap: a straight radial cut past the sweep.
                    discard;
                }
            }
            let arc_sd = centerline_distance - half_width;
            coverage = coverage * analytic_coverage(arc_sd, fwidth(arc_sd));
        } else {
            // Solid round (kind 2) and linear-gradient round (kind 4): antialias
            // the circular edge to match AppKit's oval fill.
            let round_sd = radius - 1.0;
            coverage = coverage * analytic_coverage(round_sd, fwidth(round_sd));
            if (kind >= 3.5) {
                output = mix(in.color_b, in.color_a, in.local.y);
            }
        }
    }

    // Fold the accumulated aperture/clip/primitive coverage into the premultiplied
    // output: scaling both RGB and alpha keeps it premultiplied, exactly as the
    // glyph mask path does.
    output = output * coverage;
    if (output.a <= 0.001) { discard; }
    // Every fragment output is already premultiplied-linear, and every blend
    // pipeline is the premultiplied BlendContract, so no per-mode premultiply
    // step is needed here.
    return output;
}
