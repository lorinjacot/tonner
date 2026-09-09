import numpy as np
from typing import Dict
import billiard as b

g = np.array([0.0, -1.0, 0.0])
drag_coefficient = 0.1
N = 10

BASE_POS = np.array([0.0, 0.025, 0.65])

def f(pos: np.ndarray, vel: np.ndarray, dt: float) -> np.ndarray:
    norms = np.linalg.norm(vel, axis=-1)
    non_zero = norms > 1e-3
    safe_vel = vel[non_zero,:]
    norms = np.linalg.norm(safe_vel, axis=-1)
    vel[non_zero,:] -= drag_coefficient * dt * safe_vel / norms[:,np.newaxis]
    return g

def gravity(pos: np.ndarray, vel: np.ndarray) -> np.ndarray:
    force = np.zeros(pos.shape) + g
    return force

def drag(pos: np.ndarray, vel: np.ndarray) -> np.ndarray:
    norms = np.linalg.norm(vel, axis=-1)
    non_zero = norms > 1e-3
    safe_vel = vel[non_zero,:]
    norms = np.linalg.norm(safe_vel, axis=-1)

    force = np.zeros(pos.shape)
    force[non_zero,:] = - drag_coefficient * safe_vel / norms[:,np.newaxis]
    return force

def simulate(delta_time: float, balls: Dict[b.BallColor, b.Ball], reset: bool, white_ball_impulse: np.ndarray):
    balls[b.BallColor.White].velocity += white_ball_impulse