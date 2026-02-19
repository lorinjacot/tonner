import debugpy

debugpy.listen(5678, in_process_debug_adapter=True)


def update(scene_graph):
    for node in scene_graph.nodes():
        print(node.local_translation())