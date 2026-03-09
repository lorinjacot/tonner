use glam::vec3;
use storm::{
    Context,
    geometry::GeometryBuilder,
    mesh::{MeshBuilder, MeshInstance, material::MaterialBuilder},
    scene_graph::{NodeBuilder, SceneGraph},
};

pub fn table(scene_graph: &mut SceneGraph, ctx: &Context) -> [MeshInstance; 5] {
    #[rustfmt::skip]
    let table = GeometryBuilder::new(8, 0)
        .name("Table")
        .positions([
            vec3(-0.65, 0.0, -1.25),
            vec3(-0.65, 0.0, 1.25),
            vec3(0.65, 0.0, 1.25),
            vec3(0.65, 0.0, -1.25),
            vec3(-0.65, -0.1, -1.25),
            vec3(-0.65, -0.1, 1.25),
            vec3(0.65, -0.1, 1.25),
            vec3(0.65, -0.1, -1.25),
        ])
        .unwrap()
        .indices_u16([
            // top face
            0, 1, 2,
            2, 3, 0,
            // large side 1
            1, 0, 4,
            4, 5, 1,
            // large side 2
            3, 2, 6,
            6, 7, 3,
            // small side 1
            2, 1, 5, 
            5, 6, 2, 
            // small side 2
            0, 3, 7, 
            7, 4, 0, 
            // bottom face
            4, 6, 5, 
            6, 4, 7,
        ])
        .build(&ctx)
        .unwrap();
    #[rustfmt::skip]
        let long_border = GeometryBuilder::new(8, 0)
            .name("Long border")
            .positions([
                vec3(-0.01, 0.1, -1.25),
                vec3(-0.01, 0.1, 1.25),
                vec3(0.01, 0.1, 1.25),
                vec3(0.01, 0.1, -1.25),
                vec3(-0.01, 0.0, -1.25),
                vec3(-0.01, 0.0, 1.25),
                vec3(0.01, 0.0, 1.25),
                vec3(0.01, 0.0, -1.25),
            ])
            .unwrap()
            .indices_u16([
                // top face
                0, 1, 2,
                2, 3, 0,
                // large side 1
                1, 0, 4,
                4, 5, 1,
                // large side 2
                3, 2, 6,
                6, 7, 3,
                // small side 1
                2, 1, 5, 
                5, 6, 2, 
                // small side 2
                0, 3, 7, 
                7, 4, 0,
            ])
            .build(&ctx)
            .unwrap();
    #[rustfmt::skip]
        let short_border = GeometryBuilder::new(8, 0)
            .name("Short border")
            .positions([
                vec3(-0.63, 0.1, -0.01),
                vec3(-0.63, 0.1, 0.01),
                vec3(0.63, 0.1, 0.01),
                vec3(0.63, 0.1, -0.01),
                vec3(-0.63, 0.0, -0.01),
                vec3(-0.63, 0.0, 0.01),
                vec3(0.63, 0.0, 0.01),
                vec3(0.63, 0.0, -0.01),
            ])
            .unwrap()
            .indices_u16([
                // top face
                0, 1, 2,
                2, 3, 0,
                // large side 1
                1, 0, 4,
                4, 5, 1,
                // large side 2
                3, 2, 6,
                6, 7, 3,
                // small side 1
                2, 1, 5, 
                5, 6, 2, 
                // small side 2
                0, 3, 7, 
                7, 4, 0,
            ])
            .build(&ctx)
            .unwrap();

    let table_material = MaterialBuilder::default()
        .name("Table")
        .base_color_factor([1.0, 0.0, 0.0, 1.0])
        .metallic_factor(1.0)
        .roughness_factor(0.2)
        .build(&ctx);

    let table_node = NodeBuilder::default()
        .name("Table")
        .build(scene_graph)
        .unwrap();
    let table = MeshBuilder::default()
        .name("Table")
        .primitive(table, table_material.clone())
        .build(&ctx)
        .unwrap()
        .new_instance(table_node);

    let long_border1_node = NodeBuilder::default()
        .name("Table long border 1")
        .parent(table_node)
        .local_translation(vec3(-0.64, 0.0, 0.0))
        .build(scene_graph)
        .unwrap();
    let long_border1 = MeshBuilder::default()
        .name("Table long border 1")
        .primitive(long_border.clone(), table_material.clone())
        .build(&ctx)
        .unwrap()
        .new_instance(long_border1_node);

    let long_border2_node = NodeBuilder::default()
        .name("Table long border 2")
        .parent(table_node)
        .local_translation(vec3(0.64, 0.0, 0.0))
        .build(scene_graph)
        .unwrap();
    let long_border2 = MeshBuilder::default()
        .name("Table long border 2")
        .primitive(long_border, table_material.clone())
        .build(&ctx)
        .unwrap()
        .new_instance(long_border2_node);

    let short_border1_node = NodeBuilder::default()
        .name("Table short border 1")
        .parent(table_node)
        .local_translation(vec3(0.0, 0.0, -1.24))
        .build(scene_graph)
        .unwrap();
    let short_border1 = MeshBuilder::default()
        .name("Table short border 1")
        .primitive(short_border.clone(), table_material.clone())
        .build(&ctx)
        .unwrap()
        .new_instance(short_border1_node);

    let short_border2_node = NodeBuilder::default()
        .name("Table short border 2")
        .parent(table_node)
        .local_translation(vec3(0.0, 0.0, 1.24))
        .build(scene_graph)
        .unwrap();
    let short_border2 = MeshBuilder::default()
        .name("Table short border 2")
        .primitive(short_border, table_material)
        .build(&ctx)
        .unwrap()
        .new_instance(short_border2_node);

    [
        table,
        long_border1,
        long_border2,
        short_border1,
        short_border2,
    ]
}
