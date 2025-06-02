struct Attribute {
    position: vec3<f32>,
    normal: vec3<f32>,
    #ifdef TANGENT
        tangent: vec4<f32>,
    #endif
    #ifdef TEX_COORD_0
        tex_coord_0: vec2<f32>,
    #endif
    #ifdef TEX_COORD_1
        tex_coord_1: vec2<f32>,
    #endif
    #ifdef COLOR_0
        color_0: vec4<f32>,
    #endif
    #ifdef JOINTS_0
        joints_0: vec4<u32>,
    #endif
    #ifdef WEIGHTS_0
        weights_0: vec4<f32>,
    #endif
    #ifdef MORPH_TARGET
        targets: array<MorphTarget, #MORPH_TARGET>,
    #endif
}

#ifdef MORPH_TARGET
    struct MorphTarget {

    }
#endif