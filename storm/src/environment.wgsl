const pi = 3.14159265359;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local_position: vec3<f32>,
}

@group(0) @binding(0) var<uniform> view_projection: mat4x4<f32>;

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
) -> VertexOutput {
    var result: VertexOutput;
    result.position = view_projection * vec4(position, 1.0);
    result.local_position = position.xyz;
    return result;
}

@group(1) @binding(0) var equirectangular_texture: texture_2d<f32>;
@group(1) @binding(1) var equirectangular_sampler: sampler;

const inv_atan = vec2(0.1591, 0.3183);
fn sample_spherical_map(v: vec3<f32>) -> vec2<f32> {
    var uv = vec2(atan2(v.z, v.x), asin(v.y));
    uv *= inv_atan;
    uv = 0.5 - uv;
    return uv;
}

@fragment
fn fs_equirectangular_to_cubemap(
    vertex: VertexOutput,
) -> @location(0) vec4<f32> {
    let tex_coord = vec3(
        vertex.local_position.x,
        vertex.local_position.y,
        -vertex.local_position.z,
    );
    let uv = sample_spherical_map(normalize(tex_coord));
    let color = textureSample(equirectangular_texture, equirectangular_sampler, uv).rgb;

    return vec4(color, 1.0);
}

@group(1) @binding(0) var environment_cubemap_texture: texture_cube<f32>;
@group(1) @binding(1) var environment_cubemap_sampler: sampler;

@fragment
fn fs_irradiance(
    vertex: VertexOutput
) -> @location(0) vec4<f32> {
    // the sample direction equals the hemisphere's orientation
    let normal = normalize(vertex.local_position);

    var irradiance = vec3(0.0);
    var up = vec3(0.0, 1.0, 0.0);
    let right = normalize(cross(up, normal));
    up = normalize(cross(normal, right));

    let sample_delta = 0.025;
    var nr_samples = 0.0;
    for (var phi = 0.0; phi < 2.0 * pi; phi += sample_delta) {
        for (var theta = 0.0; theta < 0.5 * pi; theta += sample_delta) {
            // spherical to cartesian (in tangent space)
            let tangent_sample = vec3(sin(theta) * cos(phi),  sin(theta) * sin(phi), cos(theta));
            // tangent space to world
            var sample_vec = tangent_sample.x * right + tangent_sample.y * up + tangent_sample.z * normal;
            sample_vec.z = -sample_vec.z;

            irradiance += textureSample(environment_cubemap_texture, environment_cubemap_sampler, sample_vec).rgb
                            * cos(theta) * sin(theta);
            nr_samples = nr_samples + 1.0;
        }
    }
    irradiance = pi * irradiance * (1.0 / nr_samples);

    return vec4(irradiance, 1.0);
}

@group(2) @binding(0) var<uniform> roughness: f32;

@fragment
fn fs_prefilter(
    vertex: VertexOutput,
) -> @location(0) vec4<f32> {
    let normal = normalize(vertex.local_position);
    let view = normal;

    const sample_count = 1024u;
    var total_weight = 0.0;   
    var prefiltered_color = vec3(0.0);
    for (var i = 0u; i < sample_count; i++) {
        let xi = hammersley(i, sample_count);
        let halfway = importanceSampleGGX(xi, normal, roughness);
        let light = normalize(2.0 * dot(view, halfway) * halfway - view);

        let n_dot_l = max(dot(normal, light), 0.0);
        if n_dot_l > 0.0 {
            prefiltered_color += textureSample(
                environment_cubemap_texture,
                environment_cubemap_sampler,
                light,
            ).rgb * n_dot_l;
            total_weight += n_dot_l;
        }
    }
    prefiltered_color = prefiltered_color / total_weight;

    return vec4(prefiltered_color, 1.0);
}

fn radicalInverse_VdC(bits: u32) -> f32
{
    var b = bits;
    b = (b << 16u) | (b >> 16u);
    b = ((b & 0x55555555u) << 1u) | ((b & 0xAAAAAAAAu) >> 1u);
    b = ((b & 0x33333333u) << 2u) | ((b & 0xCCCCCCCCu) >> 2u);
    b = ((b & 0x0F0F0F0Fu) << 4u) | ((b & 0xF0F0F0F0u) >> 4u);
    b = ((b & 0x00FF00FFu) << 8u) | ((b & 0xFF00FF00u) >> 8u);
    return f32(b) * 2.3283064365386963e-10; // / 0x100000000
}

fn hammersley(i: u32, n: u32) -> vec2<f32>
{
    return vec2(f32(i)/f32(n), radicalInverse_VdC(i));
}

fn importanceSampleGGX(xi: vec2<f32>, normal: vec3<f32>, roughness: f32) -> vec3<f32>
{
    let a = roughness * roughness;
	
    let phi = 2.0 * pi * xi.x;
    let cos_theta = sqrt((1.0 - xi.y) / (1.0 + (a * a - 1.0) * xi.y));
    let sin_theta = sqrt(1.0 - cos_theta * cos_theta);
	
    // from spherical coordinates to cartesian coordinates
    let halfway = vec3(
        cos(phi) * sin_theta,
        sin(phi) * sin_theta,
        cos_theta
    );
	
    // from tangent-space vector to world-space sample vector
    var up = vec3(0.0, 0.0, 1.0);
    if abs(normal.z) >= 0.999 {
        up = vec3(1.0, 0.0, 0.0);
    }
    let tangent   = normalize(cross(up, normal));
    let bitangent = cross(normal, tangent);
	
    let sample_vec = tangent * halfway.x + bitangent * halfway.y + normal * halfway.z;
    return normalize(sample_vec);
}

struct VertexOutput2d {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main_2d(
    @builtin(vertex_index) vertex_index: u32
) -> VertexOutput2d {
    let positions = array(
        vec4(-1.0,-1.0, 0.0, 1.0),
        vec4( 3.0,-1.0, 0.0, 1.0),
        vec4(-1.0, 3.0, 0.0, 1.0),
    );
    let uvs = array(
        vec2(0.0, 0.0),
        vec2(2.0, 0.0),
        vec2(0.0, 2.0),
    );

    var result: VertexOutput2d;
    result.position = positions[vertex_index];
    result.uv = uvs[vertex_index];
    return result;
}

@fragment
fn fs_brdf_lut(
    vertex: VertexOutput2d
) -> @location(0) vec2<f32> {
    let integrated_brdf = integrateBRDF(vertex.uv.x, vertex.uv.y);
    return integrated_brdf;
}

fn integrateBRDF(n_dot_v: f32, roughness: f32) -> vec2<f32> {
    let view = vec3(
        sqrt(1.0 - n_dot_v * n_dot_v),
        0.0,
        n_dot_v,
    );

    var a = 0.0;
    var b = 0.0;

    var normal = vec3(0.0, 0.0, 1.0);

    const sample_count = 1024u;
    for (var i = 0u; i < sample_count; i++) {
        let xi = hammersley(i, sample_count);
        let halfway = importanceSampleGGX(xi, normal, roughness);
        let light  = normalize(2.0 * dot(view, halfway) * halfway - view);

        let n_dot_l = max(light.z, 0.0);
        let n_dot_h = max(halfway.z, 0.0);
        let v_dot_h = max(dot(view, halfway), 0.0);

        if n_dot_l > 0.0 {
            let g = geometrySmith(normal, view, light, roughness);
            let g_vis = (g * v_dot_h) / (n_dot_h * n_dot_v);
            let fc = pow(1.0 - v_dot_h, 5.0);

            a += (1.0 - fc) * g_vis;
            b += fc * g_vis;
        }
    }
    a /= f32(sample_count);
    b /= f32(sample_count);
    return vec2(a, b);
}

fn geometrySchlickGGX(n_dot_v: f32, roughness: f32) -> f32 {
    let a = roughness;
    let k = (a * a) / 2.0;

    let nom   = n_dot_v;
    let denom = n_dot_v * (1.0 - k) + k;

    return nom / denom;
}

fn geometrySmith(normal: vec3<f32>, view: vec3<f32>, light: vec3<f32>, roughness: f32) -> f32 {
    let n_dot_v = max(dot(normal, view), 0.0);
    let n_dot_l = max(dot(normal, light), 0.0);
    let ggx2 = geometrySchlickGGX(n_dot_v, roughness);
    let ggx1 = geometrySchlickGGX(n_dot_l, roughness);

    return ggx1 * ggx2;
} 