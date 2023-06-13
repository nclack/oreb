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

fn sd_ellipse(p: vec2<f32>, ab: vec2<f32>) -> f32 {
    var p = abs(p);
    var ab = ab;
    if p.x > p.y {p = vec2(p.y, p.x);ab = vec2(ab.y, ab.x);}

    let l = ab.y * ab.y - ab.x * ab.x;
    let m = ab.x * p.x / l;
    let m2 = m * m;
    let n = ab.y * p.y / l;
    let n2 = n * n;
    let c = (m2 + n2 - 1.0) / 3.0;
    let c3 = c * c * c;
    let q = c3 + m2 * n2 * 2.0;
    let d = c3 + m2 * n2;
    let g = m + m * n2;
    var co: f32;
    if d < 0.0 {
        let h = acos(q / c3) / 3.0;
        let s = cos(h);
        let t = sin(h) * sqrt(3.0);
        let rx = sqrt(-c * (s + t + 2.0) + m2);
        let ry = sqrt(-c * (s - t + 2.0) + m2);
        co = (ry + sign(l) * rx + abs(g) / (rx * ry) - m) / 2.0;
    } else {
        let h = 2.0 * m * n * sqrt(d);
        let s = sign(q + h) * pow(abs(q + h), 1.0 / 3.0);
        let u = sign(q - h) * pow(abs(q - h), 1.0 / 3.0);
        let rx = -s - u - c * 4.0 + 2.0 * m2;
        let ry = (s - u) * sqrt(3.0);
        let rm = sqrt(rx * rx + ry * ry);
        co = (ry / sqrt(rm - rx) + 2.0 * g / rm - m) / 2.0;
    }
    let r = ab * vec2(co, sqrt(1.0 - co * co));
    return length(r - p) * sign(p.y - r.y);
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
