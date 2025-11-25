@vertex
fn trivial(@builtin(vertex_index) idx: u32) -> @builtin(position) vec4f {
    let x = f32((idx << 1u) & 2u);
    let y = f32(idx & 2u);

    return vec4f(x * 2. - 1., 1. - 2. * y, 0., 1.);
}

override TexSize: u32 = 128u;

struct Uniforms {
    resolution: vec2f,
    time: f32
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

fn dda(ray: Ray) -> f32 {
    let step = normalize(ray.dir);

    let box = Box(vec3f(0.), vec3f(1.));
    let boxIntersection = hit_box(ray, box);

    if all(boxIntersection < vec2f(-0.99)) {
        return 0.;
    }

    let enter = boxIntersection.x;
    let exit = boxIntersection.y;

    var pos = ray.pos + step * enter;

    let textureDims = vec3f(textureDimensions(volume));
    let voxelSize = 1. / textureDims;

    var acc = 0.;
    var dist = 0.;
    let maxDist = exit - enter;

    for (var i = 0; i < MaxSteps; i++) {
        if dist > maxDist || acc > 0.9 { break; }

        let texPos = vec3i(pos * vec3f(textureDimensions(volume)));
        let val = textureLoad(volume, texPos, 0).r;

        let alphaSample = val * (1. - acc);
        acc += alphaSample;

        let stepFactor = mix(4. * length(voxelSize), 0.5 * length(voxelSize), val);
        let stepSize = length(voxelSize) * stepFactor;

        pos += step * stepSize;
        dist += stepSize;
    }

    return acc;
}

@fragment
fn DDAMain(@builtin(position) fragcoord: vec4f) -> @location(0) vec4f {
    var uv = (fragcoord.xy / uniforms.resolution - 0.5);
    uv.x *= uniforms.resolution.x / uniforms.resolution.y;

    let focalLength = 1. / tan(0.25);
    let dir = normalize(camera.right * uv.x + camera.up * uv.y + camera.forward * focalLength);

    let color = dda(Ray(camera.pos, dir));

    return vec4(mix(Black, White, color), 1.);
}

@group(0) @binding(0)
var tex: texture_storage_3d<rgba32float, write>;
@group(0) @binding(1)
var<uniform> time: Uniforms;

struct NoiseParams {
    time: f32,
}

const Frequency : f32 = 5.;
const Octaves : u32 = 5u;

fn hash3(p: vec3f) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise3d(p: vec3f) -> f32 {
    let i = floor(p);
    let f = fract(p);

    let u = f * f * (3. - 2. * f);

// Sample corners of cube
    let n000 = hash3(i + vec3<f32>(0.0, 0.0, 0.0));
    let n100 = hash3(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash3(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash3(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash3(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash3(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash3(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash3(i + vec3<f32>(1.0, 1.0, 1.0));
    
    // Trilinear interpolation
    let nx00 = mix(n000, n100, u.x);
    let nx01 = mix(n001, n101, u.x);
    let nx10 = mix(n010, n110, u.x);
    let nx11 = mix(n011, n111, u.x);

    let nxy0 = mix(nx00, nx10, u.y);
    let nxy1 = mix(nx01, nx11, u.y);

    return mix(nxy0, nxy1, u.z);
}

fn fbm(p: vec3f, octaves: u32) -> f32 {
    var value = 0.;
    var amplitude = 1.;
    var freq = 1.;
    var maxValue = 0.;

    for (var i = 0u; i < octaves; i ++) {
        value += noise3d(p * freq) * amplitude;
        maxValue += amplitude;
        amplitude *= 0.5;
        freq *= 2.;
    }

    return value / maxValue;
}

fn hash4(p: vec4f) -> f32 {
    var p4 = fract(p * vec4f(0.1031, 0.1030, 0.0973, 0.1099));
    p4 += dot(p4, p4.wzxy + 33.33);
    return fract((p4.x + p4.y) * (p4.z + p4.w));
}

fn noise4d(p: vec4f) -> f32 {
    let i = floor(p);
    let f = fract(p);
    
    // Smooth interpolation
    let u = f * f * (3. - 2. * f);
    
    // Sample 16 corners of 4D hypercube
    let n0000 = hash4(i + vec4f(0., 0., 0., 0.));
    let n1000 = hash4(i + vec4f(1., 0., 0., 0.));
    let n0100 = hash4(i + vec4f(0., 1., 0., 0.));
    let n1100 = hash4(i + vec4f(1., 1., 0., 0.));
    let n0010 = hash4(i + vec4f(0., 0., 1., 0.));
    let n1010 = hash4(i + vec4f(1., 0., 1., 0.));
    let n0110 = hash4(i + vec4f(0., 1., 1., 0.));
    let n1110 = hash4(i + vec4f(1., 1., 1., 0.));
    let n0001 = hash4(i + vec4f(0., 0., 0., 1.));
    let n1001 = hash4(i + vec4f(1., 0., 0., 1.));
    let n0101 = hash4(i + vec4f(0., 1., 0., 1.));
    let n1101 = hash4(i + vec4f(1., 1., 0., 1.));
    let n0011 = hash4(i + vec4f(0., 0., 1., 1.));
    let n1011 = hash4(i + vec4f(1., 0., 1., 1.));
    let n0111 = hash4(i + vec4f(0., 1., 1., 1.));
    let n1111 = hash4(i + vec4f(1., 1., 1., 1.));
    
    // 4D linear interpolation (16 -> 8 -> 4 -> 2 -> 1)
    // First interpolate along X (16 -> 8)
    let nx000 = mix(n0000, n1000, u.x);
    let nx100 = mix(n0100, n1100, u.x);
    let nx010 = mix(n0010, n1010, u.x);
    let nx110 = mix(n0110, n1110, u.x);
    let nx001 = mix(n0001, n1001, u.x);
    let nx101 = mix(n0101, n1101, u.x);
    let nx011 = mix(n0011, n1011, u.x);
    let nx111 = mix(n0111, n1111, u.x);
    
    // Interpolate along Y (8 -> 4)
    let nxy00 = mix(nx000, nx100, u.y);
    let nxy10 = mix(nx010, nx110, u.y);
    let nxy01 = mix(nx001, nx101, u.y);
    let nxy11 = mix(nx011, nx111, u.y);
    
    // Interpolate along Z (4 -> 2)
    let nxyz0 = mix(nxy00, nxy10, u.z);
    let nxyz1 = mix(nxy01, nxy11, u.z);
    
    // Final interpolation along W (2 -> 1)
    return mix(nxyz0, nxyz1, u.w);
}

fn fbm4d(p: vec4f) -> f32 {
    var value = 0.;
    var amplitude = 1.;
    var freq = 1.;
    var maxValue = 0.;

    for (var i = 0u; i < Octaves; i++) {
        value += noise4d(p * freq) * amplitude;
        maxValue += amplitude;
        amplitude *= 0.5;
        freq *= 2.;
    }

    return value / maxValue;
}

@compute @workgroup_size(4, 4, 4)
fn TexMain(@builtin(global_invocation_id) globalId: vec3u) {
    let dims = textureDimensions(tex);

    if any(globalId >= dims) {
        return;
    }

    let pos = vec3f(globalId) / vec3f(dims);
    let noise = fbm4d(vec4f(pos * Frequency, time.time * 0.1));
    let density = pow(max(noise - 0.6, 0.0), 1.5);
    textureStore(tex, vec3i(globalId), vec4f(density));
}
