import numpy as np

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

def simulate(delta_time: float, balls: list, reset: bool, white_ball_impulse: np.ndarray):
    if reset:
        for ball in balls:
            ball.out = False

        d = 0.05

        dz = np.sqrt(3) / 2 * d
        dx = d

        # white
        b0 = np.array([0.0, 0.025, -0.8])

        # Row 0 (1 ball)
        b1  = np.array([0, 0, 0]) + BASE_POS

        # Row 1 (2 balls)
        b2  = np.array([-dx/2, 0, dz]) + BASE_POS
        b3  = np.array([ dx/2, 0, dz]) + BASE_POS

        # Row 2 (3 balls)
        b4  = np.array([-dx, 0, 2*dz]) + BASE_POS
        b5  = np.array([  0, 0, 2*dz]) + BASE_POS
        b6  = np.array([ dx, 0, 2*dz]) + BASE_POS

        # Row 3 (4 balls)
        b7  = np.array([-3*dx/2, 0, 3*dz]) + BASE_POS
        b8  = np.array([-dx/2,   0, 3*dz]) + BASE_POS
        b9  = np.array([ dx/2,   0, 3*dz]) + BASE_POS
        b10 = np.array([ 3*dx/2, 0, 3*dz]) + BASE_POS

        # Row 4 (5 balls)
        b11 = np.array([-2*dx, 0, 4*dz]) + BASE_POS
        b12 = np.array([-dx,   0, 4*dz]) + BASE_POS
        b13 = np.array([  0,   0, 4*dz]) + BASE_POS
        b14 = np.array([ dx,   0, 4*dz]) + BASE_POS
        b15 = np.array([ 2*dx, 0, 4*dz]) + BASE_POS

        pos = np.array([
            b0,
            b1, b2, b3,
            b4, b5, b6,
            b7, b8, b9, b10,
            b11, b12, b13, b14, b15,
        ])

        vel = np.array([
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ])
    else:
        pos = np.stack([ball.position if not ball.out else [0.0, 1e6, 0.0] for ball in balls])
        vel = np.stack([ball.velocity for ball in balls])

        vel[0] += white_ball_impulse

    for i in range(len(balls)):
        balls[i].position = pos[i]
        balls[i].velocity = vel[i]