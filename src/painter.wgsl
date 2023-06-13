struct PainterUniforms {
    edge: vec4<f32>,
    fill: vec4<f32>,
    line_width_px: f32,
}

@group(0) @binding(0)
var<uniform> painter_uniforms: PainterUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct VertexOutput {
    // get mapped from clip space to viewport (pixel) space between stages (looks like)
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
//    @location(1) viewport_position: vec2<f32>
}

@vertex
fn vs(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coords = model.tex_coords;
    out.position = vec4<f32>(model.position, 1.0);
//    out.viewport_position = 0.5*(model.position.xy+1.0)*painter_uniforms.viewport_size;
    return out;
}

// signed distance from p to a box centered at the origin of size 2*b
fn sd_box(p: vec2<f32>, b: vec2<f32>) -> f32 {
    var d = abs(p) - b;
    return length(max(d, vec2<f32>())) + min(max(d.x, d.y), 0.0);
}

fn sd_round_box(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + r;
    return length(max(q, vec2<f32>())) + min(max(q.x, q.y), 0.0) - r;
}

fn sd_circle(p: vec2<f32>, r: f32) -> f32 {
    return length(p) - r;
}

fn sd_ellipse(p: vec2<f32>, e: vec2<f32>) -> f32 {
    let pAbs = abs(p);
    let ei = 1.0 / e;
    let e2 = e * e;
    let ve = ei * vec2(e2.x - e2.y, e2.y - e2.x);

    var t = vec2(0.70710678118654752, 0.70710678118654752);
    for (var i = 0; i < 3; i++) {
        let v = ve * t * t * t;
        let u = normalize(pAbs - v) * length(t * e - v);
        let w = ei * (v + u);
        t = normalize(clamp(w, vec2(0.0), vec2(1.0)));
    }

    let nearestAbs = t * e;
    let dist = length(pAbs - nearestAbs);
    if dot(pAbs, pAbs) < dot(nearestAbs, nearestAbs) {return -dist;} else {return dist;}
}

@fragment
fn fs(in: VertexOutput) -> @location(0) vec4<f32> {
    // Scale so distance is evaluated in viewport space.
    // That lets us evaluate the line width in px.
    // Gradient can come out negative depending on triangle orientation,
    // so take the absolute value.
    let duvdx = dpdx(in.tex_coords);  // dvu/dx (tex coord units/viewport pixel)
    let duvdy = dpdy(in.tex_coords);  // duv/dy
    let dx = length(vec2(duvdx.x, duvdy.x));
    let dy = length(vec2(duvdx.y, duvdy.y));
    let s = vec2(dx, dy);

    // let d = sd_round_box(in.tex_coords.xy / s, 0.5 / s, 32.0);
    let d = max(
        -sd_ellipse(in.tex_coords.xy / s, 0.25 / s),
        // -sd_circle(in.tex_coords.xy / s, 8.0 / length(fwidth(in.tex_coords))),
        sd_round_box(in.tex_coords.xy / s, 0.5 / s, 15.0)
    );

    if d < -painter_uniforms.line_width_px {
        let eps = d + painter_uniforms.line_width_px;
        return mix(painter_uniforms.edge, painter_uniforms.fill, saturate(-eps));
    } else if d < 0.0 {
        var color = painter_uniforms.edge;
        color.a = saturate(0.5 - d);
        return color;
    } else {
        discard;
        // return vec4(in.tex_coords, 0.0, 1.0);
        // let d = d * 0.05;
        // return vec4(1.0 - d, 0.7 - 0.3 * d, 0.4 - 0.1 * d, 1.0 - 0.1 * d);
    }
}
