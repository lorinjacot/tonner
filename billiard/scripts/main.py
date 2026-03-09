from typing import Literal
import importlib, sys

import numpy as np
import quaternion

# import debugpy
# debugpy.listen(5678, in_process_debug_adapter=True)

if "physics" in sys.modules:
    importlib.reload(sys.modules["physics"])
if "constraints" in sys.modules:
    importlib.reload(sys.modules["constraints"])
if "ray" in sys.modules:
    importlib.reload(sys.modules["ray"])

from physics import simulate
from ray import Ray

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
    button: Literal["Left", "Right", "Middle"], state: Literal["Pressed", "Released"], arrow,
):
    global mouse_action

    if button == "Left":
        if mouse_action == None and state == "Pressed":
            if mouse_over_ball:
                mouse_action = "Throw"
                arrow.show = True
            else:
                mouse_action = "Rotate"
        elif mouse_action == "Throw" and state == "Released":
            print("throwing ball")
            mouse_action = None
            arrow.show = False
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


def mouse_moved(x: float, y: float, camera_node, projection_matrix: np.ndarray, balls: list, arrow):
    def pointer_ray():
        view_proj_inv = camera_node.global_transformation @ np.linalg.inv(projection_matrix)
        
        origin = view_proj_inv @ np.array([x, y, 0.0, 1.0])
        origin = origin[:3] / origin[3]

        point = view_proj_inv @ np.array([x, y, 0.1, 1.0])
        point = point[:3] / point[3]
        dir = point - origin
        dir = dir / np.linalg.norm(dir)
        
        return Ray(origin, dir)

    if mouse_action is None:
        ray = pointer_ray()

        white_ball = balls[0]
        assert white_ball.number == 0

        global mouse_over_ball
        mouse_over_ball = ray.intersects_ball(white_ball)
    
    elif mouse_action == "Throw":
        white_ball = balls[0]
        assert white_ball.number == 0
        ball_pos = (white_ball.node.global_transformation @ np.array([0, 0, 0, 1]))[:3]

        ray = pointer_ray()
        butt = ray.intersection_table()
        if butt is None:
            return
        butt[1] = ball_pos[1]
        
        dir = ball_pos - butt
        norm = np.linalg.norm(dir)
        if norm <= white_ball.radius:
            return
        dir = dir / norm

        center = butt + dir * (norm - white_ball.radius) / 2
        arrow.node.local_translation = center
        
        x_axis = np.array([1, 0, 0])
        y_axis = dir
        z_axis = np.cross(x_axis, y_axis)
        z_axis = z_axis / np.linalg.norm(z_axis)
        x_axis = np.cross(y_axis, z_axis)
        rot = np.stack([x_axis, y_axis, z_axis]).T
        rot = quaternion.from_rotation_matrix(rot)
        arrow.node.local_rotation = quaternion.as_float_array(rot)

        arrow.node.local_scale = [1, norm - white_ball.radius, 1]


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