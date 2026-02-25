import numpy as np

g = np.array([0.0, 0.0, 0.0])
n = 100

def integrate(delta_time: float, balls: list, reset: bool):
    pos = np.stack([ball.node.local_translation for ball in balls])
    vel = np.stack([ball.velocity for ball in balls])

    if reset:
        pos = np.array([
            [0.3, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ])
        vel = np.array([
            [0.0, 0.0, 0.0],
            [0.3, 0.0, 0.0],
        ])

    dt = delta_time / n

    for _ in range(n):
        vel = vel + dt * g
        old_pos = pos.copy()
        pos = pos + dt * vel

        vel = (pos - old_pos) / dt

    for i in range(len(balls)):
        balls[i].node.local_translation = pos[i]
        balls[i].velocity = vel[i]
