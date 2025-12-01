@vertex
fn trivial(@builtin(vertex_index) idx: u32) -> @builtin(position) vec4f {
    let x = f32((idx << 1u) & 2u);
    let y = f32(idx & 2u);

    return vec4f(x * 2. - 1., 1. - 2. * y, 0., 1.);
}

struct Uniforms {
    resolution: vec2f,
    time: f32,
    frame: u32
}

struct Camera {
    pos: vec3f,
    forward: vec3f,
    right: vec3f,
    up: vec3f
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;
@group(0) @binding(1) 
var<uniform> camera: Camera;

@group(0) @binding(2)
var volume: texture_3d<f32>;
@group(0) @binding(3)
var volumeSampler: sampler;

struct Ray {
    pos: vec3f,
    dir: vec3f
}

struct Box {
    min: vec3f,
    max: vec3f
}

const SkyBlue = vec3f(0.53, 0.81, 0.92);
const White = vec3f(1.);
const Black = vec3f(0.);

const PI = 3.14159265359;

fn aces(x: vec3f) -> vec3f {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3f(0.0), vec3f(1.0));
}

fn reinhard(x: vec3f) -> vec3f {
    return x / (x + vec3f(1.0));
}

fn gamma(x: vec3f, g: f32) -> vec3f {
    return pow(x, vec3f(1.0 / g));
}

fn contrast(x: vec3f, c: f32) -> vec3f {
    return (x - 0.5) * c + 0.5;
}

fn saturation(x: vec3f, s: f32) -> vec3f {
    let luma = dot(x, vec3f(0.2126, 0.7152, 0.0722));
    return mix(vec3f(luma), x, s);
}

/// Returns (front, back) 
/// with sentinel value (-1, -1) if no intersection
fn hit_box(ray: Ray, box: Box) -> vec2f {
    let invDir = 1. / ray.dir;

    let t0 = (box.min - ray.pos) * invDir;
    let t1 = (box.max - ray.pos) * invDir;

    let tmin = min(t0, t1);
    let tmax = max(t0, t1);

    let tEnter = max(max(tmin.x, tmin.y), tmin.z);
    let tExit = min(min(tmax.x, tmax.y), tmax.z);
    
    // No intersection if tExit < tEnter or tExit < 0
    if tExit < tEnter || tExit < 0.0 {
        return vec2f(-1.);
    }

    return vec2f(max(tEnter, 0.0), tExit);
}

const MaxSteps : i32 = 512;
const StepFactor : f32 = 0.5;
/*
"SOLAR"
const COLOR_A: vec3f = vec3f(0.5);
const COLOR_B: vec3f = vec3f(0.5);
const COLOR_C: vec3f = vec3f(1.);
const COLOR_D: vec3f = vec3f(0., 0.1, 0.2);
*/
const COLOR_A: vec3f = vec3f(.5);
const COLOR_B: vec3f = vec3f(.5);
const COLOR_C: vec3f = vec3f(1.);
const COLOR_D: vec3f = vec3f(.3, .2, .2);

fn palette(t: f32) -> vec3f {
    return COLOR_A + COLOR_B * cos(6.283185 * (COLOR_C * t + COLOR_D));
}

fn dda(rayin: Ray, maxDist: f32) -> vec2f {
    var ray = rayin;
    let texDims = vec3f(textureDimensions(volume));
    let voxelSize = 1. / texDims.x;
    let baseStep = 0.5 * voxelSize;

    var dist = 0.0;
    var alpha = 0.0;
    var color = 0.0;

    for (var i = 0; i < MaxSteps; i++) {
        if dist > maxDist || alpha > 0.95 { break; }

        let texPos = clamp(vec3i(ray.pos * texDims), vec3i(0), vec3i(texDims) - 1);
        let sample = textureLoad(volume, texPos, 0).rg;
        color = sample.r;

        let sampleAlpha = sample.g * (1.0 - alpha);
        alpha += sampleAlpha;

        let stepSize = baseStep * mix(2., .5, sample.g);
        ray.pos += ray.dir * stepSize;
        dist += stepSize;
    }

    return vec2f(alpha, color);
}

@fragment
fn DDAMain(@builtin(position) fragcoord: vec4f) -> @location(0) vec4f {
    var uv = (fragcoord.xy / uniforms.resolution - 0.5);
    uv.x *= uniforms.resolution.x / uniforms.resolution.y;

    let focalLength = 1. / tan(0.25);
    let dir = normalize(camera.right * uv.x + camera.up * uv.y + camera.forward * focalLength);
    let ray = Ray(camera.pos, dir);
    let box = Box(vec3f(0.), vec3f(1.));
    let intersection = hit_box(ray, box);

    if all(intersection < vec2f(-0.99)) {
        return vec4f(Black, 1.);
    } else {
        let enter = max(intersection.x, 0.0);
        let exit = intersection.y;
        let maxDist = exit - enter;

        let pos = ray.pos + ray.dir * enter;

        let result = dda(Ray(pos, ray.dir), maxDist);
        let alpha = result.x;
        let color_t = result.y;

        var color = mix(Black, palette(color_t), alpha);
        color = reinhard(color);
        color = aces(color);
        color = gamma(color, 2.2);

        return vec4f(color, 1.0);
    }
}
