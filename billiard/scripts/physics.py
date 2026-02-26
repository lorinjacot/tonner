import numpy as np
import constraints

g = np.array([0.0, -1.0, 0.0])
N = 100


C = [
    constraints.DistanceConstraint(0, 1),
    constraints.DistanceConstraint(0, 2),
    constraints.DistanceConstraint(1, 2),
    constraints.TableConstraint(),
]


def simulate(delta_time: float, balls: list, reset: bool):
    pos = np.stack([ball.node.local_translation for ball in balls])
    vel = np.stack([ball.velocity for ball in balls])

    if reset:
        pos = np.array([
            [0.5, 0.025, 0.0],
            [0.0, 0.025, 0.001],
            [-0.06, 0.025, 0.0],
        ])
        vel = np.array([
            [2.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ])

    dt = delta_time / N

    for _ in range(N):
        vel = vel + dt * g
        old_pos = pos
        pos = pos + dt * vel

        for c in C:
            loss = c(pos)
            if np.abs(loss) > 1e-6:
                grad = c.grad(pos)
                lambd = - loss / (np.sum(np.square(grad)) + c.alpha() / dt**2)
                pos = pos + lambd * grad

        vel = (pos - old_pos) / dt

    for i in range(len(balls)):
        balls[i].node.local_translation = pos[i]
        balls[i].velocity = vel[i]