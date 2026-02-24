from typing import Literal
import debugpy
import numpy as np
import quaternion

debugpy.listen(5678, in_process_debug_adapter=True)

camera_action: Literal["Rotate", "Zoom"] | None = None
camera_horizontal_angle: float = 0
camera_horizontal_speed = 1
camera_vertical_angle: float = 0
camera_vertical_speed = 1
camera_distance = 1
camera_mouse_wheel_zoom_speed = 1
camera_mouse_motion_zoom_speed = 1


def mouse_input(button: Literal["Left", "Right", "Middle"], state: Literal["Pressed", "Released"]):
    global camera_action

    if button == "Left":
        if camera_action == None and state == "Pressed":
            camera_action = "Rotate"
        elif camera_action == "Rotate" and state == "Released":
            camera_action = None
    
    elif button == "Middle":
        if camera_action == None and state == "Pressed":
            camera_action = "Zoom"
        elif camera_action == "Zoom" and state == "Released":
            camera_action = None

def mouse_motion(x: float, y: float):
    if camera_action == "Rotate":
        global camera_horizontal_angle, camera_vertical_angle
        camera_horizontal_angle += x * camera_horizontal_speed
        camera_vertical_angle += y * camera_vertical_speed

def mouse_wheel(x: float, y: float):
    pass

def update(delta_time: float, scene_graph):
    print(f"{camera_horizontal_angle=}, {camera_vertical_angle=}")