import debugpy
import numpy as np

debugpy.listen(5678, in_process_debug_adapter=True)


def update(scene_graph):
    for node in scene_graph.nodes():
        print(node.set_local_translation(np.array([1, 1, 1])))