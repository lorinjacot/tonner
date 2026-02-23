from typing import Literal
import debugpy
import numpy as np

debugpy.listen(5678, in_process_debug_adapter=True)


def update(delta_time: float, scene_graph):
    pass

def mouse_input(button: Literal["Left", "Right", "Middle"], state: Literal["Pressed", "Released"]):
    print("mouse input:", button, state)

def mouse_motion(x: float, y: float):
    pass

def mouse_wheel(x: float, y: float):
    pass