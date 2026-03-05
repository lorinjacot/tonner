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

mouse_action: Literal["Rotate", "Zoom", "Throw"] | None = None
mouse_over_ball = False

camera_horizontal_angle: float = np.pi / 4.0
camera_horizontal_speed = -1e-3
camera_vertical_angle: float = np.pi / 8.0
camera_vertical_speed = 1e-3
camera_distance = 3
camera_mouse_wheel_zoom_speed = 1e-3
camera_mouse_motion_zoom_speed = 1e-3

reset = False


def mouse_input(
    button: Literal["Left", "Right", "Middle"], state: Literal["Pressed", "Released"]
):
    global mouse_action

    if button == "Left":
        if mouse_action == None and state == "Pressed":
            mouse_action = "Rotate"
        elif mouse_action == "Rotate" and state == "Released":
            mouse_action = None

    elif button == "Middle":
        if mouse_action == None and state == "Pressed":
            mouse_action = "Zoom"
        elif mouse_action == "Zoom" and state == "Released":
            mouse_action = None

    elif button == "Right" and state == "Released":
        global reset
        reset = True


def mouse_moved(x: float, y: float, camera_node, projection_matrix: np.ndarray, balls: list):
    pass


def mouse_motion(dx: float, dy: float):
    if mouse_action == "Rotate":
        global camera_horizontal_angle, camera_vertical_angle
        camera_horizontal_angle += dx * camera_horizontal_speed
        camera_vertical_angle += dy * camera_vertical_speed

    elif mouse_action == "Zoom":
        global camera_distance
        camera_distance *= 1 + dy * camera_mouse_motion_zoom_speed


def mouse_wheel(dx: float, dy: float):
    global camera_distance
    camera_distance *= 1 - dy * camera_mouse_wheel_zoom_speed


def update(
        delta_time: float,
        scene_graph, camera_node,
        balls: list
    ):
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