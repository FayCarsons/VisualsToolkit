@vertex
fn trivial(@builtin(vertex_index) idx: u32) -> @builtin(position) vec4f {
    let x = f32((idx << 1u) & 2u);
    let y = f32(idx & 2u);

    return vec4f(x * 2. - 1., 1. - 2. * y, 0., 1.);
}

struct Uniforms {
    resolution: vec2f,
    time: f32
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var volume: texture_3d<f32>;
@group(0) @binding(2)
var volumeSampler: sampler;

struct Ray {
    pos: vec3f,
    dir: vec3f
}

const MaxSteps : i32 = 512;

fn dda(ray: Ray) -> vec4f {
    let step = normalize(ray.dir);
    var pos = ray.pos;

    let textureDims = vec3f(textureDimensions(volume));
    let voxelSize = 1. / textureDims;
    let stepSize = length(voxelSize) * 0.5;

    var acc = vec4f(0.);

    for (var i = 0; i < MaxSteps; i++) {
        if any(pos < vec3f(0.)) || any(pos > vec3f(1.)) { break; }

        let val = textureSampleLevel(volume, volumeSampler, pos, 0.);

        if acc.a >= 0.99 { break; }
        let alphaSample = val.a * (1. - acc.a);
        acc += vec4f(val.rgb * alphaSample, alphaSample);

        pos += step * stepSize;
    }

    return acc;
}

fn getRay(uv: vec2f, cameraPos: vec3f, cameraTarget: vec3f, fov: f32) -> vec3f {
    let up = vec3f(0., 1., 0.);
    let cw = normalize(cameraTarget - cameraPos);
    let cu = normalize(cross(cw, up));
    let cv = cross(cu, cw);

    let focalLength: f32 = 1. / tan(fov * 0.5);

    return normalize(cu * uv.x + cv * uv.y + cw * focalLength);
}

@fragment
fn DDAMain(@builtin(position) fragcoord: vec4f) -> @location(0) vec4f {
    let uv = (fragcoord.xy / uniforms.resolution - 0.5) * 2. * vec2f(uniforms.resolution.x / uniforms.resolution.y, 1.);

    let cameraPos = vec3f(0.5, 0.5, -1.);
    let cameraTarget = vec3f(0.5);

    let fov = 1.;

    let rayDir = getRay(uv, cameraPos, cameraTarget, fov);

    let color = dda(Ray(cameraPos, rayDir));

    return vec4(color.rgb, 1.);
}

@group(1) @binding(0)
var tex: texture_storage_3d<rgba8unorm, write>;
@group(1) @binding(1)
var<uniform> params: NoiseParams;

struct NoiseParams {
    time: f32,
    frequency: f32,
    octaves: u32,
}

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

@compute @workgroup_size(4, 4, 4)
fn TexMain(@builtin(global_invocation_id) globalId: vec3u) {
    let dims = textureDimensions(tex);

    if any(globalId >= dims) {
        return;
    }

    let pos = vec3f(globalId) / vec3f(dims);

    let animated = pos * params.frequency + vec3f(params.time * 0.1, 0., 0.);
    let val = fbm(animated, params.octaves);

    let center = vec3f(0.5);
    let distFromCenter = length(pos - center);
    let density = val * (1. - smoothstep(0.2, 0.5, distFromCenter));

    let color = mix(
        vec3f(0.2, 0.4, 0.8),
        vec3f(1.),
        pos.y
    );

    textureStore(tex, vec3i(globalId), vec4f(color * density, density));
}
