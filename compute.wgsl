@group(0) @binding(0)
var inputTex: texture_3d<f32>;
@group(0) @binding(1)
var tex: texture_storage_3d<rgba32float, write>;
@group(0) @binding(2)
var<uniform> time: Time;

struct Time {
    delta: f32,
    frame: u32,
    seed: f32,
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

fn fbm(p: vec3f) -> f32 {
    var value = 0.;
    var amplitude = 1.;
    var freq = 1.;
    var maxValue = 0.;

    for (var i = 0u; i < Octaves; i ++) {
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

// MITOSIS 
const F: f32 = 0.0367;
const K: f32 = 0.0649;
// CORAL 
//const F: f32 = 0.0545;
//const K: f32 = 0.062;
// WORMS 
/*
const F: f32 = 0.042;
const K: f32 = 0.063;
*/
// HOLES 
/*
const F: f32 = 0.039;
const K: f32 = 0.058;
*/
// SPIRALS
/*
const F: f32 = 0.014;
const K: f32 = 0.045;
*/
// PULSING
/*
const F: f32 = 0.025;
const K: f32 = 0.056;
*/
const DA: f32 = 1.;
const DB: f32 = .5;
const DT: f32 = 1.;

fn wrap(v: vec3<i32>, size: i32) -> vec3<i32> {
    return ((v % size) + size) % size;
}

fn laplacian(pos: vec3<i32>, size: i32) -> vec2<f32> {
    let center = textureLoad(inputTex, pos, 0).rg;

    // Weights based on inverse distance
    let wFace = 1.0;           // 6 neighbors, distance 1
    let wEdge = 0.707107;      // 12 neighbors, distance sqrt(2)
    let wCorner = 0.577350;    // 8 neighbors, distance sqrt(3)

    var sum = vec2<f32>(0.0);
    var totalWeight = 0.0;
    
    // 6 face neighbors
    sum += wFace * textureLoad(inputTex, wrap(pos + vec3(1, 0, 0), size), 0).rg;
    sum += wFace * textureLoad(inputTex, wrap(pos + vec3(-1, 0, 0), size), 0).rg;
    sum += wFace * textureLoad(inputTex, wrap(pos + vec3(0, 1, 0), size), 0).rg;
    sum += wFace * textureLoad(inputTex, wrap(pos + vec3(0, -1, 0), size), 0).rg;
    sum += wFace * textureLoad(inputTex, wrap(pos + vec3(0, 0, 1), size), 0).rg;
    sum += wFace * textureLoad(inputTex, wrap(pos + vec3(0, 0, -1), size), 0).rg;
    totalWeight += 6.0 * wFace;
    
    // 12 edge neighbors
    sum += wEdge * textureLoad(inputTex, wrap(pos + vec3(1, 1, 0), size), 0).rg;
    sum += wEdge * textureLoad(inputTex, wrap(pos + vec3(1, -1, 0), size), 0).rg;
    sum += wEdge * textureLoad(inputTex, wrap(pos + vec3(-1, 1, 0), size), 0).rg;
    sum += wEdge * textureLoad(inputTex, wrap(pos + vec3(-1, -1, 0), size), 0).rg;
    sum += wEdge * textureLoad(inputTex, wrap(pos + vec3(1, 0, 1), size), 0).rg;
    sum += wEdge * textureLoad(inputTex, wrap(pos + vec3(1, 0, -1), size), 0).rg;
    sum += wEdge * textureLoad(inputTex, wrap(pos + vec3(-1, 0, 1), size), 0).rg;
    sum += wEdge * textureLoad(inputTex, wrap(pos + vec3(-1, 0, -1), size), 0).rg;
    sum += wEdge * textureLoad(inputTex, wrap(pos + vec3(0, 1, 1), size), 0).rg;
    sum += wEdge * textureLoad(inputTex, wrap(pos + vec3(0, 1, -1), size), 0).rg;
    sum += wEdge * textureLoad(inputTex, wrap(pos + vec3(0, -1, 1), size), 0).rg;
    sum += wEdge * textureLoad(inputTex, wrap(pos + vec3(0, -1, -1), size), 0).rg;
    totalWeight += 12.0 * wEdge;
    
    // 8 corner neighbors
    sum += wCorner * textureLoad(inputTex, wrap(pos + vec3(1, 1, 1), size), 0).rg;
    sum += wCorner * textureLoad(inputTex, wrap(pos + vec3(1, 1, -1), size), 0).rg;
    sum += wCorner * textureLoad(inputTex, wrap(pos + vec3(1, -1, 1), size), 0).rg;
    sum += wCorner * textureLoad(inputTex, wrap(pos + vec3(1, -1, -1), size), 0).rg;
    sum += wCorner * textureLoad(inputTex, wrap(pos + vec3(-1, 1, 1), size), 0).rg;
    sum += wCorner * textureLoad(inputTex, wrap(pos + vec3(-1, 1, -1), size), 0).rg;
    sum += wCorner * textureLoad(inputTex, wrap(pos + vec3(-1, -1, 1), size), 0).rg;
    sum += wCorner * textureLoad(inputTex, wrap(pos + vec3(-1, -1, -1), size), 0).rg;
    totalWeight += 8.0 * wCorner;

    return (sum / totalWeight) - center;
}

@compute @workgroup_size(4, 4, 4)
fn TexMain(@builtin(global_invocation_id) globalId: vec3u) {
    let size = vec3i(textureDimensions(inputTex));
    let pos = vec3i(globalId);

    let current = textureLoad(inputTex, pos, 0).rg;
    var a = current.x;
    var b = current.y;

    if time.frame == 0u {
        a = 1.;

        let uv = vec3f(pos) / vec3f(size);
        let center = vec3f(0.5);  // center in UV space

        if distance(uv, center) < 0.2 {
            let noise = fbm4d(vec4f(uv * 40., time.seed));
            b = max(noise - 0.5, 0.);
        }
    } else {
        let lap = laplacian(pos, size.x);
        let reaction = a * b * b;
        let newA = a + (DA * lap.r - reaction + F * (1. - a)) * DT;
        let newB = b + (DB * lap.g + reaction - (K + F) * b) * DT;

        a = clamp(newA, 0., 1.);
        b = clamp(newB, 0., 1.);
    }

    textureStore(tex, pos, vec4f(a, b, 0., 1.));
}
