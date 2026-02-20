import debugpy
import numpy as np

debugpy.listen(5678, in_process_debug_adapter=True)


def update(delta_time: float, scene_graph):
    for node in scene_graph.nodes():
        print(node.local_translation)