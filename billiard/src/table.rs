use glam::{Vec3, vec3};
use storm::{
    Context,
    geometry::BoxBuilder,
    mesh::{MeshBuilder, MeshInstance, material::MaterialBuilder},
    scene_graph::{NodeBuilder, SceneGraph},
};

pub fn table(scene_graph: &mut SceneGraph, ctx: &Context) -> MeshInstance {
    let surface = BoxBuilder::default()
        .name("Table surface")
        .width(1.3)
        .height(0.1)
        .depth(2.5)
        .translate(Vec3::NEG_Y * 0.05)
        .build(ctx);

    let long_borders = BoxBuilder::default().width(0.02).height(0.05).depth(1.1);
    let long_border1a = long_borders
        .clone()
        .name("Table long border 1a")
        .translate(vec3(-0.64, 0.025, 0.6))
        .build(ctx);
    let long_border1b = long_borders
        .clone()
        .name("Table long border 1b")
        .translate(vec3(-0.64, 0.025, -0.6))
        .build(ctx);
    let long_border2a = long_borders
        .clone()
        .name("Table long border 2a")
        .translate(vec3(0.64, 0.025, 0.6))
        .build(ctx);
    let long_border2b = long_borders
        .name("Table long border 2b")
        .translate(vec3(0.64, 0.025, -0.6))
        .build(ctx);

    let short_borders = BoxBuilder::default().width(1.1).height(0.05).depth(0.02);
    let short_border1 = short_borders
        .clone()
        .name("Table short border 1")
        .translate(vec3(0.0, 0.025, -1.24))
        .build(ctx);
    let short_border2 = short_borders
        .name("Table short border 2")
        .translate(vec3(0.0, 0.025, 1.24))
        .build(ctx);

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

    MeshBuilder::default()
        .name("Table")
        .primitive(long_border1a, table_material.clone())
        .primitive(long_border1b, table_material.clone())
        .primitive(long_border2a, table_material.clone())
        .primitive(long_border2b, table_material.clone())
        .primitive(short_border1, table_material.clone())
        .primitive(short_border2, table_material.clone())
        .primitive(surface, table_material)
        .build(&ctx)
        .unwrap()
        .new_instance(table_node)
}
