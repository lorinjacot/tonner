from typing import Literal
import importlib, sys

import debugpy
import numpy as np
import quaternion

debugpy.listen(5678, in_process_debug_adapter=True)

if "physics" in sys.modules:
    importlib.reload(sys.modules["physics"])
if "constraints" in sys.modules:
    importlib.reload(sys.modules["constraints"])

from physics import simulate

camera_action: Literal["Rotate", "Zoom"] | None = None
camera_horizontal_angle: float = np.pi / 4.0
camera_horizontal_speed = -1e-3
camera_vertical_angle: float = np.pi / 8.0
camera_vertical_speed = 1e-3
camera_distance = 3
camera_mouse_wheel_zoom_speed = 1.2
camera_mouse_motion_zoom_speed = 1e-3

reset = False


def mouse_input(
    button: Literal["Left", "Right", "Middle"], state: Literal["Pressed", "Released"]
):
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

    elif button == "Right" and state == "Released":
        global reset
        reset = True


def mouse_motion(x: float, y: float):
    if camera_action == "Rotate":
        global camera_horizontal_angle, camera_vertical_angle
        camera_horizontal_angle += x * camera_horizontal_speed
        camera_vertical_angle += y * camera_vertical_speed

    elif camera_action == "Zoom":
        global camera_distance
        camera_distance *= 1 - y * camera_mouse_motion_zoom_speed


def mouse_wheel(x: float, y: float):
    global camera_distance
    if y > 0:
        camera_distance /= y * camera_mouse_wheel_zoom_speed
    else:
        camera_distance *= - y * camera_mouse_wheel_zoom_speed


def update(delta_time: float, scene_graph, camera_node, balls: list):
    r = camera_distance
    theta = camera_horizontal_angle
    phi = camera_vertical_angle
    pos = np.array(
        [
            r * np.cos(phi) * np.sin(theta),
            r * np.sin(phi),
            r * np.cos(phi) * np.cos(theta),
        ]
    )
    camera_node.local_translation = pos

    dir = -pos
    dir = dir / np.linalg.norm(dir)

    up = np.array([0, 1, 0])
    right = np.cross(dir, up)
    right = right / np.linalg.norm(right)

    up = np.cross(right, dir)
    rot = np.stack([
        right, up, -dir
    ]).T
    rot = quaternion.from_rotation_matrix(rot)
    camera_node.local_rotation = quaternion.as_float_array(rot)

    global reset
    simulate(delta_time, balls, reset)
    reset = False