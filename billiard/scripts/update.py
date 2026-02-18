import debugpy

debugpy.listen(5678, in_process_debug_adapter=True)


def update(scene_graph):
    print("node count:", scene_graph.node_count())
